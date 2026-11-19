# Fixed installer diagnostics. No policy/task/account actions or arbitrary elevated reader.
function Lab-BoundedText($Value,[int]$Limit=2048){
    $text=[string]$Value
    if($text.Length -gt $Limit){return $text.Substring(0,$Limit)+' [truncated]'}
    return $text
}
function Get-LabFailureSummary($Record,[string]$Stage){
    @{stage=(Lab-BoundedText $Stage 80);type=$Record.Exception.GetType().FullName;message=(Lab-BoundedText $Record.Exception.Message);
        hresult=$Record.Exception.HResult;error_id=(Lab-BoundedText $Record.FullyQualifiedErrorId 256)}
}
function Set-LabStage([string]$Stage){$script:LabStage=$Stage}
function Get-LabStage {
    $v=Get-Variable -Scope Script -Name LabStage -ErrorAction SilentlyContinue
    if($v){return [string]$v.Value};return 'transaction'
}
function Select-LabFailure($Value){
    if(!$Value){return $null}
    $selected=[ordered]@{}
    foreach($name in @('stage','type','message','hresult','error_id')){
        if($Value -is [Collections.IDictionary]){
            if($Value.Contains($name)){$selected[$name]=Lab-BoundedText $Value[$name]}
        }elseif($Value.PSObject.Properties[$name]){$selected[$name]=Lab-BoundedText $Value.$name}
    }
    return $selected
}
function Get-LabLogSample([string]$Path) {
    $f=$null
    try {
        Lab-Plain $Path
        $f=[IO.File]::Open($Path,'Open','Read','Read')
        $length=$f.Length;$fullHash=$null
        if($length -le 2097152){
            $hash=[Security.Cryptography.SHA256]::Create()
            try{$fullHash=[BitConverter]::ToString($hash.ComputeHash($f)).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
        }
        $chunks=@();$regions=if($length -le 65536){@(@{offset=[long]0;count=[int]$length})}else{@(@{offset=[long]0;count=32768},@{offset=($length-32768);count=32768})}
        foreach($region in $regions){
            $f.Position=$region.offset;$bytes=[byte[]]::new($region.count);$used=0
            while($used -lt $bytes.Length){$got=$f.Read($bytes,$used,$bytes.Length-$used);if(!$got){throw 'Diagnostic short read'};$used+=$got}
            $hash=[Security.Cryptography.SHA256]::Create()
            try{$pin=[BitConverter]::ToString($hash.ComputeHash($bytes)).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
            $chunks+=@{offset=$region.offset;bytes=$bytes.Length;sha256=$pin;data_base64=[Convert]::ToBase64String($bytes)}
        }
        return @{path=$Path;available=$true;source_bytes=$length;exported_bytes=[Math]::Min($length,65536);truncated=($length -gt 65536);
            full_hash_complete=($null -ne $fullHash);source_sha256=$fullHash;snapshot_complete=$true;chunks=$chunks}
    }catch{return @{path=$Path;available=$false;error=(Lab-BoundedText $_.Exception.Message);snapshot_complete=$false}}
    finally{if($f){$f.Dispose()}}
}
function Get-LabMinimalJournal([string]$Path,[string]$ExpectedSession){
    try {
        $j=Lab-Read $Path -MaxBytes 2097152
        if(!$j){return @{available=$false;reason='absent'}}
        $phase=[string]$j.phase
        if($phase -cnotin @('installing','installed','rollback_incomplete','rolled_back','recovering','removing','removed')){throw 'Unknown journal phase'}
        $owner=if($j.PSObject.Properties['setup_session']){[string]$j.setup_session}else{''}
        $errors=@();$total=0
        if($j.PSObject.Properties['rollback_errors']){foreach($e in @($j.rollback_errors)){$total++;if($errors.Count -lt 16){$errors+=,(Lab-BoundedText $e)}}}
        $primary=if($j.PSObject.Properties['primary_failure']){Select-LabFailure $j.primary_failure}else{$null}
        return @{available=$true;phase=$phase;belongs_to_invocation=($owner -ceq $ExpectedSession);setup_session=(Lab-BoundedText $owner 80);
            primary=$primary;rollback_errors=$errors;rollback_error_count=$total;rollback_errors_truncated=($total -gt 16)}
    }catch{return @{available=$false;reason='unreadable_or_unsupported';error=(Lab-BoundedText $_.Exception.Message)}}
}
function Get-LabSetupDiagnostics([string]$ProtectedPackage,[string]$JournalPath,[string]$Session) {
    $logs=[ordered]@{};foreach($name in @('main.stdout.log','main.stderr.log','recovery.stdout.log','recovery.stderr.log')){
        $logs[$name]=Get-LabLogSample (Join-Path $ProtectedPackage $name)
    }
    $failures=[ordered]@{}
    foreach($name in @('main-error.json','recovery-error.json')){
        try{
            $e=Lab-Read (Join-Path $ProtectedPackage $name)
            $failures[$name]=if($e){@{primary=(Select-LabFailure $e.primary);terminal=(Select-LabFailure $e.terminal)}}else{$null}
        }catch{$failures[$name]=@{unavailable=$true;error=(Lab-BoundedText $_.Exception.Message)}}
    }
    @{logs=$logs;failures=$failures;journal=(Get-LabMinimalJournal $JournalPath $Session);
        bounds=@{per_log_export_bytes=65536;per_log_full_hash_bytes=2097152;journal_read_bytes=2097152};
        scope='fixed main/recovery logs and selected status only; no initial policy, Audit or credentials'}
}
function Publish-LabDiagnosticResult([string]$Path,$Value,[string]$Sid){
    # Caller supplies a fixed, validated admin/System result directory. Only this new
    # public file gets the intended ordinary SID's read permission, never write/execute.
    Lab-Write $Path $Value -MaxBytes 2097152
    $acl=Get-Acl -LiteralPath $Path
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($Sid),'Read','Allow'))
    Set-Acl -LiteralPath $Path -AclObject $acl
}
function Get-LabExportScope([ValidateSet('A','B')][string]$Which){
    if($Which -ceq 'A'){
        return @{side='A';host='DESKTOP-PAUL-RY';sid='S-1-5-21-1776862868-2140151431-2099235586-1001';
            base='E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f';session='setup-e4125cddbe3f4dba9d3017aec19af6cd';
            manifest='5e7b721ccd38192e2dfbb7e674446ab84cb3e8c74de4ffbcb3fc7ab6ae8a39f4'}
    }
    return @{side='B';host='PAUL_HP_OMEN';sid='S-1-5-21-2671144489-3236106012-1032288181-1006';
        base='C:\Users\test\KilogramLab\terminal-t11f';session='setup-3b96084a7c5a49889a0d9fcb61fb1e1e';
        manifest='ce75a4ac647d6886e5d7ad8c2424051691fedbc0c11c45d23a4bf64cfd3ecdda'}
}
