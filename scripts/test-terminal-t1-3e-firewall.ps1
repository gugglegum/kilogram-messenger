[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Audit','Apply','Watchdog','Rollback')][string]$Mode,
    [Parameter(Mandatory)][ValidateSet('A','B')][string]$Side,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9][a-z0-9-]{0,30}$')][string]$Attempt,
    [ValidatePattern('^S-1-5-21-(\d+-){3}\d+$')][string]$TestSid,
    [ValidatePattern('^[a-fA-F0-9]{64}$')][string]$AuditSha256
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$admin = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$system = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (!([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole($admin)) {
    throw 'Use an elevated Windows PowerShell on the physical host'
}
$root = 'C:\Program Files\Kilogram-T13e'
$run = Join-Path $root "$Side-$Attempt"
$journalPath = Join-Path $run 'journal.json'
$gatePath = Join-Path $run 'gate.json'
$pipeName = "Kilogram-T13e-$Side-$Attempt"
$tag = "Kilogram T1.3e $Side $Attempt"
$net = if ($Side -eq 'A') {
    @{host='DESKTOP-PAUL-RY'; ip='192.168.0.140'; peer='192.168.0.152'; alias64='0JHQtdGB0L/RgNC+0LLQvtC00L3QsNGPINGB0LXRgtGMIDI='
      program='E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f\bin\kilogram-cli.exe'
      index=22; adapter='b80a5d15-1daa-4d9e-8a23-d44643e3c46a'}
} else {
    @{host='PAUL_HP_OMEN'; ip='192.168.0.152'; peer='192.168.0.140'; alias64='0JHQtdGB0L/RgNC+0LLQvtC00L3QsNGPINGB0LXRgtGM'
      program='C:\Users\test\KilogramLab\terminal-t11f\bin\kilogram-cli.exe'
      index=19; adapter='e70ed7fb-d0d9-4aba-88df-364117667047'}
}
$net.alias = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($net.alias64))
if ($env:COMPUTERNAME -ine $net.host) { throw 'Unexpected Windows host' }

function Assert-AdminPath([string]$Path, [bool]$Directory) {
    $item = Get-Item -LiteralPath $Path
    $acl = Get-Acl -LiteralPath $Path
    if ([bool]$item.PSIsContainer -ne $Directory -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        !$acl.AreAccessRulesProtected -or $acl.GetOwner([Security.Principal.SecurityIdentifier]) -notin @($admin,$system)) {
        throw "Unexpected owner, inheritance or path at $Path"
    }
    foreach ($entry in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        if ($entry.AccessControlType -eq 'Allow' -and $entry.IdentityReference -notin @($admin,$system)) {
            throw "Unreviewed ACL at $Path"
        }
    }
}

function Write-State([string]$Path, $Value) {
    $temp = "$Path.$([guid]::NewGuid().ToString('N')).new"
    $bytes = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 14 -Compress))
    $file = [IO.File]::Open($temp, 'CreateNew', 'Write', 'None')
    try { $file.Write($bytes,0,$bytes.Length); $file.Flush($true) }
    finally { $file.Dispose() }
    if ([IO.File]::Exists($Path)) { [IO.File]::Replace($temp,$Path,[NullString]::Value) }
    else { [IO.File]::Move($temp,$Path) }
}

function Read-State([string]$Path) {
    # Permit the other local process to atomically replace the snapshot.
    $file = [IO.File]::Open($Path,'Open','Read',('ReadWrite, Delete'))
    $reader = [IO.StreamReader]::new($file)
    try { ConvertFrom-Json -InputObject $reader.ReadToEnd() } finally { $reader.Dispose() }
}

function Tick { [Diagnostics.Stopwatch]::GetTimestamp() }
function Due([int]$Seconds) { (Tick) + $Seconds * [Diagnostics.Stopwatch]::Frequency }

