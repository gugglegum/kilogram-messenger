$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$env:PSModulePath = "$PSHOME\Modules"
if ($PSVersionTable.PSEdition -ne 'Desktop' -or $PSVersionTable.PSVersion.Major -ne 5) {
    throw 'Run this check with Windows PowerShell 5.1'
}
Import-Module "$PSHOME/Modules/NetSecurity/NetSecurity.psd1"

# Execute only the functions under test. The fixture never queries or writes a firewall store.
$tokens = $null
$errors = $null
$source = Join-Path $PSScriptRoot 'test-terminal-t1-3e-firewall.ps1'
$ast = [Management.Automation.Language.Parser]::ParseFile($source, [ref]$tokens, [ref]$errors)
if ($errors) { throw ($errors | Out-String) }
foreach ($name in 'Write-State','Read-State','Check-Enabled') {
    $function = $ast.Find({
        param($node)
        $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq $name
    }, $false)
    if (!$function) { throw "Expected function $name" }
    . ([ScriptBlock]::Create($function.Extent.Text))
}

$repo = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path $repo ".tmp\terminal-t1-3e\state-check-$([Guid]::NewGuid().ToString('N'))"
[void][IO.Directory]::CreateDirectory($fixture)
$journalPath = Join-Path $fixture 'journal.json'
Write-State $journalPath @{profile_started=$false; parent=2}
$reader = [IO.StreamReader]::new([IO.File]::Open($journalPath, 'Open', 'Read', 'ReadWrite, Delete'))
try {
    Write-State $journalPath @{profile_started=$true; parent=2}
    $before = $reader.ReadToEnd() | ConvertFrom-Json
    $after = Read-State $journalPath
    if ($before.profile_started -ne $false -or $after.profile_started -ne $true -or $after.parent -ne 2) {
        throw 'Atomic replacement must preserve the open reader and publish the new journal'
    }
} finally { $reader.Dispose() }
$gatePath = Join-Path $fixture 'gate.json'
foreach ($state in 'open','held','released') {
    Write-State $gatePath @{state=$state}
    if ((Read-State $gatePath).state -cne $state) { throw "Gate replacement failed at $state" }
}
if (@(Get-ChildItem -LiteralPath $fixture -File).Count -ne 2) { throw 'Unexpected snapshot temp or backup file' }

function Profile-View([string]$Store) {
    if ($Store -cne 'ActiveStore') { throw 'Unexpected profile query' }
    @{Enabled='True'; AllowInboundRules='True'; AllowLocalFirewallRules='True'
      DefaultInboundAction='Block'; DefaultOutboundAction='Allow'; DisabledInterfaceAliases=@()}
}
function Get-NetFirewallRule([string]$PolicyStore, [switch]$TracePolicyStore, [string[]]$Name) {
    if ($PolicyStore -cne 'ActiveStore' -or !$TracePolicyStore -or $Name.Count -ne 1 -or $Name[0] -cne 'diagnostic') {
        throw 'Unexpected firewall rule query'
    }
    $rule
}
function Assert-Rule($Spec, $Rule, [string]$Store, [string]$OwnerTag) {
    if ($Spec.Name -cne $Rule.Name -or $Store -cne 'ActiveStore' -or $OwnerTag -cne $tag) {
        throw 'Unexpected filter validation'
    }
    $script:filterChecks++
}
# Use two display projections while leaving the native CIM properties intact.
Update-TypeData -TypeName Kilogram.Test.Status.Enforced -MemberType NoteProperty -MemberName EnforcementStatus -Value @('Enforced')
Update-TypeData -TypeName Kilogram.Test.Status.Full -MemberType NoteProperty -MemberName EnforcementStatus -Value @('Full')
function New-RuleFixture([uint16[]]$Codes, [string]$Label, [uint16]$Enabled=1,
                         [string]$Owner=$tag, [string]$Source='PersistentStore') {
    $instance = [Microsoft.Management.Infrastructure.CimInstance]::new('MSFT_NetFirewallRule','root/standardcimv2')
    $instance.PSObject.TypeNames.Insert(0,'Microsoft.Management.Infrastructure.CimInstance#root/standardcimv2/MSFT_NetFirewallRule')
    $property = [Microsoft.Management.Infrastructure.CimProperty]
    $instance.CimInstanceProperties.Add($property::Create('InstanceID','diagnostic','String','None'))
    $instance.CimInstanceProperties.Add($property::Create('Description',$Owner,'String','None'))
    $instance.CimInstanceProperties.Add($property::Create('PolicyStoreSource',$Source,'String','None'))
    $instance.CimInstanceProperties.Add($property::Create('PolicyStoreSourceType',[uint16]1,'UInt16','None'))
    $instance.CimInstanceProperties.Add($property::Create('Enabled',$Enabled,'UInt16','None'))
    $instance.CimInstanceProperties.Add($property::Create('PrimaryStatus',[uint16]1,'UInt16','None'))
    $instance.CimInstanceProperties.Add($property::Create('StatusCode',[uint32]42,'UInt32','None'))
    $instance.CimInstanceProperties.Add($property::Create('Status','synthetic status','String','None'))
    if ($null -ne $Codes) {
        $instance.CimInstanceProperties.Add($property::Create('EnforcementStatus',$Codes,'UInt16Array','None'))
    }
    $instance.PSObject.TypeNames.Insert(0,"Kilogram.Test.Status.$Label")
    $instance
}

