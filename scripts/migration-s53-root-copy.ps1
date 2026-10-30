# Definitions only. r38 source submission: never dot-source/execute without a later allocation.
Set-StrictMode -Version Latest

function Stop-S53RootCopy([string]$Code) {
    $e = [InvalidOperationException]::new('RootCopies bounded refusal')
    $e.Data['rootcopies_code'] = $Code
    throw $e
}
function Get-S53RootFailure($Exception) {
    while ($Exception.InnerException) { $Exception=$Exception.InnerException }
    if ($Exception.Data.Contains('rootcopies_code') -and
        [string]$Exception.Data['rootcopies_code'] -cmatch '\A[a-z_]{1,64}\z') {
        return [string]$Exception.Data['rootcopies_code']
    }
    return 'io_or_native_failure'
}
function Get-S53RootDigest([byte[]]$Bytes) {
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}
function Assert-S53RootPath([string]$Path, [string]$Expected) {
    if ($Path -cmatch '[\x00-\x1f]' -or $Path -notmatch '\A[A-Za-z]:\\' -or
        $Path.Substring(3).Contains(':') -or $Path -match '(?:\A|\\)\.{1,2}(?:\\|\z)' -or
        -not [string]::Equals($Path,$Expected,[StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)) {
        Stop-S53RootCopy 'literal_path'
    }
}
function Initialize-S53RootNative {
    if (-not $IsWindows) { Stop-S53RootCopy 'windows_required' }
    if ('S53RootNativeR38' -as [type]) { return }
    # Not compiled during r38. Public preflight must compile/exercise this under its own future C gate.
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Threading.Tasks;
using Microsoft.Win32.SafeHandles;
public static class S53RootNativeR38 {
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME Created, Accessed, Written;
        public uint Volume, SizeHigh, SizeLow, Links, IndexHigh, IndexLow;
    }
    [StructLayout(LayoutKind.Sequential)] struct SA {
        public int Length; public IntPtr Descriptor; [MarshalAs(UnmanagedType.Bool)] public bool Inherit;
    }
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern SafeFileHandle CreateFileW(string p,uint a,uint s,IntPtr sa,uint c,uint f,IntPtr t);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true,EntryPoint="CreateFileW")]
    static extern SafeFileHandle CreateSecureFile(string p,uint a,uint s,ref SA sa,uint c,uint f,IntPtr t);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern bool CreateDirectoryW(string p,ref SA sa);
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string s,uint rev,out IntPtr sd,out uint bytes);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr p);
    [DllImport("kernel32.dll",SetLastError=true)]
    static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern uint GetFinalPathNameByHandleW(SafeFileHandle h,StringBuilder b,uint n,uint flags);
    static Exception Refuse(string code) {
        var e=new InvalidOperationException("RootCopies bounded native refusal");
        e.Data["rootcopies_code"]=code; return e;
    }
    static string User() {
        using(var identity=WindowsIdentity.GetCurrent()) {
            if(identity.User==null) throw Refuse("owner_sid");
            return identity.User.Value;
        }
    }
    static SA Security(out IntPtr sd) {
        uint size;
        string s="O:"+User()+"D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;"+User()+")";
        if(!ConvertStringSecurityDescriptorToSecurityDescriptorW(s,1,out sd,out size)) throw new Win32Exception();
        return new SA { Length=Marshal.SizeOf(typeof(SA)), Descriptor=sd, Inherit=false };
    }
    static Info Check(SafeFileHandle h,string path,bool directory) {
        if(h.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
        Info i; if(!GetFileInformationByHandle(h,out i)) throw new Win32Exception();
        if((i.Attributes & (0x400u|0x40u))!=0 ||
           ((i.Attributes & 0x10u)!=0)!=directory || (!directory && i.Links!=1)) throw Refuse("handle_shape_or_alias");
        var name=new StringBuilder(32768);
        uint n=GetFinalPathNameByHandleW(h,name,(uint)name.Capacity,0);
        if(n==0 || n>=name.Capacity) throw Refuse("handle_path");
        string actual=name.ToString();
        if(actual.StartsWith(@"\\?\",StringComparison.Ordinal)) actual=actual.Substring(4);
        if(!String.Equals(actual.TrimEnd('\\'),path.TrimEnd('\\'),StringComparison.OrdinalIgnoreCase)) throw Refuse("handle_path");
        return i;
    }
    public static SafeFileHandle DirectoryHandle(string path) {
        var h=CreateFileW(path,0x80,3,IntPtr.Zero,3,0x02200000,IntPtr.Zero);
        try { Check(h,path,true); var result=h; h=null; return result; }
        finally { if(h!=null) h.Dispose(); }
    }
    public static FileStream Read(string path,bool exclusive) {
        var h=CreateFileW(path,0x80000000,exclusive?0u:1u,IntPtr.Zero,3,0x00200000,IntPtr.Zero);
        try { Check(h,path,false); var f=new FileStream(h,FileAccess.Read,16384,false); h=null; return f; }
        finally { if(h!=null) h.Dispose(); }
    }
    public static string Stamp(FileStream f,string path) {
        var i=Check(f.SafeFileHandle,path,false);
        return String.Join(":",i.Volume,i.IndexHigh,i.IndexLow,i.Attributes,i.SizeHigh,i.SizeLow,
            i.Created.dwHighDateTime,i.Created.dwLowDateTime,i.Written.dwHighDateTime,i.Written.dwLowDateTime);
    }
    public static void NewDirectory(string path) {
        IntPtr sd=IntPtr.Zero;
        try { var sa=Security(out sd); if(!CreateDirectoryW(path,ref sa)) throw new Win32Exception(); }
        finally { if(sd!=IntPtr.Zero) LocalFree(sd); }
    }
    public static FileStream NewFile(string path) {
        IntPtr sd=IntPtr.Zero; SafeFileHandle h=null;
        try {
            var sa=Security(out sd);
            h=CreateSecureFile(path,0xC0000000,0,ref sa,1,0x00200000,IntPtr.Zero);
            Check(h,path,false);
            var f=new FileStream(h,FileAccess.ReadWrite,16384,false); h=null; return f;
        } finally { if(h!=null) h.Dispose(); if(sd!=IntPtr.Zero) LocalFree(sd); }
    }
}
// No PowerShell callback executes on pipe threads. Each stream has its own bounded sink.
public sealed class S53RootCaptureR38 {
    readonly FileStream stdout,stderr;
    readonly Task<long> a,b;
    public S53RootCaptureR38(System.Diagnostics.Process process,FileStream output,FileStream error) {
        stdout=output; stderr=error;
        a=Task.Run(()=>Drain(process.StandardOutput.BaseStream,stdout));
        b=Task.Run(()=>Drain(process.StandardError.BaseStream,stderr));
    }
    static long Drain(Stream input,FileStream output) {
        byte[] buffer=new byte[16384]; long size=0; int n;
        while((n=input.Read(buffer,0,buffer.Length))!=0) {
            if(size+n>131072) { var e=new IOException("bounded pipe cap"); e.Data["rootcopies_code"]="cli_output_cap"; throw e; }
            output.Write(buffer,0,n); size+=n;
        }
        output.Flush(true); return size;
    }
    public bool Completed { get { return a.IsCompleted && b.IsCompleted; } }
    public bool Faulted { get { return a.IsFaulted || b.IsFaulted; } }
    public long[] CloseCompleted() {
        if(!Completed) throw new InvalidOperationException("pipe not drained");
        try { return new long[] { a.GetAwaiter().GetResult(),b.GetAwaiter().GetResult() }; }
        finally { stdout.Dispose(); stderr.Dispose(); }
    }
}
'@
}

function Assert-S53RootPrivateAcl([string]$Path, [bool]$Directory) {
    $item=if($Directory){[IO.DirectoryInfo]::new($Path)}else{[IO.FileInfo]::new($Path)}
    $acl=[IO.FileSystemAclExtensions]::GetAccessControl($item)
    $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
    try { $user=$identity.User.Value } finally { $identity.Dispose() }
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -cne $user -or
        ($Directory -and -not $acl.AreAccessRulesProtected)) { Stop-S53RootCopy 'private_acl' }
    $rules=$acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($rule in $rules) {
        $sid=$rule.IdentityReference.Value
        if ($sid -cnotin @($user,'S-1-5-18','S-1-5-32-544') -or
            $rule.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or
            ($rule.FileSystemRights -band [Security.AccessControl.FileSystemRights]::FullControl) -ne [Security.AccessControl.FileSystemRights]::FullControl) { Stop-S53RootCopy 'private_acl' }
        $null=$seen.Add($sid)
    }
    if (-not $seen.Contains($user)) { Stop-S53RootCopy 'private_acl' }
}
function New-S53RootDirectory([string]$Path,[scriptblock]$Created=$null) {
    Assert-S53RootPath $Path $Path
    [S53RootNativeR38]::NewDirectory($Path)
    # Signal ownership immediately after native fail-if-exists creation, even if a
    # subsequent verification fails. Never claim ownership of a collision.
    if($Created){& $Created}
    $handle=[S53RootNativeR38]::DirectoryHandle($Path)
    try{Assert-S53RootPrivateAcl $Path $true}finally{$handle.Dispose()}
}
function Add-S53RootAncestors([string]$Path, $Held, [scriptblock]$Check) {
    $chain=[Collections.Generic.Stack[string]]::new()
    $at=[IO.Path]::GetDirectoryName($Path)
    while ($at) { $chain.Push($at); $at=[IO.Path]::GetDirectoryName($at) }
    foreach ($directory in $chain) { & $Check; $Held.Add([S53RootNativeR38]::DirectoryHandle($directory)) }
}
function Read-S53RootStream([IO.FileStream]$Stream, [long]$Maximum, [scriptblock]$Check) {
    if ($Stream.Length -gt $Maximum -or $Stream.Length -lt 0) { Stop-S53RootCopy 'file_cap' }
    $Stream.Position=0; $memory=[IO.MemoryStream]::new()
    try {
        $buffer=[byte[]]::new(16384)
        while ($true) {
            & $Check; $n=$Stream.Read($buffer,0,$buffer.Length); & $Check
            if ($n -eq 0) { break }
            if ($memory.Length+$n -gt $Maximum) { Stop-S53RootCopy 'file_cap' }
            $memory.Write($buffer,0,$n)
        }
        if ($Stream.Length -ne $memory.Length) { Stop-S53RootCopy 'file_changed' }
        return ,$memory.ToArray()
    } finally { $memory.Dispose() }
}
function Get-S53RootEntries([string]$Root, $Budget, [scriptblock]$Check) {
    # Incremental enumeration is capped BEFORE sorting/materializing entries.
    $rows=[Collections.Generic.List[object]]::new()
    $names=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $required=@('account-root-secret.key','authority-log-version','next-authority-sequence','authority-write.lock','revocations','conversation-memberships')
    $enumerator=[IO.DirectoryInfo]::new($Root).EnumerateFileSystemInfos().GetEnumerator()
    try {
        while ($enumerator.MoveNext()) {
            & $Check; $entry=$enumerator.Current
            if (-not $names.Add($entry.Name) -or $names.Count -gt 7) { Stop-S53RootCopy 'layout' }
            if (($entry.Attributes -band ([IO.FileAttributes]::ReparsePoint -bor [IO.FileAttributes]::Device)) -ne 0) { Stop-S53RootCopy 'layout' }
            $isDir=($entry.Attributes -band [IO.FileAttributes]::Directory) -ne 0
            if ($isDir) {
                if ($entry.Name -cnotin @('revocations','conversation-memberships')) { Stop-S53RootCopy 'layout' }
                $Budget.dirs++; if ($Budget.dirs -gt 64) { Stop-S53RootCopy 'directory_cap' }
                $rows.Add([ordered]@{relative=$entry.Name;directory=$true})
                $suffix=if($entry.Name -ceq 'revocations'){'revocation'}else{'membership'}
                $leaves=$entry.EnumerateFileSystemInfos().GetEnumerator()
                try {
                    while ($leaves.MoveNext()) {
                        & $Check; $leaf=$leaves.Current
                        $Budget.files++; if ($Budget.files -gt 256) { Stop-S53RootCopy 'file_count_cap' }
                        if ($leaf.Name -cnotmatch ("\A[0-9a-f]{64}\."+$suffix+'\z') -or
                            ($leaf.Attributes -band ([IO.FileAttributes]::ReparsePoint -bor [IO.FileAttributes]::Directory -bor [IO.FileAttributes]::Device)) -ne 0) { Stop-S53RootCopy 'layout' }
                        $rows.Add([ordered]@{relative=($entry.Name+'\'+$leaf.Name);directory=$false})
                    }
                } finally { $leaves.Dispose() }
            } else {
                if ($entry.Name -cnotin @('account-root-secret.key','authority-log-version','next-authority-sequence','authority-write.lock','account-device-list.snapshot')) { Stop-S53RootCopy 'layout' }
                $Budget.files++; if ($Budget.files -gt 256) { Stop-S53RootCopy 'file_count_cap' }
                $rows.Add([ordered]@{relative=$entry.Name;directory=$false})
            }
        }
    } finally { $enumerator.Dispose() }
    foreach ($name in $required) { if (-not $names.Contains($name)) { Stop-S53RootCopy 'layout_missing' } }
    return ,@($rows | Sort-Object -Property relative -CaseSensitive)
}
function Open-S53RootSnapshot([string]$Root, $Budget, $Held, [scriptblock]$Check, [IO.FileStream]$ExistingLock=$null) {
    Assert-S53RootPath $Root $Root
    $Budget.dirs++; if ($Budget.dirs -gt 64) { Stop-S53RootCopy 'directory_cap' }
    $Held.Add([S53RootNativeR38]::DirectoryHandle($Root))
    $entries=Get-S53RootEntries $Root $Budget $Check
    $files=[Collections.Generic.List[object]]::new()
    $sequence=$null; $revocations=0
    # Parent already holds BOTH original authority locks before calling this.
    # Standalone synthetic/destination snapshots acquire their own lock first.
    $readOrder=@($entries | Sort-Object @{Expression={if($_.relative -ceq 'authority-write.lock'){0}else{1}}},relative)
    foreach ($entry in $readOrder) {
        & $Check; $path=Join-Path $Root $entry.relative
        if ($entry.directory) { $Held.Add([S53RootNativeR38]::DirectoryHandle($path)); continue }
        $stream=$null; $external=$entry.relative -ceq 'authority-write.lock' -and $null -ne $ExistingLock
        try {
            $stream=if($external){$ExistingLock}else{[S53RootNativeR38]::Read($path,($entry.relative -ceq 'authority-write.lock'))}
            $stamp=[S53RootNativeR38]::Stamp($stream,$path)
            $maximum=if($entry.relative -cin @('authority-log-version','next-authority-sequence')){4096}else{65536}
            $raw=Read-S53RootStream $stream $maximum $Check
            $Budget.bytes+=$raw.Length; if ($Budget.bytes -gt 16777216) { Stop-S53RootCopy 'total_bytes_cap' }
            if ($entry.relative -ceq 'authority-write.lock' -and $raw.Length -ne 0) { Stop-S53RootCopy 'lock_nonempty' }
            if ($entry.relative -ceq 'account-root-secret.key' -and $raw.Length -eq 0) { Stop-S53RootCopy 'key_empty' }
            if ($entry.relative -cin @('authority-log-version','next-authority-sequence')) {
                $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
                if ($entry.relative -ceq 'authority-log-version') {
                    if ($text -cnotmatch '\A1(?:\r?\n)?\z') { Stop-S53RootCopy 'authority_version' }
                } else {
                    [ulong]$number=0
                    if ($text -cnotmatch '\A(?:0|[1-9][0-9]{0,19})(?:\r?\n)?\z' -or
                        -not [ulong]::TryParse($text.TrimEnd([char]10,[char]13),[Globalization.NumberStyles]::None,[Globalization.CultureInfo]::InvariantCulture,[ref]$number)) { Stop-S53RootCopy 'authority_sequence' }
                    $sequence=$number
                }
            }
            if ($entry.relative.StartsWith('revocations\',[StringComparison]::Ordinal)) { $revocations++ }
            if ([S53RootNativeR38]::Stamp($stream,$path) -cne $stamp) { Stop-S53RootCopy 'file_changed' }
            $files.Add([ordered]@{relative=$entry.relative;path=$path;stream=$stream;stamp=$stamp;bytes=$raw.Length;sha256=(Get-S53RootDigest $raw)})
            if(-not $external){$Held.Add($stream)}
            $stream=$null; $raw=$null
        } finally { if($null -ne $stream -and -not $external){$stream.Dispose()} }
    }
    return [ordered]@{root=$Root;entries=$entries;files=$files;sequence=$sequence;revocations=$revocations}
}
function Get-S53RootTuple($Snapshot) {
    return ,@($Snapshot.files | ForEach-Object { [ordered]@{path=$_.relative;bytes=$_.bytes;sha256=$_.sha256;provenance=if($_.relative -ceq 'authority-write.lock'){'verified-empty-existing-lock'}else{'actual-byte-read'}} })
}
function Assert-S53RootPreserved($Snapshot, [scriptblock]$Check) {
    $budget=@{files=0;dirs=1;bytes=0}
    $now=Get-S53RootEntries $Snapshot.root $budget $Check
    if (($now|ConvertTo-Json -Compress) -cne ($Snapshot.entries|ConvertTo-Json -Compress)) { Stop-S53RootCopy 'entry_set_changed' }
    foreach ($file in $Snapshot.files) {
        & $Check
        if ([S53RootNativeR38]::Stamp($file.stream,$file.path) -cne $file.stamp) { Stop-S53RootCopy 'source_changed' }
        $raw=Read-S53RootStream $file.stream 65536 $Check
        if ($raw.Length -ne $file.bytes -or (Get-S53RootDigest $raw) -cne $file.sha256 -or
            [S53RootNativeR38]::Stamp($file.stream,$file.path) -cne $file.stamp) { Stop-S53RootCopy 'source_changed' }
        $raw=$null
    }
}
function Copy-S53RootSnapshot($Snapshot, [string]$Destination, [scriptblock]$Check) {
    Assert-S53RootPath $Destination $Destination
    New-S53RootDirectory $Destination
    foreach ($entry in $Snapshot.entries) {
        & $Check
        if ($entry.directory) { New-S53RootDirectory (Join-Path $Destination $entry.relative) }
    }
    foreach ($file in $Snapshot.files) {
        & $Check; $target=Join-Path $Destination $file.relative
        $copy=[S53RootNativeR38]::NewFile($target)
        try {
            Assert-S53RootPrivateAcl $target $false
            $raw=Read-S53RootStream $file.stream 65536 $Check
            if ($raw.Length -ne $file.bytes -or (Get-S53RootDigest $raw) -cne $file.sha256) { Stop-S53RootCopy 'source_changed' }
            # The verified-empty OS lock is CreateNew-empty; no original is ever written.
            $copy.Write($raw,0,$raw.Length); $copy.Flush($true)
            $actual=Read-S53RootStream $copy 65536 $Check
            if ([Convert]::ToBase64String($raw) -cne [Convert]::ToBase64String($actual)) { Stop-S53RootCopy 'copy_equality' }
            $raw=$null; $actual=$null
        } finally { $copy.Dispose() } # R key handles MUST be closed before the CLI can replace an envelope.
    }
    Assert-S53RootPreserved $Snapshot $Check
}
function Assert-S53RootDestination($Original, [string]$Destination, [bool]$AllowKeyMigration, [scriptblock]$Check) {
    $held=[Collections.Generic.List[object]]::new()
    try {
        $budget=@{files=0;dirs=0;bytes=0}
        $copy=Open-S53RootSnapshot $Destination $budget $held $Check
        if (($copy.entries|ConvertTo-Json -Compress) -cne ($Original.entries|ConvertTo-Json -Compress)) { Stop-S53RootCopy 'destination_changed' }
        foreach ($file in $copy.files) {
            Assert-S53RootPrivateAcl $file.path $false
            $expected=@($Original.files | Where-Object { $_.relative -ceq $file.relative })
            if ($expected.Count -ne 1 -or
                (($file.relative -cne 'account-root-secret.key' -or -not $AllowKeyMigration) -and
                ($file.bytes -ne $expected[0].bytes -or $file.sha256 -cne $expected[0].sha256))) { Stop-S53RootCopy 'destination_changed' }
        }
        return ,(Get-S53RootTuple $copy)
    } finally { foreach($handle in $held){$handle.Dispose()} } # No R verification handle survives into a CLI call.
}
function Write-S53RootPrivate([string]$Path, [byte[]]$Raw, [long]$Maximum=1048576) {
    if ($Raw.Length -gt $Maximum) { Stop-S53RootCopy 'private_receipt_cap' }
    $stream=[S53RootNativeR38]::NewFile($Path)
    try { Assert-S53RootPrivateAcl $Path $false; $stream.Write($Raw,0,$Raw.Length); $stream.Flush($true) }
    finally { $stream.Dispose() }
}
function Assert-S53RootProofTree([string]$Root,[scriptblock]$Check) {
    $expected=@('owner.private.json','copy-before-load.private.json','a.after-load.private.json','b.after-load.private.json','source-after.private.json','a.authority','b.authority','a.show.stdout','a.show.stderr','a.snapshot.stdout','a.snapshot.stderr','b.show.stdout','b.show.stderr','b.snapshot.stdout','b.snapshot.stderr')
    $top=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $walk=[IO.DirectoryInfo]::new($Root).EnumerateFileSystemInfos().GetEnumerator()
    try{while($walk.MoveNext()){
        & $Check;$item=$walk.Current
        if($top.Count -ge 3 -or -not $top.Add($item.Name) -or $item.Name -cnotin @('two.account','one.account','proof') -or
            ($item.Attributes -band [IO.FileAttributes]::Directory) -eq 0 -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){Stop-S53RootCopy 'r_tree_changed'}
        Assert-S53RootPrivateAcl $item.FullName $true
    }}finally{$walk.Dispose()}
    if($top.Count -ne 3){Stop-S53RootCopy 'r_tree_changed'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $walk=[IO.DirectoryInfo]::new("$Root\proof").EnumerateFileSystemInfos().GetEnumerator()
    try{while($walk.MoveNext()){
        & $Check;$item=$walk.Current
        if($seen.Count -ge $expected.Count -or -not $seen.Add($item.Name) -or $item.Name -cnotin $expected -or
            ($item.Attributes -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint -bor [IO.FileAttributes]::Device)) -ne 0){Stop-S53RootCopy 'proof_tree_changed'}
        $maximum=if($item.Extension -ceq '.authority'){16384}elseif($item.Extension -cin @('.stdout','.stderr')){131072}else{1048576}
        if($item.Length -gt $maximum){Stop-S53RootCopy 'proof_tree_cap'}
        Assert-S53RootPrivateAcl $item.FullName $false
    }}finally{$walk.Dispose()}
    if($seen.Count -ne $expected.Count){Stop-S53RootCopy 'proof_tree_changed'}
}
function Get-S53RootOutputFields([byte[]]$Raw,[string[]]$Names) {
    if ($Raw.Length -gt 131072) { Stop-S53RootCopy 'cli_output_cap' }
    try { $text=[Text.UTF8Encoding]::new($false,$true).GetString($Raw) } catch { Stop-S53RootCopy 'cli_output_shape' }
    $text=[regex]::Replace($text,'\r?\n\z','')
    $fields=[Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach ($line in [regex]::Split($text,'\r?\n')) {
        if ($line -cnotmatch '\A([a-z_]+)=([^\r\n\x00]+)\z' -or
            $Matches[1] -cnotin $Names -or -not $fields.TryAdd($Matches[1],$Matches[2])) { Stop-S53RootCopy 'cli_output_fields' }
    }
    if ($fields.Count -ne $Names.Count) { Stop-S53RootCopy 'cli_output_fields' }
    return ,$fields
}
function Assert-S53RootShow([byte[]]$Raw,[string]$Account) {
    $f=Get-S53RootOutputFields $Raw @('account_id','root_key_protection','root_key_load','status')
    if ($f['account_id'] -cne $Account -or $f['status'] -cne 'account-loaded' -or
        $f['root_key_protection'] -cne 'windows-dpapi-current-user' -or
        $f['root_key_load'] -cnotin @('already-current','legacy-migrated')) { Stop-S53RootCopy 'cli_show_contract' }
    return [ordered]@{account=$Account;protection=$f['root_key_protection'];load=$f['root_key_load']}
}
function Assert-S53RootSnapshotOutput([byte[]]$Raw,[byte[]]$File,[string]$Path,[string]$Account,[ulong]$Revision,[int]$Revocations) {
    $f=Get-S53RootOutputFields $Raw @('account_id','authority_revision','revocation_count','snapshot_file','snapshot','status')
    [ulong]$revisionValue=0; [ulong]$countValue=0
    if ($f['account_id'] -cne $Account -or $f['status'] -cne 'account-snapshot-exported' -or
        $f['snapshot_file'] -cne $Path -or $File.Length -eq 0 -or $File.Length -gt 16384 -or
        $f['authority_revision'] -cnotmatch '\A(?:0|[1-9][0-9]{0,19})\z' -or
        $f['revocation_count'] -cnotmatch '\A(?:0|[1-9][0-9]{0,19})\z' -or
        -not [ulong]::TryParse($f['authority_revision'],[ref]$revisionValue) -or
        -not [ulong]::TryParse($f['revocation_count'],[ref]$countValue) -or
        $revisionValue -ne $Revision -or $countValue -ne $Revocations -or
        $f['snapshot'] -cnotmatch '\A[A-Za-z0-9_-]+\z') { Stop-S53RootCopy 'cli_snapshot_contract' }
    $encoded=$f['snapshot']; $canonical=[Convert]::ToBase64String($File).TrimEnd('=').Replace('+','-').Replace('/','_')
    if ($encoded -cne $canonical) { Stop-S53RootCopy 'snapshot_file_equality' }
    # Deliberately no PowerShell signature-verifier claim. The pinned CLI encode verifies;
    # a later independently approved Cold phase must decode/verify again and compare S floors.
    return [ordered]@{account=$Account;revision=$revisionValue;revocations=$countValue;bytes=$File.Length;sha256=(Get-S53RootDigest $File);signing_proof='pinned-cli-encode-and-exact-file'}
}