function Profile-View([string]$Store) {
    $p = Get-NetFirewallProfile -PolicyStore $Store -Name Private
    [ordered]@{Enabled=[string]$p.Enabled; DefaultInboundAction=[string]$p.DefaultInboundAction
        DefaultOutboundAction=[string]$p.DefaultOutboundAction; AllowInboundRules=[string]$p.AllowInboundRules
        AllowLocalFirewallRules=[string]$p.AllowLocalFirewallRules
        DisabledInterfaceAliases=@($p.DisabledInterfaceAliases | Sort-Object)}
}

$filterProperties = [ordered]@{
    Application=@('Program','Package'); Service=@('Service')
    Port=@('Protocol','LocalPort','RemotePort','DynamicTransport')
    Address=@('LocalAddress','RemoteAddress'); Interface=@('InterfaceAlias'); InterfaceType=@('InterfaceType')
    Security=@('Authentication','Encryption','OverrideBlockRules','LocalUser','RemoteUser','RemoteMachine')
}
$rawProperty = @{Program='AppPath'; Service='ServiceName'; LocalUser='LocalUsers'
    RemoteUser='RemoteUsers'; RemoteMachine='RemoteMachines'}

function Filter-View($Rule, $Index, [string]$Store) {
    $out = [ordered]@{}
    foreach ($kind in $filterProperties.Keys) {
        $id = $Rule.InstanceID
        if ($null -eq $Index) {
            $filters = @(& "Get-NetFirewall${kind}Filter" -AssociatedNetFirewallRule $Rule -PolicyStore $Store -ErrorAction Stop)
            if ($filters.Count -ne 1 -or $filters[0].InstanceID -ne $id) { throw "Missing association $kind / $id" }
            $filter = $filters[0]
        } else {
            $filter = $Index[$kind][$id]
            if ($null -eq $filter) { throw "Missing index entry $kind / $id" }
        }
        foreach ($key in $filterProperties[$kind]) {
            $raw = if ($rawProperty.ContainsKey($key)) { $rawProperty[$key] } else { $key }
            if ($null -eq $filter.CimInstanceProperties[$raw] -or $null -eq $filter.PSObject.Properties[$key]) {
                throw "Missing CIM property $kind.$key"
            }
            $out[$key] = @($filter.$key | ForEach-Object { [string]$_ } | Sort-Object)
        }
    }
    $out
}

function Assert-Interface {
    $adapter = @(Get-NetAdapter -InterfaceIndex $net.index)
    $address = @(Get-NetIPAddress -InterfaceIndex $net.index -AddressFamily IPv4 -IPAddress $net.ip)
    $category = @(Get-NetConnectionProfile -InterfaceIndex $net.index)
    if ($adapter.Count -ne 1 -or $adapter.Name -cne $net.alias -or $adapter.Status -ne 'Up' -or
        [guid]$adapter.InterfaceGuid -ne [guid]$net.adapter -or
        $address.Count -ne 1 -or $address.PrefixLength -ne 24 -or
        $category.Count -ne 1 -or $category.NetworkCategory -ne 'Private') { throw 'Wi-Fi binding changed' }
}

function Test-AuditCandidate($Filters, [string]$Direction, [string]$Side, [string]$ProgramPath) {
    $program = [Environment]::ExpandEnvironmentVariables(($Filters.Program -join ''))
    if ($program -ieq $ProgramPath -or $program -match '(?i)\\sshd?\.exe$') { return $true }
    if ($program -notin @('','Any') -or ($Filters.Package -join '') -notin @('','Any') -or
        ($Filters.Service -join '') -notin @('','Any')) { return $false }
    $protocol = $Filters.Protocol -join ''
    return ($protocol -in @('Any','UDP','17','256') -or $Direction -eq 'Outbound' -or
        ($Side -eq 'B' -and $protocol -in @('TCP','6')))
}

