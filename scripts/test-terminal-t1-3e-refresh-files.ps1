# Narrow file-object operations for the two A harness scripts. Never sets security.
function Initialize-RefreshFileType {
    if('RefreshAFileObject' -as [type]){return}
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
public static class RefreshAFileObject {
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes,CreatedLow,CreatedHigh,AccessedLow,AccessedHigh,WrittenLow,WrittenHigh;
        public uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
    }
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
    [DllImport("kernel32.dll",SetLastError=true,CharSet=CharSet.Unicode)] static extern SafeFileHandle CreateFileW(string path,uint access,uint share,IntPtr security,uint disposition,uint attributes,IntPtr template);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetFileInformationByHandle(SafeFileHandle h,int kind,IntPtr info,uint bytes);
    public static string Key(SafeFileHandle h) {
        Info i;if(!GetFileInformationByHandle(h,out i))throw new Win32Exception();
        if(i.Links!=1)throw new InvalidOperationException("REFRESH_SINGLE_LINK_REQUIRED");
        return i.Volume.ToString("x8")+":"+i.IndexHigh.ToString("x8")+i.IndexLow.ToString("x8");
    }
    public static void Rename(string from,string to) { Rename(from,to,null); }
    public static void Rename(string from,string to,string expectedKey) {
        // FileRenameInfo, ReplaceIfExists=false. No copy, security merge or fallback.
        using(var handle=CreateFileW(from,0x30080,0,IntPtr.Zero,3,0,IntPtr.Zero)) {
            if(handle.IsInvalid)throw new Win32Exception();
            if(expectedKey!=null && Key(handle)!=expectedKey)throw new InvalidOperationException("REFRESH_RENAME_HANDLE_DRIFT");
            byte[] name=Encoding.Unicode.GetBytes(to);
            int offset=IntPtr.Size==8?20:12, length=offset+name.Length+2;
            IntPtr buffer=Marshal.AllocHGlobal(length);
            try {
                Marshal.Copy(new byte[length],0,buffer,length);
                Marshal.WriteInt32(buffer,offset-4,name.Length);
                Marshal.Copy(name,0,IntPtr.Add(buffer,offset),name.Length);
                if(!SetFileInformationByHandle(handle,3,buffer,(uint)length))throw new Win32Exception();
            }finally{Marshal.FreeHGlobal(buffer);}
        }
    }
}
'@
}
function Get-RefreshStreamObject($Stream) {
    Initialize-RefreshFileType
    return @{key=[RefreshAFileObject]::Key($Stream.SafeFileHandle);
        sddl=$Stream.GetAccessControl().GetSecurityDescriptorSddlForm('All')}
}
function Get-RefreshObject([string]$Path) {
    Lab-Plain $Path
    $f=[IO.File]::Open($Path,'Open','Read',('ReadWrite, Delete'))
    try{return Get-RefreshStreamObject $f}finally{$f.Dispose()}
}
function Assert-RefreshObject($Actual,$Expected) {
    if($Actual.key -cne $Expected.key -or $Actual.sddl -cne $Expected.sddl){throw 'REFRESH_FILE_OBJECT_DRIFT'}
    $sd=[Security.AccessControl.RawSecurityDescriptor]::new($Expected.sddl)
    if(!( ([int]$sd.ControlFlags) -band 0x1400 )){
        foreach($ace in $sd.DiscretionaryAcl){if(([int]$ace.AceFlags) -band 16){throw 'REFRESH_LEGACY_INHERITANCE_UNSUPPORTED'}}
    }
}
function Assert-RefreshObjects([string]$Directory,$Pins) {
    foreach($name in $Pins.Keys){Assert-RefreshObject (Get-RefreshObject (Join-Path $Directory $name)) $Pins[$name]}
}
function Move-RefreshObject($Context,$Io,[string]$From,[string]$To,$Expected) {
    Assert-RefreshWork $Context
    Lab-Plain ([IO.Path]::GetDirectoryName($To))
    Assert-RefreshObject (Get-RefreshObject $From) $Expected
    if(Test-Path -LiteralPath $To){throw 'REFRESH_RENAME_DEST_OCCUPIED'}
    Refresh-Step $Context 'rename-intent' @{from=$From;to=$To;object=$Expected}
    # The exact fixed paths have plain ancestors. The native call forbids cross-volume copy.
    if($Io.ContainsKey('move')){& $Io.move $From $To}else{[RefreshAFileObject]::Rename($From,$To,$Expected.key)}
    Assert-RefreshObject (Get-RefreshObject $To) $Expected
    Refresh-Step $Context 'renamed' @{from=$From;to=$To;object=$Expected}
}
function Get-RefreshStreamHash($Stream) {
    $Stream.Position=0;$hash=[Security.Cryptography.SHA256]::Create()
    try{return ([BitConverter]::ToString($hash.ComputeHash($Stream))).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
}
function Write-RefreshObject($Context,$Io,[string]$Name,[string]$Path,[string]$Source,[string]$ExpectedHash,$Object,[string]$DesiredHash) {
    Assert-RefreshWork $Context
    $bytes=[IO.File]::ReadAllBytes($Source)
    if($bytes.Length -gt 1048576){throw 'REFRESH_SCRIPT_CAP'}
    if(!$DesiredHash){$DesiredHash=Lab-Hash $Source}
    $sha=[Security.Cryptography.SHA256]::Create()
    try{$sourceHash=([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
    if($sourceHash -cne $DesiredHash){throw 'REFRESH_WRITE_SOURCE_PIN'}
    $stream=[IO.File]::Open($Path,'Open','ReadWrite','None')
    try {
        Assert-RefreshObject (Get-RefreshStreamObject $stream) $Object
        if($stream.Length -gt 1048576){throw 'REFRESH_SCRIPT_CAP'}
        if((Get-RefreshStreamHash $stream) -cne $ExpectedHash){throw 'REFRESH_WORK_CONTENT_DRIFT'}
        $prior=[byte[]]::new([int]$stream.Length);$stream.Position=0
        $offset=0;while($offset -lt $prior.Length){$n=$stream.Read($prior,$offset,$prior.Length-$offset);if(!$n){throw 'REFRESH_READ_SHORT'};$offset+=$n}
        Refresh-Step $Context 'write-intent' @{name=$Name;before=$ExpectedHash;source_sha256=(Lab-Hash $Source);object=$Object}
        try {
            $stream.Position=0
            if($Io.ContainsKey('write')){& $Io.write $stream $bytes}else{$stream.Write($bytes,0,$bytes.Length);$stream.SetLength($bytes.Length);$stream.Flush($true)}
            if((Get-RefreshStreamHash $stream) -cne $DesiredHash){throw 'REFRESH_WRITE_HASH'}
            Assert-RefreshObject (Get-RefreshStreamObject $stream) $Object
        }catch{
            $failure=$_
            # Same exclusive handle: no foreign writer can race this bounded rollback.
            try{$stream.Position=0;$stream.Write($prior,0,$prior.Length);$stream.SetLength($prior.Length);$stream.Flush($true)
                if((Get-RefreshStreamHash $stream) -cne $ExpectedHash){throw 'REFRESH_WRITE_UNDO_HASH'}
                Assert-RefreshObject (Get-RefreshStreamObject $stream) $Object
            }catch{
                $Context.knownPartial[$Name]=Get-RefreshStreamHash $stream
                Refresh-Step $Context 'write-undo-unconfirmed' @{name=$Name;hash=$Context.knownPartial[$Name];object=$Object}
                throw 'REFRESH_WRITE_UNDO_UNCONFIRMED'
            }
            throw $failure
        }
    }finally{$stream.Dispose()}
}
function Get-RefreshWorkingPath($Context,[string]$Name) {return Join-Path $Context.operation ('working/'+$Name)}
function Assert-RefreshFileLocks($Context) {
    foreach($key in @('agent','images')){
        if(!$Context.handles.ContainsKey($key) -or !$Context.handles[$key].CanWrite -or
           $Context.handles[$key].Name -ine (Join-Path $Context.directory ('control/'+$key+'.lock'))){throw 'REFRESH_RECONCILE_LOCKS_REQUIRED'}
    }
}
function Find-RefreshOriginal($Context,[string]$Name) {
    $paths=@((Join-Path $Context.directory $Name),(Get-RefreshWorkingPath $Context $Name))
    if($Context.recovery){$paths+=,$Context.originalPaths[$Name]}
    $found=@()
    foreach($path in $paths){if([IO.File]::Exists($path)){
        $actual=Get-RefreshObject $path
        if($actual.key -ceq $Context.objects[$Name].key){Assert-RefreshObject $actual $Context.objects[$Name];$found+=,$path}
    }}
    if($found.Count -ne 1){throw 'REFRESH_ORIGINAL_LOCATION_UNKNOWN'}
    return $found[0]
}
function Start-RefreshObjectUpdate($Context,$Io) {
    Assert-RefreshFileLocks $Context
    foreach($name in $RefreshNames){
        $origin=if($Context.recovery){$Context.originalPaths[$name]}else{Join-Path $Context.directory $name}
        Assert-RefreshObject (Get-RefreshObject $origin) $Context.objects[$name]
        $expected=if($Context.recovery){$Context.maps.old[$name]}else{$Context.from[$name]}
        if((Lab-Hash $origin) -cne $expected){throw 'REFRESH_ORIGINAL_CONTENT_DRIFT'}
        [IO.File]::Copy($origin,(Join-Path $Context.operation ('origin-bytes/'+$name)),$false)
    }
    # Removing agent first prevents Task from loading a partial/mixed update.
    foreach($name in $RefreshNames){
        $Context.attempted+=,$name
        $destination=if($Context.recovery){Join-Path $Context.operation ('quarantine/'+$name)}else{Get-RefreshWorkingPath $Context $name}
        Move-RefreshObject $Context $Io (Join-Path $Context.directory $name) $destination $Context.initialObjects[$name]
    }
    if($Context.recovery){foreach($name in $RefreshNames){
        Move-RefreshObject $Context $Io $Context.originalPaths[$name] (Get-RefreshWorkingPath $Context $name) $Context.objects[$name]
    }}
    foreach($name in $RefreshNames){
        $from=if($Context.recovery){$Context.maps.old[$name]}else{$Context.from[$name]}
        Write-RefreshObject $Context $Io $name (Get-RefreshWorkingPath $Context $name) (Join-Path $Context.operation ('stage/'+$name)) $from $Context.objects[$name] $Context.to[$name]
    }
    # Common first, complete agent last. No ReplaceFile or security setters.
    foreach($name in @($RefreshNames[1],$RefreshNames[0])){
        Move-RefreshObject $Context $Io (Get-RefreshWorkingPath $Context $name) (Join-Path $Context.directory $name) $Context.objects[$name]
    }
}
function Undo-RefreshObjectUpdate($Context,$Io) {
    Assert-RefreshFileLocks $Context
    # Reconcile actual IDs, including an exception AFTER a native rename succeeded.
    foreach($name in $RefreshNames){
        $location=Find-RefreshOriginal $Context $name
        if($location -ieq (Join-Path $Context.directory $name)){
            Move-RefreshObject $Context $Io $location (Get-RefreshWorkingPath $Context $name) $Context.objects[$name]
        }
    }
    foreach($name in $RefreshNames){
        $location=Find-RefreshOriginal $Context $name
        if($Context.recovery -and $location -ieq $Context.originalPaths[$name]){continue}
        $current=Lab-Hash $location
        $restore=if($Context.recovery){$Context.maps.old[$name]}else{$Context.from[$name]}
        $known=@($restore,$Context.to[$name]);if($Context.knownPartial.ContainsKey($name)){$known+=,$Context.knownPartial[$name]}
        if($current -cnotin $known){throw 'REFRESH_FOREIGN_CONTENT_PRESERVED'}
        if($current -cne $restore){
            Write-RefreshObject $Context @{} $name $location (Join-Path $Context.operation ('origin-bytes/'+$name)) $current $Context.objects[$name] $restore
        }
        if($Context.recovery){Move-RefreshObject $Context $Io $location $Context.originalPaths[$name] $Context.objects[$name]}
    }
    foreach($name in @($RefreshNames[1],$RefreshNames[0])){
        $destination=Join-Path $Context.directory $name
        if($Context.recovery){
            $source=Join-Path $Context.operation ('quarantine/'+$name)
            if(![IO.File]::Exists($source)){Assert-RefreshObject (Get-RefreshObject $destination) $Context.initialObjects[$name];continue}
        }else{$source=Get-RefreshWorkingPath $Context $name}
        Move-RefreshObject $Context $Io $source $destination $Context.initialObjects[$name]
    }
    Assert-RefreshFiles $Context.directory $Context.from
    Assert-RefreshObjects $Context.directory $Context.initialObjects
}
