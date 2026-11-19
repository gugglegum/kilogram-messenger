param([Parameter(Mandatory)][string]$Request)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$r = ConvertFrom-Json $Request
$who = [Security.Principal.WindowsIdentity]::GetCurrent()
$admins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
if ([Security.Principal.WindowsPrincipal]::new($who).IsInRole($admins)) { throw 'Field observer must run as ordinary user' }
if ($env:COMPUTERNAME -ine $r.host -or $r.attempt -cnotmatch '^[a-z0-9][a-z0-9-]{0,30}$') { throw 'Observer scope' }
if ($r.side -eq 'B') {
    $source,$sourcePort,$dest,$destPort = $env:SSH_CONNECTION -split '\s+'
    if ($source -ne '192.168.0.140' -or $dest -ne '192.168.0.152' -or $destPort -ne '22') { throw 'Pinned SSH route changed' }
    $base = 'C:\Users\test\KilogramLab\terminal-t11f'
    $ip = '192.168.0.152'
} elseif ($r.side -eq 'A') {
    $base = 'E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f'
    $ip = '192.168.0.140'
} else { throw 'Unexpected observer side' }

function Token($Id) {
    [ordered]@{user=$Id.User.Value; groups=@($Id.Groups | ForEach-Object Value | Sort-Object -Unique)
        administrator=[Security.Principal.WindowsPrincipal]::new($Id).IsInRole($admins)}
}
function PlainFile([string]$Path) {
    $item = Get-Item -LiteralPath $Path
    $check = $item
    while ($check) {
        if ($check.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Observer cannot traverse reparse point' }
        $check = if ($check -is [IO.FileInfo]) { $check.Directory } else { $check.Parent }
    }
    if ($item.PSIsContainer) { throw 'Expected a plain file' }
    $item
}

if ($r.op -eq 'gate') {
    if ($r.command -cnotmatch '^[ABCDRSX]$' -or ($r.side -eq 'B' -and $r.command -cnotmatch '^[SX]$')) {
        throw 'Unexpected gate operation'
    }
    $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', "Kilogram-T13e-$($r.side)-$($r.attempt)", 'InOut')
    try {
        $pipe.Connect(2500)
        $pipe.WriteByte([byte][char]$r.command)
        $pipe.Flush()
        $bytes = New-Object byte[] 4096
        $n = 0
        while ($n -lt $bytes.Length) {
            $b = $pipe.ReadByte()
            if ($b -eq 10) { break }
            if ($b -lt 0) { throw 'Gate operation rejected or outcome unknown' }
            $bytes[$n++] = $b
        }
        if ($n -eq $bytes.Length) { throw 'Gate reply limit' }
        $json = [Text.Encoding]::UTF8.GetString($bytes,0,$n)
    } finally { $pipe.Dispose() }
    $json
    return
}
if ($r.op -eq 'identity') { Token $who | ConvertTo-Json -Depth 5 -Compress; return }

if ($r.op -eq 'network') {
    $source = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class T13eNlm {
 [ComImport, Guid("dcb00000-570f-4a9b-8d69-199fdba5723b"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
 interface Manager { void Slot0(); void Slot1(); Connections GetNetworkConnections(); }
 [ComImport, Guid("dcb00006-570f-4a9b-8d69-199fdba5723b"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
 interface Connections {
  void Slot0();
  [PreserveSig] int Next(uint wanted, out Connection item, out uint returned);
 }
 [ComImport, Guid("dcb00005-570f-4a9b-8d69-199fdba5723b"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
 interface Connection {
  Network GetNetwork(); void Slot1(); short IsConnected();
  void Slot3(); void Slot4(); Guid GetAdapterId();
 }
 [ComImport, Guid("dcb00002-570f-4a9b-8d69-199fdba5723b"), InterfaceType(ComInterfaceType.InterfaceIsDual)]
 interface Network {
  void Slot0(); void Slot1(); void Slot2(); void Slot3(); Guid GetNetworkId();
  void Slot5(); void Slot6(); void Slot7(); void Slot8(); void Slot9(); void Slot10(); int GetCategory();
 }
 public class Binding { public string network_id; public int category; }
 public static Binding[] Read(Guid adapterId) {
  var manager = (Manager)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("dcb00c01-570f-4a9b-8d69-199fdba5723b")));
  var connections = manager.GetNetworkConnections();
  var bindings = new List<Binding>();
  for (int i = 0; i < 128; i++) {
   uint returned;
   Connection item;
   Marshal.ThrowExceptionForHR(connections.Next(1, out item, out returned));
   if (returned == 0) return bindings.ToArray();
   if (item.GetAdapterId() != adapterId || item.IsConnected() == 0) continue;
   var network = item.GetNetwork();
   bindings.Add(new Binding { network_id = network.GetNetworkId().ToString("N"), category = network.GetCategory() });
  }
  throw new InvalidOperationException("NLM enumeration limit");
 }
}
'@
    Add-Type -TypeDefinition $source
    $adapters = @([Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces() | Where-Object {
        $_.OperationalStatus -eq 'Up' -and $_.GetIPProperties().UnicastAddresses.Address.IPAddressToString -contains $ip
    })
    if ($adapters.Count -ne 1) { throw 'Expected one live adapter at pinned IPv4' }
    $adapter = $adapters[0]
    $properties = $adapter.GetIPProperties()
    $address = @($properties.UnicastAddresses | Where-Object {$_.Address.IPAddressToString -eq $ip})
    if ($address.Count -ne 1 -or $address[0].IPv4Mask.IPAddressToString -ne '255.255.255.0') { throw 'IPv4 mask changed' }
    $ranges = @(& "$env:windir/System32/netsh.exe" int ipv4 show dynamicport udp |
        ForEach-Object { if ($_ -match ':\s*(\d+)\s*$') { [int]$Matches[1] } })
    if ($LASTEXITCODE -ne 0 -or $ranges.Count -ne 2) { throw 'UDP range observation failed' }
    @{ip=$ip; index=$properties.GetIPv4Properties().Index; adapter_id=([guid]$adapter.Id).ToString('N')
        binding=@([T13eNlm]::Read([guid]$adapter.Id)); udp=@($ranges[0],($ranges[0]+$ranges[1]-1))} |
        ConvertTo-Json -Depth 6 -Compress
    return
}

if ($r.op -notin @('status','ports','preconnect')) { throw 'Unknown observer operation' }
$all = @(& "$env:windir/System32/netstat.exe" -ano -p UDP | ForEach-Object {
    if ($_ -match '^\s*UDP\s+([^\s]+):(\d+)\s+\S+\s+(\d+)\s*$' -and $Matches[1] -in @($ip,'0.0.0.0','[::]')) {
        @{ip=$Matches[1];port=[int]$Matches[2];pid=[int]$Matches[3]}
    }
})
if ($LASTEXITCODE -ne 0) { throw 'UDP socket observation failed' }
if ($r.op -eq 'ports') { @{sockets=$all} | ConvertTo-Json -Depth 5 -Compress; return }

$exe = PlainFile (Join-Path $base 'bin/kilogram-cli.exe')
$ipc = Join-Path $base "runs/$($r.attempt)/setup/connection/ipc.json"
if ($r.op -eq 'preconnect') {
    if ($r.tag -cnotmatch '^owner-[abc]-1$') { throw 'Unexpected preconnect tag' }
    $setup=Join-Path $base "runs/$($r.attempt)/setup"
    $scope=[regex]::Escape($setup)
    $processes=@(Get-CimInstance Win32_Process -Filter "Name='kilogram-cli.exe'" | Where-Object {
        $_.ExecutablePath -ieq $exe.FullName -and $_.CommandLine.Replace('/','\') -match $scope
    })
    $chat=@($processes | Where-Object { $_.CommandLine -match '\bchat\b' })
    $runtime=@($processes | Where-Object { $_.CommandLine -match '\bruntime-from-profile\b' })
    $socketReceipt=Join-Path $base "runs/$($r.attempt)/receipts/$($r.tag)-socket.json"
    if ($chat.Count -ne 1 -or $runtime.Count -or [IO.File]::Exists($ipc) -or [IO.File]::Exists($socketReceipt)) {
        throw 'Prestarted Chat has a runtime, descriptor or socket receipt'
    }
    $sockets=@($all | Where-Object { $_.pid -in @($processes.ProcessId) })
    if ($sockets.Count) { throw 'Prestarted Chat owns a UDP socket' }
    @{disconnected=$true;descriptor=$false;sockets=$sockets;chat_pid=[int]$chat[0].ProcessId;
      runtime_pids=@();tag=$r.tag} | ConvertTo-Json -Compress
    return
}
if (![IO.File]::Exists($ipc)) { '{"ready":false}'; return }
$null = PlainFile $ipc
if ($r.sha256 -cnotmatch '^[0-9a-f]{64}$' -or (Get-FileHash -LiteralPath $exe.FullName).Hash -ine $r.sha256) {
    throw 'CLI diagnostic executable SHA-256 mismatch'
}
try {
    $raw = & $exe.FullName ordered status2 --ipc-file $ipc 2>$null
    if ($LASTEXITCODE -ne 0) { throw 'Nonzero status2' }
    $v = ConvertFrom-Json ($raw -join "`n")
} catch { throw 'T13E_STATUS2_FAILED' }
if ($v.runtime_pid -ne $r.runtime_pid -or $r.runtime_started -le 0) { throw 'T13E_RUNTIME_JOB_PID' }
try {
    $owner = Get-Process -Id $v.runtime_pid
    try {
        $actualPath = $owner.Path
        $started = $owner.StartTime.ToFileTimeUtc()
        $ownerSession = $owner.SessionId
    } finally { $owner.Dispose() }
} catch { throw 'T13E_RUNTIME_PROCESS_READ' }
if ($actualPath -ine $exe.FullName -or $actualPath -ine $r.runtime_image) { throw 'T13E_RUNTIME_IMAGE' }
if ($started -ne $r.runtime_started) { throw 'T13E_RUNTIME_START' }
if ($r.side -ceq 'B' -and ($r.owner_session -le 0 -or $ownerSession -ne $r.owner_session)) {
    throw 'T13E_RUNTIME_SESSION'
}
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;
public static class T13eToken {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetProcessTimes(IntPtr process,out long created,out long exited,out long kernel,out long user);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
 public static WindowsIdentity Read(int pid,long start) {
  var process = OpenProcess(0x1000,false,pid);
  if (process == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
  try {
   long created,exited,kernel,user;
   if (!GetProcessTimes(process,out created,out exited,out kernel,out user)) throw new Win32Exception(Marshal.GetLastWin32Error());
   if (created != start) throw new InvalidOperationException("Runtime process incarnation changed");
   const uint StillActive = 259;
   uint code;
   if (!GetExitCodeProcess(process,out code)) throw new Win32Exception(Marshal.GetLastWin32Error());
   if (code != StillActive) throw new InvalidOperationException("Runtime process already exited");
   IntPtr token;
   if (!OpenProcessToken(process,10,out token)) throw new Win32Exception(Marshal.GetLastWin32Error()); // QUERY|DUPLICATE for WindowsPrincipal.IsInRole.
   try { return new WindowsIdentity(token); }
   finally { CloseHandle(token); }
  } finally { CloseHandle(process); }
 }
}
'@
$expectedToken = if ($r.side -ceq 'B') {
    if ($r.owner_token.user -cne $who.User.Value -or $r.owner_token.administrator -isnot [bool] -or
        $r.owner_token.administrator -or !@($r.owner_token.groups).Count) { throw 'T13E_RUNTIME_EXPECTED_TOKEN' }
    $r.owner_token
} else { Token $who }
try { $runtimeIdentity = [T13eToken]::Read($v.runtime_pid,$started) }
catch { throw 'T13E_RUNTIME_TOKEN_READ' }
try {
    $runtimeToken = Token $runtimeIdentity
    if ($runtimeToken.user -cne $expectedToken.user -or $runtimeToken.administrator -or
        ($runtimeToken.groups -join '|') -cne ($expectedToken.groups -join '|')) {
        throw 'T13E_RUNTIME_TOKEN_MISMATCH'
    }
} finally { $runtimeIdentity.Dispose() }
$slots = @($v.contexts | Select-Object slot,context,role,phase,active,live_admitted,lease_expires,selected_lan_peer,
    own_locator_revision,peer_locator_revision,publication_ready,blocked,policy_error)
@{ready=$true; runtime_instance=$v.runtime_instance; runtime_pid=$v.runtime_pid; runtime_started=$started; runtime_image=$actualPath; endpoint=$v.endpoint
    discovery=$v.lan_discovery; route=$v.route; slots=$slots; token=$runtimeToken
    sockets=@($all | Where-Object pid -eq $v.runtime_pid)} | ConvertTo-Json -Depth 12 -Compress