function Get-Audit {
    Assert-Interface
    $active = Profile-View ActiveStore
    $persistent = Profile-View PersistentStore
    if ($active.Enabled -ne 'False' -or $persistent.Enabled -ne 'False' -or
        $active.DefaultInboundAction -ne 'Block' -or $active.DefaultOutboundAction -ne 'Allow' -or
        $active.AllowInboundRules -ne 'True' -or $active.AllowLocalFirewallRules -ne 'True' -or
        $active.DisabledInterfaceAliases -contains $net.alias) { throw 'Unexpected effective Private profile' }
    $all = @(Get-NetFirewallRule -PolicyStore ActiveStore -TracePolicyStore | Sort-Object Name)
    $rules = @($all | Where-Object { [string]$_.Enabled -eq 'True' -and
        ([string]$_.Profile -eq 'Any' -or [string]$_.Profile -match '\bPrivate\b') })
    if (!$rules.Count -or @($all | Where-Object Name -like 'Kilogram-T13e-*').Count -or
        @(Get-NetFirewallRule -PolicyStore PersistentStore | Where-Object Name -like 'Kilogram-T13e-*').Count) {
        throw 'No active policy evidence or reserved rule names already present'
    }
    $index = @{}
    foreach ($kind in $filterProperties.Keys) {
        $byId = @{}
        foreach ($filter in (& "Get-NetFirewall${kind}Filter" -PolicyStore ActiveStore -ErrorAction Stop)) {
            if (!$filter.InstanceID -or $byId.ContainsKey($filter.InstanceID)) { throw "Ambiguous $kind filter" }
            $byId[$filter.InstanceID] = $filter
        }
        $index[$kind] = $byId
    }
    $records = @(foreach ($rule in $rules) {
        $f = Filter-View $rule $index ActiveStore
        $candidate = Test-AuditCandidate $f ([string]$rule.Direction) $Side $net.program
        if ($candidate) {
            $associated = Filter-View $rule $null ActiveStore
            if ((ConvertTo-Json $associated -Depth 4 -Compress) -cne (ConvertTo-Json $f -Depth 4 -Compress)) {
                throw "Indexed filter differs from actual association for $($rule.Name)"
            }
        }
        [ordered]@{name=$rule.Name; profile=[string]$rule.Profile; direction=[string]$rule.Direction
            action=[string]$rule.Action; edge=[string]$rule.EdgeTraversalPolicy; source=$rule.PolicyStoreSource
            source_type=[string]$rule.PolicyStoreSourceType; candidate=[bool]$candidate; filters=$f}
    })
    $plan = [ordered]@{side=$Side; host=$net.host; adapter=$net.adapter; alias=$net.alias; ip=$net.ip
        peer=$net.peer; program=$net.program; test_sid=$TestSid; active=$active; persistent=$persistent; rules=$records}
    $json = ConvertTo-Json $plan -Depth 14 -Compress
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $digest = [BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($json))).Replace('-','') }
    finally { $sha.Dispose() }
    [ordered]@{sha256=$digest.ToLowerInvariant(); policy=$plan}
}

function Spec([string]$Label, [string]$Action, [string]$Direction, [string]$Local, [string]$Port) {
    @{Name="Kilogram-T13e-$Label-$Side"; Action=$Action; Direction=$Direction
        LocalAddress=$Local; LocalPort=$Port; RemotePort=$Port}
}
$permit = @(
    (Spec DiscoveryIn Allow Inbound '239.255.75.72' 50372)
    (Spec IrohIn Allow Inbound $net.ip '49152-65535')
)
$hold = if ($Side -eq 'A') { @(
    (Spec HoldDiscoveryIn Block Inbound '239.255.75.72' 50372)
    (Spec HoldIrohIn Block Inbound $net.ip '49152-65535')
    (Spec HoldIrohOut Block Outbound $net.ip '49152-65535')
) } else { @() }
$specs = @($permit) + @($hold | Where-Object { $_ })

function Find-Rule([string]$Name, [string]$Store) {
    try { Get-NetFirewallRule -PolicyStore $Store -Name $Name }
    catch {
        if ($_.CategoryInfo.Category -ne 'ObjectNotFound' -or [string]$_.TargetObject -cne $Name -or
            $_.FullyQualifiedErrorId -notin @('CmdletizationQuery_NotFound_InstanceID,Get-NetFirewallRule',
                'CmdletizationQuery_NotFound_Name,Get-NetFirewallRule')) { throw }
    }
}