$net = @{alias='synthetic interface'}
$tag = 'Kilogram synthetic rule'
$specs = @(@{Name='diagnostic'})
$filterChecks = 0
foreach ($codes in @(@(1),@(1,5))) {
    $rule = New-RuleFixture $codes 'Enforced'
    if ($rule.EnforcementStatus[0] -cne 'Enforced') { throw 'Expected non-Full display label' }
    Check-Enabled $specs $true
}
if ($filterChecks -ne 2) { throw 'Raw Full must reach filter validation' }

$cases = [ordered]@{
    inactive=(New-RuleFixture @(5) 'Full')
    disallowed=(New-RuleFixture @(16) 'Full')
    missing_status=(New-RuleFixture -Label 'Full')
    disabled=(New-RuleFixture @(1) 'Enforced' -Enabled 2)
    wrong_owner=(New-RuleFixture @(1) 'Enforced' -Owner 'Other owner')
    wrong_source=(New-RuleFixture @(1) 'Enforced' -Source 'Other store')
}
foreach ($case in $cases.Keys) {
    $rule = $cases[$case]
    if ($case -in @('inactive','disallowed','missing_status') -and $rule.EnforcementStatus[0] -cne 'Full') {
        throw "Expected a misleading Full display label for $case"
    }
    $details = $null
    try { Check-Enabled $specs $true }
    catch {
        $prefix = 'Effective owned rules are not in the expected state: '
        if (!$_.Exception.Message.StartsWith($prefix)) { throw }
        $details = $_.Exception.Message.Substring($prefix.Length) | ConvertFrom-Json
    }
    $raw = $rule.PSBase.CimInstanceProperties['EnforcementStatus'].Value
    if (!$details -or $details.expected_enforcement_code -ne 1 -or
        ($details.observed[0].enforcement_codes -join ',') -cne ($raw -join ',') -or
        $details.observed[0].status_code -ne 42 -or $details.observed[0].status -cne $rule.Status) {
        throw "Expected scoped rejection and raw status evidence for $case"
    }
}
if ($filterChecks -ne 2) { throw 'Rejected rule must not reach filters' }

[ordered]@{
    fixture=$fixture
    atomic_open_reader_replace='PASS'
    repeated_write_and_no_leftovers='PASS'
    raw_full_and_filter_validation='PASS'
    rejected_rule_cases=@($cases.Keys)
} | ConvertTo-Json -Compress
