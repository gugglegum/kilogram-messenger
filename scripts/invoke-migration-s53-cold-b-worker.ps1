[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$AssemblySha256,
 [Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$SourceSha256,
 [Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$DecisionSha256,
 [Parameter(Mandatory)][long]$WorkDeadlineTimestamp)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$artifact="$w\target\verification-scale\s53-cold-worker"
$e="$w\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-b-01-attempt-02"
if([Environment]::ProcessPath -cne 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -or $WorkDeadlineTimestamp -le [Diagnostics.Stopwatch]::GetTimestamp()){throw 'Fixed shell/deadline'}
if((Get-FileHash -LiteralPath "$artifact\worker.dll" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $AssemblySha256 -or (Get-FileHash -LiteralPath "$artifact\worker.cs" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $SourceSha256){throw 'Reviewed assembly/source mismatch'}
# Loads only the previously materialized assembly. No activation-time Add-Type.
[Reflection.Assembly]::LoadFrom("$artifact\worker.dll")|Out-Null
. "$w\scripts\migration-s53-root-copy.ps1"
Assert-S53RootPrivateAcl $e $true
$directory=[S53RootNativeR38]::DirectoryHandle($e)
$creator=[Func[string,Microsoft.Win32.SafeHandles.SafeFileHandle]]{
 param($path)
 if($path -cnotin @("$e\comparison.private.jsonl","$e\continuation-baseline.private.json")){throw 'Exact private report path'}
 $stream=[S53RootNativeR38]::NewFile($path)
 try{$handle=[S53TypedLockWorker]::DuplicateOutput($stream.SafeFileHandle);Assert-S53RootPrivateAcl $path $false;return $handle}finally{$stream.Dispose()}
}
$acl=[Action[string]]{param($path) Assert-S53RootPrivateAcl $path ($path -ceq $e)}
exit ([S53TypedLockWorker]::RunPrivate($SourceSha256,'P2-s53-exhausted-cold-b-01-attempt-02',$WorkDeadlineTimestamp,$directory,$creator,$acl,$DecisionSha256))