function Assert-Rule($Spec, $Rule, [string]$Store, [string]$OwnerTag) {
    if ($Rule.Name -cne $Spec.Name -or $Rule.DisplayName -cne $Spec.Name -or
        $Rule.Description -cne $OwnerTag -or [string]$Rule.Profile -ne 'Private' -or
        [string]$Rule.Action -ne $Spec.Action -or [string]$Rule.Direction -ne $Spec.Direction -or
        [string]$Rule.EdgeTraversalPolicy -ne 'Block') { throw "Rule ownership differs for $($Spec.Name)" }
    $f = Filter-View $Rule $null $Store
    $want = @{Program=$net.program; Protocol='UDP'; LocalAddress=$Spec.LocalAddress
        RemoteAddress=$net.peer; LocalPort=$Spec.LocalPort; RemotePort=$Spec.RemotePort
        InterfaceAlias=$net.alias; Package='Any'; Service='Any'; InterfaceType='Any'
        Authentication='NotRequired'; Encryption='NotRequired'; OverrideBlockRules='False'
        LocalUser='Any'; RemoteUser='Any'; RemoteMachine='Any'; DynamicTransport='Any'}
    foreach ($key in $want.Keys) {
        $actual = $f[$key] -join ','
        if ($key -in @('Package','Service','LocalUser','RemoteUser','RemoteMachine','DynamicTransport') -and !$actual) {
            $actual = 'Any'
        }
        if ($key -in @('LocalAddress','RemoteAddress')) { $actual = $actual -replace '/(32|255\.255\.255\.255)$','' }
        if ($actual -ine $want[$key]) { throw "Rule filter $key differs for $($Spec.Name): $actual" }
    }
}

function Check-Enabled($Expected, [bool]$InspectFilters=$false) {
    $profile = Profile-View ActiveStore
    if ($profile.Enabled -ne 'True' -or $profile.AllowInboundRules -ne 'True' -or
        $profile.AllowLocalFirewallRules -ne 'True' -or $profile.DefaultInboundAction -ne 'Block' -or
        $profile.DefaultOutboundAction -ne 'Allow' -or $profile.DisabledInterfaceAliases -contains $net.alias) {
        throw 'Effective Private Firewall changed'
    }
    $names = @($specs | ForEach-Object { $_.Name })
    $observed = @(Get-NetFirewallRule -PolicyStore ActiveStore -TracePolicyStore -Name $names)
    $active = @($observed | Where-Object { $_.Name -in $names -and [string]$_.Enabled -eq 'True' })
    $expectedNames = @($Expected | ForEach-Object { $_.Name })
    # NetSecurity formats raw CIM Full (1) as Enforced. Check the raw codes.
    if ($active.Count -ne $expectedNames.Count -or @($active | Where-Object {
            $_.Name -notin $expectedNames -or $_.Description -cne $tag -or
            $_.PolicyStoreSource -ne 'PersistentStore' -or
            [uint16]1 -notin @($_.PSBase.CimInstanceProperties['EnforcementStatus'].Value)
        }).Count) {
        $details = [ordered]@{expected_names=$expectedNames; expected_owner=$tag; expected_source='PersistentStore'
            expected_enforcement='Full'; expected_enforcement_code=1; enabled_count=$active.Count
            observed=@($observed | Where-Object Name -In $names | ForEach-Object {
                [ordered]@{name=$_.Name; enabled=[string]$_.Enabled; owner=$_.Description
                    source=[string]$_.PolicyStoreSource; source_type=[string]$_.PolicyStoreSourceType
                    enforcement=@($_.EnforcementStatus | ForEach-Object { [string]$_ })
                    enforcement_codes=$_.PSBase.CimInstanceProperties['EnforcementStatus'].Value
                    primary_status=[string]$_.PrimaryStatus; status_code=$_.StatusCode; status=$_.Status}
            })}
        throw "Effective owned rules are not in the expected state: $(ConvertTo-Json -InputObject $details -Depth 7 -Compress)"
    }
    if ($InspectFilters) {
        foreach ($spec in $Expected) {
            Assert-Rule $spec ($active | Where-Object Name -ceq $spec.Name) ActiveStore $tag
        }
    }
}

function Restore-Firewall($Journal) {
    $failures = @()
    if ($Journal.profile_started) {
        try {
            $local = Profile-View PersistentStore
            if ($local.Enabled -eq 'True') { Set-NetFirewallProfile -PolicyStore PersistentStore -Name Private -Enabled False }
            if ((Profile-View PersistentStore).Enabled -ne 'False' -or (Profile-View ActiveStore).Enabled -ne 'False') {
                throw 'Private profile did not return to its original state'
            }
        } catch { $failures += $_.Exception.Message }
    }
    foreach ($spec in $specs) {
        try {
            $current = @(Find-Rule $spec.Name PersistentStore)
            if ($current.Count) {
                if ($current.Count -ne 1) { throw "Ambiguous rollback name $($spec.Name)" }
                Assert-Rule $spec $current[0] PersistentStore $Journal.tag
                Remove-NetFirewallRule -PolicyStore PersistentStore -Name $spec.Name -Confirm:$false
            }
            if (@(Find-Rule $spec.Name ActiveStore).Count) { throw "Name remains in ActiveStore: $($spec.Name)" }
        } catch { $failures += $_.Exception.Message }
    }
    Write-State (Join-Path $run 'rollback.json') @{ok=($failures.Count -eq 0); errors=$failures; tick=(Tick)}
    if ($failures.Count) { throw ($failures -join ' / ') }
}

function Gate-Reply($State, [string]$Operation) {
    $now = Tick
    [ordered]@{host=$net.host; side=$Side; attempt=$Attempt; op=$Operation; tick=$now
        frequency=[Diagnostics.Stopwatch]::Frequency; audit=$AuditSha256.ToLowerInvariant()
        phase=$State.phase; state=$State.state; expires=$State.expires; uid=$TestSid
        seconds_left=[Math]::Max(0,($journal.end-$now)/[Diagnostics.Stopwatch]::Frequency)}
}

Assert-AdminPath $root $true
if ($PSCommandPath -ine (Join-Path $root 'test-terminal-t1-3e-firewall.ps1')) {
    throw 'Run a reviewed copy from the protected administrative directory'
}
Assert-AdminPath $PSCommandPath $false

if ($Mode -eq 'Audit') {
    if (!$TestSid) { throw 'Provide the ordinary test user SID recorded on this host' }
    Get-Audit | ConvertTo-Json -Depth 14
    exit
}

if ($Mode -in @('Watchdog','Rollback')) {
    Assert-AdminPath $run $true
    $journal = Read-State $journalPath
    $parent = Get-Process -Id $journal.parent -ErrorAction SilentlyContinue
    if ($parent -and $parent.StartTime.ToUniversalTime().Ticks -ne $journal.parent_start) { $parent = $null }
    if ($Mode -eq 'Rollback' -and $parent -and !$parent.HasExited) { throw 'Use the armed bridge or wait for its exit' }
    if ($Mode -eq 'Watchdog') {
        if (!$parent) { throw 'Bridge process was not present at watchdog startup' }
        $null = $parent.Handle
        Write-State (Join-Path $run 'watchdog.json') @{pid=$PID; tick=(Tick)}
        while ($parent -and !$parent.HasExited) {
            if ([IO.File]::Exists($gatePath)) { $gate = Read-State $gatePath }
            if ([IO.File]::Exists((Join-Path $run 'rollback.json')) -and (Read-State (Join-Path $run 'rollback.json')).ok) { exit }
            if ((Tick) -ge $journal.end -or ($gate.state -eq 'held' -and (Tick) -ge $gate.expires)) {
                # This handle identifies this attempt's original elevated process, including after PID reuse.
                $parent.Kill()
                if (!$parent.WaitForExit(5000)) { throw 'Owned bridge did not exit before watchdog rollback' }
                break
            }
            Start-Sleep -Milliseconds 200
        }
        $journal = Read-State $journalPath
        $rollback = Join-Path $run 'rollback.json'
        if ([IO.File]::Exists($rollback) -and (Read-State $rollback).ok) { exit }
    }
    Restore-Firewall $journal
    exit
}

if (!$TestSid -or !$AuditSha256 -or (Get-Audit).sha256 -ine $AuditSha256) {
    throw 'Provide the reviewed SHA-256 of the current administrative audit for this host'
}
if ((Get-FileHash -LiteralPath $net.program).Hash -ine '736827d50c3cfb4f34282b6651dccc82f25131c4c86a8594105771a38cda1ac2') {
    throw 'Stable test CLI does not match the accepted candidate'
}
if (Test-Path -LiteralPath $run) { throw 'Administrative attempt directory already exists' }
[void][IO.Directory]::CreateDirectory($run)
$acl = Get-Acl -LiteralPath $root
Set-Acl -LiteralPath $run -AclObject $acl
Assert-AdminPath $run $true
$journal = @{tag=$tag; parent=$PID; parent_start=(Get-Process -Id $PID).StartTime.ToUniversalTime().Ticks
    end=(Due 1800); profile_started=$false}
Write-State $journalPath $journal
$argsForWatchdog = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',('"' + $PSCommandPath + '"'),
    '-Mode','Watchdog','-Side',$Side,'-Attempt',$Attempt)
$watch = Start-Process "$PSHOME\powershell.exe" -ArgumentList $argsForWatchdog -WindowStyle Hidden -PassThru
$watchReady = Join-Path $run 'watchdog.json'
$armBy = Due 10
while (![IO.File]::Exists($watchReady) -and !$watch.HasExited -and (Tick) -lt $armBy) { Start-Sleep -Milliseconds 100 }
if ($watch.HasExited -or ![IO.File]::Exists($watchReady) -or (Read-State $watchReady).pid -ne $watch.Id) {
    throw 'Independent watchdog failed to arm'
}

$state = @{phase=''; state='open'; expires=0}
$next = 0
$phases = 'ABCD'
$security = [IO.Pipes.PipeSecurity]::new()
foreach ($rule in @(
    [IO.Pipes.PipeAccessRule]::new([Security.Principal.SecurityIdentifier]::new($TestSid),'ReadWrite','Allow')
    [IO.Pipes.PipeAccessRule]::new($admin,'FullControl','Allow')
)) { $security.AddAccessRule($rule) }
Microsoft.PowerShell.Utility\Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class T13eLocalPipe {
 [StructLayout(LayoutKind.Sequential)]
 struct SecurityAttributes { public int Length; public IntPtr Descriptor; public int Inherit; }
 [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
 static extern SafePipeHandle CreateNamedPipe(string path, uint access, uint mode, uint count,
     uint output, uint input, uint timeout, ref SecurityAttributes security);
 public static NamedPipeServerStream Create(string name, PipeSecurity security) {
  var pin = GCHandle.Alloc(security.GetSecurityDescriptorBinaryForm(), GCHandleType.Pinned);
  try {
   var attributes = new SecurityAttributes {
    Length = Marshal.SizeOf(typeof(SecurityAttributes)), Descriptor = pin.AddrOfPinnedObject()
   };
   // Duplex, overlapped, first instance. PIPE_REJECT_REMOTE_CLIENTS tests the connection,
   // so the ordinary SSH-launched local client may still carry Windows' NETWORK group.
   var pipe = CreateNamedPipe(@"\\.\pipe\" + name, 0x40080003, 8, 1, 4096, 128, 0, ref attributes);
   if (pipe.IsInvalid) {
    int error = Marshal.GetLastWin32Error();
    pipe.Dispose();
    throw new Win32Exception(error);
   }
   try { return new NamedPipeServerStream(PipeDirection.InOut, true, false, pipe); }
   catch { pipe.Dispose(); throw; }
  } finally { pin.Free(); }
 }
}
'@
$applyError = $null
try {
    foreach ($s in $specs) {
        $params = $s.Clone()
        $params.Enabled = if ($s.Action -eq 'Allow') { 'True' } else { 'False' }
        if ($s.Direction -eq 'Inbound') { $params.EdgeTraversalPolicy = 'Block' }
        $r = New-NetFirewallRule @params -PolicyStore PersistentStore -DisplayName $s.Name -Description $tag `
            -Profile Private -Protocol UDP -Program $net.program -RemoteAddress $net.peer -InterfaceAlias $net.alias
        Assert-Rule $s $r PersistentStore $tag
    }
    $journal.profile_started = $true
    Write-State $journalPath $journal
    Set-NetFirewallProfile -PolicyStore PersistentStore -Name Private -Enabled True
    $expected = @($permit)
    Check-Enabled $expected $true
    "Armed $pipeName"
    while ((Tick) -lt $journal.end -and ($state.state -ne 'held' -or (Tick) -lt $state.expires)) {
        $server = [T13eLocalPipe]::Create($pipeName,$security)
        try {
            $accept = $server.WaitForConnectionAsync()
            while (!$accept.Wait(200)) {
                if ((Tick) -ge $journal.end -or ($state.state -eq 'held' -and (Tick) -ge $state.expires)) { break }
            }
            if (!$server.IsConnected -or (Tick) -ge $journal.end -or
                ($state.state -eq 'held' -and (Tick) -ge $state.expires)) { break }
            $buffer = New-Object byte[] 1
            $read = $server.ReadAsync($buffer,0,1)
            if (!$read.Wait(800) -or $read.Result -ne 1) { continue }
            $op = [string][char]$buffer[0]
            $done = $op -ceq 'X'
            if ($done) {
                Restore-Firewall $journal
                $state.state = 'rolled_back'
            } elseif ($op -ceq 'R' -and $Side -eq 'A' -and $state.state -eq 'held' -and (Tick) -lt $state.expires) {
                foreach ($item in $expected | Where-Object Action -eq Block) {
                    Assert-Rule $item @(Find-Rule $item.Name PersistentStore)[0] PersistentStore $tag
                    Disable-NetFirewallRule -PolicyStore PersistentStore -Name $item.Name
                }
                $expected = @($permit)
                Check-Enabled $expected $true
                if ((Tick) -ge $state.expires) { throw 'Release finished after the hold expired' }
                $state.state = 'released'
                Write-State $gatePath $state
                $next++
            } elseif ($Side -eq 'A' -and $next -lt 4 -and $op -ceq [string]$phases[$next] -and
                $state.state -in @('open','released')) {
                $budget = if ($op -ceq 'D') { 40 } else { 55 }
                $state = @{phase=$op; state='held'; expires=(Due $budget)}
                Write-State $gatePath $state
                $blocking = @($hold | Where-Object { $_.Direction -eq 'Inbound' -or $op -ceq 'D' })
                foreach ($item in $blocking) {
                    Assert-Rule $item @(Find-Rule $item.Name PersistentStore)[0] PersistentStore $tag
                    Enable-NetFirewallRule -PolicyStore PersistentStore -Name $item.Name
                }
                $expected = @($permit) + $blocking
                Check-Enabled $expected $true
            } elseif ($op -cne 'S') { continue }
            if ($op -ceq 'S') { Check-Enabled $expected }
            $payload = [Text.Encoding]::UTF8.GetBytes(((Gate-Reply $state $op | ConvertTo-Json -Compress) + "`n"))
            $write = $server.WriteAsync($payload,0,$payload.Length)
            if (!$write.Wait(800)) { throw 'Gate client write deadline' }
            if ($done) { break }
        } finally { $server.Dispose() }
    }
} catch {
    $applyError = $_
    throw
} finally {
    try {
        $receipt = Join-Path $run 'rollback.json'
        if (![IO.File]::Exists($receipt) -or !(Read-State $receipt).ok) { Restore-Firewall $journal }
    } catch {
        if (!$applyError) { throw }
        [Console]::Error.WriteLine("T1.3e rollback also failed: $($_.Exception.Message)")
    }
}
