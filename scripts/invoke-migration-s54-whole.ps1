[CmdletBinding()]
param(
    [ValidateSet('Unavailable','Prepare','Run')][string]$Action='Unavailable',
    [string]$InventorySha256,
    [string]$ActivationSha256,
    [string]$PreparationRunId,
    [ValidateSet('WholeP','FeasibilityF')][string]$Purpose='WholeP'
)
# Dot-source defines policy/outer functions only. Actions are explicit.
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'cargo-frozen-artifacts.ps1')
function Get-S54Sha([byte[]]$Bytes) { [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant() }
function Get-S54Path([string]$Path,[switch]$AllowMissing) {
    if(-not [IO.Path]::IsPathRooted($Path) -or $Path.Length -gt 1024 -or $Path -match '(^|[\\/])\.\.?([\\/]|$)'){throw 'S54 absolute path/components.'}
    $full=[IO.Path]::GetFullPath($Path);$probe=$full
    while(-not (Test-Path -LiteralPath $probe)) {
        if(-not $AllowMissing){throw 'S54 missing public path.'}
        $probe=Split-Path -Parent $probe;if(-not $probe){throw 'S54 missing ancestor.'}
    }
    if($probe -cne $full -and -not (Get-Item -LiteralPath $probe -Force).PSIsContainer){throw 'S54 missing suffix requires a directory.'}
    if($probe -cmatch '^[A-Za-z]:\\'){
        # Fresh leaf-to-root observations, without provider cmdlet overhead.
        for($p=$probe;$p;$p=[IO.Path]::GetDirectoryName([IO.Path]::TrimEndingDirectorySeparator($p))){
            if([IO.File]::GetAttributes($p) -band [IO.FileAttributes]::ReparsePoint){throw 'S54 reparse ancestor.'}
        }
    }else{
        # UNC/device parent semantics differ: retain the complete provider walk.
        for($p=$probe;$p;$p=Split-Path -Parent $p){
            if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'S54 reparse ancestor.'}
        }
    }
    return $full
}
function Assert-S54SamePath([string]$Actual,[string]$Expected) {
    if(-not [string]::Equals((Get-S54Path $Actual -AllowMissing),(Get-S54Path $Expected -AllowMissing),[StringComparison]::OrdinalIgnoreCase)){throw 'S54 path role mismatch.'}
}
function Read-S54Public([string]$Path,[long]$Cap) {
    $path=Get-S54Path $Path;$before=Get-Item -LiteralPath $path -Force
    if($before.PSIsContainer -or $before.Length -gt $Cap){throw 'S54 public file bound.'}
    $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try {
        if($stream.Length -ne $before.Length){throw 'S54 read-time file identity.'}
        $memory=[IO.MemoryStream]::new();$buffer=[byte[]]::new(8192)
        try {while(($n=$stream.Read($buffer,0,[int][Math]::Min($buffer.Length,$Cap+1-$memory.Length))) -gt 0){$memory.Write($buffer,0,$n);if($memory.Length -gt $Cap){throw 'S54 actual public read bound.'}};$raw=$memory.ToArray()}
        finally {$memory.Dispose()}
        $after=Get-Item -LiteralPath $path -Force
        $null=Get-S54Path $path
        if($after.Length -ne $before.Length -or $after.CreationTimeUtc -ne $before.CreationTimeUtc -or $after.LastWriteTimeUtc -ne $before.LastWriteTimeUtc -or $raw.Length -ne $before.Length -or $stream.Length -ne $before.Length){throw 'S54 post-read file identity.'}
        return ,$raw
    } finally {$stream.Dispose()}
}
function Get-S54Pin([string]$Path,[long]$Cap=134217728) {$raw=Read-S54Public $Path $Cap;[ordered]@{path=(Get-S54Path $Path);bytes=[long]$raw.Length;sha256=(Get-S54Sha $raw)}}
function Read-S54Pin($Pin,[long]$Cap) {
    Assert-S54Wire $Pin 'Pin'
    if($Pin.sha256 -cnotmatch '^[0-9a-f]{64}$'){throw 'S54 lower hex digest.'}
    $raw=Read-S54Public $Pin.path $Cap
    if($raw.Length -ne $Pin.bytes -or (Get-S54Sha $raw) -cne $Pin.sha256){throw 'S54 public pin mismatch.'};return ,$raw
}
function ConvertFrom-S54Strict([byte[]]$Bytes,[long]$Cap) {
    if($Bytes.Length -gt $Cap -or ($Bytes.Length -ge 3 -and $Bytes[0] -eq 239 -and $Bytes[1] -eq 187 -and $Bytes[2] -eq 191)){throw 'S54 JSON bound/BOM.'}
    $utf8=[Text.UTF8Encoding]::new($false,$true);$text=$utf8.GetString($Bytes)
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=128
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try {
        function Test-Keys($Element) {
            if($Element.ValueKind -eq [Text.Json.JsonValueKind]::Object){
                $keys=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
                foreach($p in $Element.EnumerateObject()){if(-not $keys.Add($p.Name)){throw 'S54 duplicate nested JSON key.'};Test-Keys $p.Value}
            }elseif($Element.ValueKind -eq [Text.Json.JsonValueKind]::Array){foreach($v in $Element.EnumerateArray()){Test-Keys $v}}
        }
        Test-Keys $doc.RootElement
        return ,(ConvertFrom-Json -InputObject $text -AsHashtable -Depth 128)
    }finally{$doc.Dispose()}
}
function Read-S54Json($Pin,[long]$Cap) {return ,(ConvertFrom-S54Strict (Read-S54Pin $Pin $Cap) $Cap)}
function Get-S54SourceHead([string]$Root) {
    $ErrorActionPreference='Stop'
    $git=Get-S54Path (Join-Path $Root '.git')
    if(-not (Get-Item -LiteralPath $git -Force).PSIsContainer){throw 'S54 Git directory required.'}
    $utf8=[Text.UTF8Encoding]::new($false,$true)
    $value=$utf8.GetString((Read-S54Public (Join-Path $git 'HEAD') 4096)).Trim()
    if($value.StartsWith('ref: ',[StringComparison]::Ordinal)){
        $reference=$value.Substring(5)
        if(-not $reference.StartsWith('refs/heads/',[StringComparison]::Ordinal) -or $reference.Length -le 11 -or $reference.EndsWith('/') -or $reference.Contains('//') -or $reference.Contains('..') -or $reference -match '[:\\]'){throw 'S54 HEAD ref.'}
        $loose=Get-S54Path (Join-Path $git $reference) -AllowMissing
        if(-not $loose.StartsWith($git+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'S54 HEAD ref confinement.'}
        $absent={param([string]$Path)
            try{$attributes=[IO.File]::GetAttributes($Path)}
            catch [IO.FileNotFoundException]{return $true}
            catch [IO.DirectoryNotFoundException]{return $true}
            if($attributes -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)){throw 'S54 loose HEAD must be a nonreparse file.'}
            return $false
        }
        if(-not (& $absent $loose)){
            $value=$utf8.GetString((Read-S54Public $loose 4096)).Trim()
        }else{
            # Only precise not-found results authorize packed lookup. Other IO
            # failures, including denied attributes/reads, remain fatal.
            $null=Get-S54Path $loose -AllowMissing
            $packed=$utf8.GetString((Read-S54Public (Join-Path $git 'packed-refs') 1048576))
            if(-not (& $absent $loose)){throw 'S54 loose HEAD appeared during packed read.'}
            $null=Get-S54Path $loose -AllowMissing
            $records=@(foreach($line in ($packed -split '\r?\n')){
                if($line.StartsWith('#',[StringComparison]::Ordinal) -or $line.StartsWith('^',[StringComparison]::Ordinal)){continue}
                $split=$line.IndexOf(' ')
                if($split -ge 0 -and [string]::Equals($line.Substring($split+1),$reference,[StringComparison]::Ordinal)){$line.Substring(0,$split)}
            })
            # Count before validating digests: malformed duplicates still count.
            if($records.Count -ne 1){throw 'S54 exact packed source HEAD.'}
            $value=$records[0]
        }
    }
    if($value -cnotmatch '^[0-9a-f]{40}$'){throw 'S54 source HEAD digest.'}
    return $value
}
function Assert-S54Wire($Value,[string]$Type) {
    $shape=$script:S54Schema[$Type]
    if($null -eq $shape -or $Value -isnot [Collections.IDictionary] -or $Value.Count -ne $shape.Count){throw "S54 closed object $Type."}
    foreach($k in $Value.Keys){if(-not @($shape.Keys|Where-Object {$_ -ceq $k}).Count){throw "S54 unknown field $Type."}}
    foreach($k in $shape.Keys){
        $d=$shape[$k];$v=$Value[$k]
        if($d.StartsWith('n')){if($null -eq $v){continue};$d=$d.Substring(1)}
        if($d.StartsWith('a')){
            if($v -isnot [System.Collections.IList] -or $v -is [string] -or $v.Count -gt 4096){throw "S54 array $Type/$k."}
            $item=$d.Substring(1);foreach($e in $v){Assert-S54Value $e $item};continue
        }
        Assert-S54Value $v $d
    }
}
function Assert-S54Value($Value,[string]$Type) {
    switch -CaseSensitive ($Type) {
        's' {if($Value -isnot [string] -or $Value.Length -gt 1024){throw 'S54 string type/bound.'};return}
        'raw' {if($Value -isnot [string] -or [Text.Encoding]::UTF8.GetByteCount($Value) -gt 131072){throw 'S54 raw compiler line.'};return}
        'p' {if($Value -isnot [string]){throw 'S54 path type.'};$null=Get-S54Path $Value -AllowMissing;return}
        'u' {if($Value -is [bool] -or $null -eq $Value -or $Value -isnot [ValueType] -or $Value.GetType().Name -notin @('Int32','Int64','UInt32','UInt64','BigInteger') -or $Value -lt 0 -or $Value -gt [UInt64]::MaxValue){throw 'S54 unsigned integer type.'};return}
        'b' {if($Value -isnot [bool]){throw 'S54 boolean type.'};return}
        'j' {return}
        default {Assert-S54Wire $Value $Type}
    }
}
function ConvertTo-S54Canonical($Value) {
    if($null -eq $Value){return $null}
    if($Value -is [Collections.IDictionary]){$out=[ordered]@{};foreach($k in @($Value.Keys|Sort-Object -CaseSensitive)){$out[$k]=ConvertTo-S54Canonical $Value[$k]};return $out}
    if($Value -is [System.Collections.IList] -and $Value -isnot [string]){return ,@($Value|ForEach-Object {ConvertTo-S54Canonical $_})}
    return $Value
}
function Assert-S54Equal($Actual,$Expected) {
    if((ConvertTo-KilogramFrozenJson (ConvertTo-S54Canonical $Actual)) -cne (ConvertTo-KilogramFrozenJson (ConvertTo-S54Canonical $Expected))){throw 'S54 exact structured identity.'}
}
function Assert-S54PinEqual($Actual,$Expected) {
    Assert-S54Wire $Actual 'Pin';Assert-S54Wire $Expected 'Pin'
    Assert-S54SamePath $Actual.path $Expected.path
    if($Actual.bytes -ne $Expected.bytes -or $Actual.sha256 -cne $Expected.sha256){throw 'S54 exact bound pin bytes/hash.'}
}
function Assert-S54Environment($Actual,$Expected) {
    $maps=@();foreach($rows in @($Actual,$Expected)){
        $map=@{};foreach($r in $rows){if($map.ContainsKey($r.name)){throw 'S54 duplicate environment role.'};$map[$r.name]=$r}
        $maps+=,$map
    }
    Assert-S54Equal $maps[0] $maps[1]
}
function Get-S54SourcePaths([string]$Root) {
    $out=[Collections.Generic.List[string]]::new();$todo=[Collections.Generic.Stack[string]]::new();$entries=0
    foreach($n in @('apps','crates')){$todo.Push((Get-S54Path (Join-Path $Root $n)))}
    while($todo.Count){foreach($e in Get-ChildItem -LiteralPath $todo.Pop() -Force){
        $entries++;if($entries -gt 32768){throw 'S54 source enumeration bound.'}
        $null=Get-S54Path $e.FullName
        if($e.PSIsContainer){if($e.Name -cnotin @('target','.tmp','.git')){$todo.Push($e.FullName)}}
        elseif($e.Name -ceq 'Cargo.toml' -or $e.Extension -cin @('.rs','.c','.h','.cpp')){$out.Add($e.FullName)}
    }}
    foreach($n in @('Cargo.toml','Cargo.lock','rust-toolchain','rust-toolchain.toml')){if(Test-Path -LiteralPath (Join-Path $Root $n)){$out.Add((Get-S54Path (Join-Path $Root $n)))}}
    foreach($n in $script:S54Helpers){$out.Add((Get-S54Path (Join-Path $Root $n)))}
    foreach($n in $script:S54Embedded){$p=Get-S54Path (Join-Path $Root $n);if(-not $out.Contains($p)){$out.Add($p)}}
    if($out.Count -gt 4096){throw 'S54 source count bound.'};$out.Sort([StringComparer]::OrdinalIgnoreCase)
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($p in $out){if(-not $seen.Add($p)){throw 'S54 source alias.'}}
    return ,$out.ToArray()
}
function Test-S54Relevant([string]$Path) {
    return (($Path -cmatch '^(apps|crates)/' -and ($Path -cmatch '\.(rs|c|h|cpp)$' -or $Path.EndsWith('/Cargo.toml'))) -or $Path -cin @('Cargo.toml','Cargo.lock','rust-toolchain','rust-toolchain.toml') -or $Path -cin $script:S54Helpers -or $Path -cin $script:S54Embedded)
}
function Assert-S54Strings($Rows,[int]$Cap) {
    if($Rows.Count -gt $Cap){throw 'S54 bounded compiler strings.'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($s in $Rows){if(-not $s -or [Text.Encoding]::UTF8.GetByteCount($s) -gt 1024 -or -not $seen.Add($s)){throw 'S54 bounded unique compiler string.'}}
}
function Get-S54EmbeddedSnapshot([string]$Root) {
    # Exact public compile inputs only; no archive enumeration or execution.
    return ,@($script:S54Embedded|ForEach-Object {Get-S54Pin (Join-Path $Root $_) 8388608})
}
function Get-S54DepInfoPins($Capture,[bool]$Normal) {
    $roles=@(if($Normal){$script:S54Images[1]}else{$script:S54Images[3..8]})
    return ,@($roles|ForEach-Object {
        $role=$_;$u=@($Capture.units|Where-Object {$_.package -ceq $role.package -and $_.target -ceq $role.target -and (@($_.kind)-join ',') -ceq $role.kind -and $_.profile.test -ne $Normal -and $_.executable})
        if($u.Count -ne 1){throw 'S54 selected compiler dep-info role.'}
        Get-S54Pin ([IO.Path]::ChangeExtension($u[0].executable,'d')) 1048576
    })
}
function Assert-S54EmbeddedDependencies([string]$Root,$DepInfo,$SourceFiles) {
    $source=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($s in $SourceFiles){$null=$source.Add((Get-S54Path $s.path))}
    $found=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($pin in $DepInfo){
        $raw=Read-S54Pin $pin 1048576
        if($raw.Length -ge 3 -and $raw[0] -eq 239 -and $raw[1] -eq 187 -and $raw[2] -eq 191){throw 'S54 dep-info BOM.'}
        $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
        $first=($text -split '\r?\n')[0];$split=$first.IndexOf(': ')
        if($split -lt 1){throw 'S54 dep-info dependency rule.'}
        Assert-S54SamePath ($first.Substring(0,$split).Replace('\ ',' ')) $pin.path
        $names=[regex]::Split($first.Substring($split+2),'(?<!\\) ')
        if($names.Count -gt 4096){throw 'S54 dep-info dependency bound.'}
        foreach($n in $names){
            if(-not $n){throw 'S54 dep-info empty dependency.'}
            # rustc include! concat paths contain ../; normalize the compiler
            # dependency spelling, then use the ordinary nonreparse policy.
            $dependency=$n.Replace('\ ',' ')
            $path=Get-S54Path ([IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($dependency)){$dependency}else{Join-Path $Root $dependency})))
            if(-not $path.StartsWith($Root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or -not $source.Contains($path)){throw 'S54 unadmitted compiler dependency input.'}
            foreach($s in $script:S54Embedded){if([string]::Equals($path,(Get-S54Path (Join-Path $Root $s)),[StringComparison]::OrdinalIgnoreCase)){$null=$found.Add($s)}}
        }
    }
    if($found.Count -ne $script:S54Embedded.Count){throw 'S54 embedded dep-info completeness.'}
}
function Assert-S54Embedded($I,$Policy,$Tests) {
    $e=$I.compiler.embedded
    if($e.before.Count -ne $script:S54Embedded.Count -or $e.after.Count -ne $script:S54Embedded.Count){throw 'S54 exact embedded before/after input set.'}
    foreach($rows in @($e.before,$e.after)){
        $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach($p in $rows){
            $path=Get-S54Path $p.path
            $role=@($script:S54Embedded|Where-Object {[string]::Equals($path,(Get-S54Path (Join-Path $Policy.Root $_)),[StringComparison]::OrdinalIgnoreCase)})
            if($role.Count -ne 1 -or -not $seen.Add($path)){throw 'S54 embedded input role/duplicate.'}
            $source=@($I.source.files|Where-Object {[string]::Equals((Get-S54Path $_.path),$path,[StringComparison]::OrdinalIgnoreCase)})
            if($source.Count -ne 1){throw 'S54 embedded admitted source.'}
            Assert-S54PinEqual $p $source[0];$null=Read-S54Pin $p 8388608
        }
    }
    $expected=Get-S54DepInfoPins $Tests $false
    if($e.dep_info.Count -ne $expected.Count){throw 'S54 selected compiler dep-info set.'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($p in $e.dep_info){
        $path=Get-S54Path $p.path
        $match=@($expected|Where-Object {[string]::Equals((Get-S54Path $_.path),$path,[StringComparison]::OrdinalIgnoreCase)})
        if($match.Count -ne 1 -or -not $seen.Add($path)){throw 'S54 exact compiler dep-info role.'}
        Assert-S54PinEqual $p $match[0]
    }
    Assert-S54EmbeddedDependencies $Policy.Root $e.dep_info $I.source.files
}
function Get-S54Purpose([ValidateSet('WholeP','FeasibilityF')][string]$Name='WholeP') {
    $f=$Name -ceq 'FeasibilityF'
    $case=if($f){'F-s54-mixed-hooks-feasibility-01'}else{'P-s54-whole-resumed-l1-01'}
    return @{Name=$Name;Case=$case;Attempt=($case+$(if($f){'-attempt-04'}else{'-attempt-01'}));
        Selector=$(if($f){'live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public'}else{'live_l1::handoff::final_p5::s54_continuous_whole_resumed_l1_p5_public'});
        Namespace=$(if($f){'migration-s54-feasibility'}else{'migration-s54-integrated'});
        PublicNamespace=$(if($f){'migration-s54-feasibility-public-evidence'}else{'migration-s54-public-evidence'});
        Origin=$(if($f){'f-origin'}else{'p5-origin'});
        InventoryClass=$(if($f){'bound_s54_feasibility_inventory'}else{'bound_s54_whole_inventory'});
        ActivationClass=$(if($f){'root_s54_feasibility_activation'}else{'root_s54_whole_activation'});
        Native=$(if($f){'S54_FEASIBILITY_FUNCTIONAL'}else{'S54_FUNCTIONAL'});
        Wrapper=$(if($f){'S54_FEASIBILITY_FROZEN_COMPLETED'}else{'S54_FROZEN_COMPLETED'});
        FunctionalClass=$(if($f){'feasibility_assertions_completed'}else{'functional_assertions_completed'});
        WrapperClass=$(if($f){'s54_feasibility_frozen_execution_completed'}else{'s54_frozen_execution_completed'});
        ObservationsClass=$(if($f){'F feasibility functional only'}else{'P11 functional phase only'});
        OuterClass=$(if($f){'s54_feasibility_outer_observations'}else{'s54_outer_observations'});
        Deadline=@{controller=$(if($f){450}else{2350});work=$(if($f){480}else{2375});outer=$(if($f){510}else{2400});origin='owned_launch';clock='windows_tick_count64'}}
}
function Set-S54Purpose([ValidateSet('WholeP','FeasibilityF')][string]$Name='WholeP') {
    $script:S54PolicyPurpose=Get-S54Purpose $Name
    $script:S54Case=$script:S54PolicyPurpose.Case;$script:S54Attempt=$script:S54PolicyPurpose.Attempt;$script:S54Selector=$script:S54PolicyPurpose.Selector
}
function Get-S54OutputPurpose($I) {
    # Public parser fixtures retain the historical P grammar. Production identity
    # admission is mandatory; an exact F identity alone selects the fixed F grammar.
    if($I.identity.case -ceq 'F-s54-mixed-hooks-feasibility-01') {
        $p=Get-S54Purpose FeasibilityF
        if($I.identity.attempt -cne $p.Attempt -or $I.identity.selector -cne $p.Selector){throw 'S54 cross-purpose output identity.'}
        return $p
    }
    return Get-S54Purpose WholeP
}
function New-S54Policy([string]$FixtureRoot) {
    $workspace=Split-Path -Parent $PSScriptRoot
    if($FixtureRoot){
        $root=Get-S54Path $FixtureRoot
        if(-not $root.StartsWith((Join-Path $workspace '.tmp/s54-block4b-r1-01/rust-public-fixtures/') ,[StringComparison]::OrdinalIgnoreCase)){throw 'S54 public fixture root only.'}
        $resolution=@('pwsh.exe','powershell.exe','git.exe','rustup.exe','cargo.exe','rustc.exe')|ForEach-Object {[ordered]@{name=$_;path=(Join-Path $root "tools/$_")}}
        return @{Root=$root;Scope='public_schema_fixture_only';Configs=@((Join-Path $root '.cargo/config'));Resolutions=@($resolution);
            Legacy=@{bytes=([Text.Encoding]::UTF8.GetByteCount('public fixture image legacy-cli'));sha256=(Get-S54Sha ([Text.Encoding]::UTF8.GetBytes('public fixture image legacy-cli')))};
            Relay=@{bytes=([Text.Encoding]::UTF8.GetByteCount('public fixture image relay'));sha256=(Get-S54Sha ([Text.Encoding]::UTF8.GetBytes('public fixture image relay')))}}
    }
    if(-not [Environment]::Is64BitProcess -or -not $IsWindows){throw 'S54 production admission requires x64 Windows.'}
    $configs=[Collections.Generic.List[string]]::new();for($d=[IO.DirectoryInfo]::new($workspace);$null -ne $d;$d=$d.Parent){foreach($n in @('config','config.toml')){$configs.Add((Join-Path $d.FullName ".cargo/$n"))}}
    $homeDir=if($env:CARGO_HOME){$env:CARGO_HOME}else{Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo'}
    foreach($n in @('config','config.toml')){$configs.Add((Join-Path $homeDir $n))}
    $resolutions=@('pwsh.exe','powershell.exe','git.exe','rustup.exe','cargo.exe','rustc.exe')|ForEach-Object {
        $c=Get-Command $_ -CommandType Application -ErrorAction Stop|Select-Object -First 1
        [ordered]@{name=$_;path=(Get-S54Path $c.Source)}
    }
    return @{Root=(Get-S54Path $workspace);Scope='slice_a_schema_inspection_only';Configs=@($configs|Sort-Object -Unique);Resolutions=@($resolutions);
        Legacy=@{bytes=61531648;sha256='d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1'};
        Relay=@{bytes=10460160;sha256='cf9d029cebf1d04745c90a9712401d70e823ecf4af5a082fe7269649df358e5e'}}
}
function New-S54CompilerSourceIndex($Sources) {
    # Membership names only; this call owns the index and every path is fresh.
    $index=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($p in $Sources){if(-not $index.Add((Get-S54Path $p.path))){throw 'S54 duplicate canonical compiler source.'}}
    return ,$index
}
function Assert-S54CompilerSourcesFresh($Sources) {
    # The index never authorizes bytes or a stale filesystem observation.
    foreach($p in $Sources){$null=Read-S54Pin $p 8388608}
}
function Assert-S54Compiler($I,$Policy,$Capture,[bool]$Normal) {
    Assert-S54Wire $Capture 'CompilerCapture'
    if($Capture.classification -cne $(if($Normal){'nonexecuting-normal-cli-compiler-provenance'}else{'nonexecuting-compiler-artifact-provenance'}) -or $Capture.profile -cne 'verification-scale' -or $Capture.target -cne 'x86_64-pc-windows-msvc'){throw 'S54 compiler role/profile.'}
    if($null -ne $Capture.expected_manifest_sha256){
        if($Normal -or $Capture.expected_manifest_sha256 -cnotmatch '^[0-9a-f]{64}$' -or $Capture.expected_manifest_sha256 -cne $I.compiler.frozen_manifest.sha256 -or -not $Capture.frozen_guard_verified){throw 'S54 frozen expected-manifest identity.'}
    }elseif($Capture.frozen_guard_verified){throw 'S54 no invented frozen guard.'}
    Assert-KilogramFrozenInputs $Capture.build_inputs_before $Capture.build_inputs_after
    $b=$Capture.build_inputs_before
    if($b.format_version -ne 1 -or $b.profile -cne 'verification-scale' -or $b.target -cne 'x86_64-pc-windows-msvc' -or $b.cargo_features.Count -ne 0){throw 'S54 build input policy.'}
    Assert-S54Equal $b.packages @('kilogram-cli','kilogram-transport-iroh')
    Assert-S54Environment $b.environment $I.runtime.environment;Assert-S54Equal $b.configs $I.runtime.configs
    if($b.rustc -cnotcontains 'release: 1.98.0' -or $b.rustc -cnotcontains 'host: x86_64-pc-windows-msvc' -or $b.cargo.Count -eq 0 -or $b.tools.Count -ne 2){throw 'S54 compiler/tool evidence.'}
    Assert-S54Strings $b.rustc 32;Assert-S54Strings $b.cargo 32
    for($n=0;$n -lt 2;$n++){
        $t=$b.tools[$n];$name=@('cargo','rustc')[$n];if($t.name -cne $name){throw 'S54 compiler tool order.'}
        if([IO.Path]::GetFileName($t.path) -cne "$name.exe"){throw 'S54 compiler executable role.'}
        foreach($tuple in @(@($t.path,$t.sha256),@($t.shim_path,$t.shim_sha256))){$bytes=Read-S54Public $tuple[0] 134217728;if($bytes.Length -eq 0 -or $tuple[1] -cnotmatch '^[0-9a-f]{64}$' -or (Get-S54Sha $bytes) -cne $tuple[1]){throw 'S54 compiler tool/shim bytes.'}}
        Assert-S54SamePath $t.shim_path @($I.runtime.resolutions|Where-Object name -CEQ "$name.exe")[0].path
    }
    $source=@{};foreach($r in $I.source.files){$rel=[IO.Path]::GetRelativePath($Policy.Root,$r.path).Replace('\','/');if($rel -cnotin $script:S54Embedded -and ($rel -notin $script:S54Helpers -or $rel -in $script:S54Helpers[0..2])){$source[$rel]=$r.sha256}}
    $projected=@{};$seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($r in $b.source_inputs){if($r.path -match '\\|(^|/)\.\.(/|$)' -or [IO.Path]::IsPathRooted($r.path) -or -not $seen.Add($r.path) -or $r.sha256 -cnotmatch '^[0-9a-f]{64}$'){throw 'S54 relative compiler source row.'};if((Test-S54Relevant $r.path) -and $r.path -cnotin $script:S54Embedded){$projected[$r.path]=$r.sha256}}
    Assert-S54Equal $projected $source
    if($b.package_names.Count -eq 0 -or $b.package_names.Count -ne $b.resolved_graph.Count){throw 'S54 complete package graph.'}
    $map=[Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach($row in $b.package_names){if(-not $row.id -or [Text.Encoding]::UTF8.GetByteCount($row.id) -gt 1024 -or -not $row.name -or [Text.Encoding]::UTF8.GetByteCount($row.name) -gt 256 -or $map.ContainsKey($row.id)){throw 'S54 unique package ID/name map.'};$map.Add($row.id,$row.name)}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($node in $b.resolved_graph){if(-not $map.ContainsKey($node.id) -or -not $seen.Add($node.id)){throw 'S54 graph package identity.'};$edges=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        Assert-S54Strings $node.features 256
        foreach($dep in $node.deps){if(-not $map.ContainsKey($dep.pkg) -or -not $dep.name -or [Text.Encoding]::UTF8.GetByteCount($dep.name) -gt 256 -or -not $edges.Add((ConvertTo-KilogramFrozenJson @($dep.name,$dep.pkg))) -or $dep.dep_kinds.Count -eq 0 -or $dep.dep_kinds.Count -gt 32){throw 'S54 graph edge.'}
            $kinds=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach($k in $dep.dep_kinds){if(($null -ne $k.kind -and $k.kind -cnotin @('normal','build','dev')) -or ($null -ne $k.target -and [Text.Encoding]::UTF8.GetByteCount($k.target) -gt 1024) -or -not $kinds.Add((ConvertTo-KilogramFrozenJson @($k.kind,$k.target)))){throw 'S54 bounded dependency kinds.'}}
        }
    }
    if($Capture.units.Count -eq 0 -or $Capture.units.Count -ne $Capture.raw_compiler_artifact_json.Count){throw 'S54 complete compiler artifact/units evidence.'}
    $sourceIndex=New-S54CompilerSourceIndex $I.source.files
    $prefix=(Get-S54Path (Join-Path $Policy.Root 'target/x86_64-pc-windows-msvc/verification-scale') -AllowMissing)+[IO.Path]::DirectorySeparatorChar
    foreach($line in $Capture.raw_compiler_artifact_json){
        $message=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($line)) 131072;Assert-S54Wire $message 'CargoArtifact'
        if($message.reason -cne 'compiler-artifact' -or -not $map.ContainsKey($message.package_id)){throw 'S54 actual known compiler-artifact.'}
        Assert-S54Strings $message.target.kind 8;Assert-S54Strings $message.target.crate_types 8;Assert-S54Strings $message.features 256
        if($message.target.kind.Count -eq 0 -or -not $message.target.name -or [Text.Encoding]::UTF8.GetByteCount($message.target.name) -gt 256 -or $message.target.edition -cnotin @('2015','2018','2021','2024') -or $message.filenames.Count -eq 0 -or $message.filenames.Count -gt 32){throw 'S54 bounded compiler target/output facts.'}
        $names=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach($f in $message.filenames){if(-not $names.Add((Get-S54Path $f -AllowMissing))){throw 'S54 unique compiler output paths.'}}
        $manifest=Get-S54Path $message.manifest_path;$null=Get-S54Path $message.target.src_path
        if($manifest.StartsWith($Policy.Root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){
            foreach($path in @($message.manifest_path,$message.target.src_path)){if(-not $sourceIndex.Contains((Get-S54Path $path))){throw 'S54 workspace compiler target belongs to admitted live sources.'}}
        }
        $runtime=@($message.filenames|Where-Object {(Get-S54Path $_ -AllowMissing).StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)}).Count -gt 0 -and -not @($message.target.kind|Where-Object {$_ -cin @('custom-build','proc-macro')}).Count
        if($runtime -and ($message.profile.opt_level -cne '3' -or -not $message.profile.debug_assertions -or -not $message.profile.overflow_checks -or $message.profile.debuginfo -isnot [string] -or $message.profile.debuginfo -cne 'line-tables-only')){throw 'S54 runtime opt3/debug/overflow profile.'}
    }
    $actual=@(Get-KilogramCompilerUnits -Lines $Capture.raw_compiler_artifact_json -Workspace $Policy.Root -Target $b.target -Profile $b.profile -PackageNames $b.package_names)
    $canonical={param($u)
        $v=[ordered]@{};foreach($k in $u.Keys){$v[$k]=$u[$k]}
        $v.filenames=@($u.filenames|ForEach-Object {Get-S54Path $_ -AllowMissing})
        $v.features=@($u.features|Sort-Object -CaseSensitive)
        if($null -ne $u.executable){$v.executable=Get-S54Path $u.executable -AllowMissing}
        ConvertTo-KilogramFrozenJson (ConvertTo-S54Canonical $v)
    }
    $actualSet=@($actual|ForEach-Object {& $canonical $_}|Sort-Object -Unique)
    $givenSet=@($Capture.units|ForEach-Object {& $canonical $_}|Sort-Object -Unique)
    if($givenSet.Count -ne $Capture.units.Count -or $actualSet.Count -ne $Capture.raw_compiler_artifact_json.Count){throw 'S54 duplicate compiler unit.'};Assert-S54Equal $actualSet $givenSet
    $roles=@(if($Normal){$script:S54Images[1]}else{$script:S54Images[3..8]})
    if($Capture.stable_artifacts.Count -ne $roles.Count){throw 'S54 exact compiler executable set.'}
    $seenRoles=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);$seenPaths=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($a in $Capture.stable_artifacts){
        $r=@($roles|Where-Object {$_.package -ceq $a.package -and $_.kind -ceq $a.kind -and $_.target -ceq $a.target});if($r.Count -ne 1){throw 'S54 compiler artifact role.'}
        if(-not $seenRoles.Add($r[0].role) -or -not $seenPaths.Add((Get-S54Path $a.path))){throw 'S54 duplicate compiler image path/role.'}
        $pin=@($I.images|Where-Object role -CEQ $r[0].role)[0].pin
        Assert-S54SamePath $a.path $pin.path;if($a.bytes -ne $pin.bytes -or $a.sha256 -cne $pin.sha256 -or $a.profile -cne 'verification-scale'){throw 'S54 compiler fixed image pin.'}
        $u=@($Capture.units|Where-Object {$_.package -ceq $a.package -and $_.target -ceq $a.target -and (@($_.kind)-join ',') -ceq $a.kind -and $_.profile.test -ne $Normal -and $_.class -ceq 'runtime-target' -and $_.executable})
        if($u.Count -ne 1){throw 'S54 one actual compiler executable unit.'}
        if(-not (Get-S54Path $u[0].executable).StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)){throw 'S54 compiler executable target/profile path.'}
        Assert-S54CompilerImageBytes $u[0].executable $pin
    }
    Assert-S54CompilerSourcesFresh $I.source.files
}
function Assert-S54Inventory($I,$Policy,[string]$State='Fresh',$Owner=$null) {
    Assert-S54Wire $I 'Inventory'
    $id=$I.identity;$root=$Policy.Root
    if($I.schema -ne 1 -or $I.classification -cne $script:S54PolicyPurpose.InventoryClass -or $id.case -cne $script:S54Case -or $id.attempt -cne $script:S54Attempt -or $id.selector -cne $script:S54Selector -or $id.run_id -cnotmatch '^[0-9a-f]{64}$' -or $id.run_id -ceq ('0'*64)){throw 'S54 required identity.'}
    Assert-S54Equal $I.deadline $script:S54PolicyPurpose.Deadline
    Assert-S54SamePath $id.workspace $root;Assert-S54SamePath $id.run (Join-Path $root ".tmp/$($script:S54PolicyPurpose.Namespace)/$script:S54Attempt")
    foreach($n in @('activation','launch','capture')){Assert-S54SamePath $id[$n] (Join-Path $id.run "$n.json")};Assert-S54SamePath $id.output (Join-Path $id.run 'assertions.json')
    Assert-S54Directory $I $Policy $State $Owner
    $public=Join-Path $root ".tmp/$($script:S54PolicyPurpose.PublicNamespace)/$script:S54Attempt"
    foreach($pair in @(@($I.compiler.normal,'normal.json'),@($I.compiler.tests,'tests.json'),@($I.compiler.frozen_manifest,'frozen.json'),@($I.selection.isolated,'isolated.json'),@($I.selection.list,'list.txt'))){Assert-S54SamePath $pair[0].path (Join-Path $public $pair[1])}
    if($I.source.s53_product -cne '1f404b54ebce47ae3b52eaeb6855e30f3b8f3309' -or $I.source.closure_publication -cne '20418e41e4c486158fdc30b0c9f857750432e25e' -or $I.source.implementation -cnotmatch '^[0-9a-f]{40}$' -or $I.source.observed_head -cne $I.source.implementation){throw 'S54 source revision roles.'}
    $head=Get-S54SourceHead $root
    if($head -cne $I.source.observed_head){throw 'S54 actual HEAD.'}
    $actual=Get-S54SourcePaths $root;$pins=@($I.source.files|ForEach-Object {Get-S54Path $_.path})
    Assert-S54Equal @($actual|Sort-Object) @($pins|Sort-Object)
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($p in $I.source.files){if(-not $seen.Add($p.path)){throw 'S54 duplicate source.'};$null=Read-S54Pin $p 8388608}
    if($I.images.Count -ne 9){throw 'S54 image set count.'};$seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($r in $I.images){if(-not $seen.Add($r.role)){throw 'S54 duplicate image role.'};$role=@($script:S54Images|Where-Object role -CEQ $r.role);if($role.Count -ne 1){throw 'S54 image role.'};Assert-S54SamePath $r.pin.path (Join-Path $root $role[0].path);if($r.pin.bytes -le 0){throw 'S54 empty image.'};$null=Read-S54Pin $r.pin 134217728}
    foreach($n in @('legacy-cli','relay')){$pin=@($I.images|Where-Object role -CEQ $n)[0].pin;$expected=if($n -ceq 'relay'){$Policy.Relay}else{$Policy.Legacy};if($pin.bytes -ne $expected.bytes -or $pin.sha256 -cne $expected.sha256){throw 'S54 accepted origin/relay bytes.'}}
    $r=$I.runtime
    if($r.cargo_jobs -ne 2 -or $r.priority -cne 'BelowNormal' -or -not $r.offline -or -not $r.locked -or -not $r.skip_cache -or $r.architecture -cne 'x64'){throw 'S54 resource/runtime policy.'}
    if($r.resolutions.Count -ne $Policy.Resolutions.Count){throw 'S54 exact resolver roles.'}
    for($n=0;$n -lt $r.resolutions.Count;$n++){if($r.resolutions[$n].name -cne $Policy.Resolutions[$n].name){throw 'S54 resolver role/order.'};Assert-S54SamePath $r.resolutions[$n].path $Policy.Resolutions[$n].path}
    $null=Read-S54Pin $r.metadata_shell 134217728;$null=Read-S54Pin $r.memory_shell 134217728
    Assert-S54SamePath $r.metadata_shell.path $Policy.Resolutions[0].path;Assert-S54SamePath $r.memory_shell.path $Policy.Resolutions[1].path
    if($Policy.Scope -cne 'public_schema_fixture_only'){Assert-S54SamePath $r.memory_shell.path 'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe';Assert-S54SamePath $r.metadata_shell.path ([Diagnostics.Process]::GetCurrentProcess().MainModule.FileName)}
    foreach($d in @($root,(Join-Path $root 'target/verification-scale/deps'))){if(Test-Path -LiteralPath (Join-Path $d 'powershell.exe')){throw 'S54 memory shell shadow.'}}
    Assert-S54Environment $r.environment @(Get-KilogramFrozenEnvironment -Entries @(Get-ChildItem Env:))
    if($env:CARGO_NET_OFFLINE -cne 'true'){throw 'S54 offline environment.'}
    if($r.configs.Count -ne $Policy.Configs.Count){throw 'S54 config search set.'}
    $configSet=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($path in $Policy.Configs){$null=$configSet.Add((Get-S54Path $path -AllowMissing))}
    foreach($row in $r.configs){if(-not $configSet.Remove((Get-S54Path $row.path -AllowMissing))){throw 'S54 config path role/alias.'}}
    foreach($c in $r.configs){$exists=Test-Path -LiteralPath $c.path -PathType Leaf;if($c.exists -ne $exists -or $c.exists -ne ($null -ne $c.sha256)){throw 'S54 config presence.'};if($exists){$bytes=Read-S54Public $c.path 65536;if((Get-S54Sha $bytes) -cne $c.sha256 -or [Text.Encoding]::UTF8.GetString($bytes) -match '(?i)(profile|rustflags|rustc|wrapper|include|\[env\]|linker|build\.target)'){throw 'S54 config identity/override.'}}}
    $value=[Environment]::GetEnvironmentVariable('TOKIO_WORKER_THREADS','Process');$effective=if($null -eq $value){[Environment]::ProcessorCount}else{[int]$value}
    if($r.workers.present -ne ($null -ne $value) -or $r.workers.value -cne $value -or $r.workers.available -ne [Environment]::ProcessorCount -or $r.workers.effective -ne $effective -or $effective -lt 2){throw 'S54 actual worker descriptor.'}
    $normal=Read-S54Json $I.compiler.normal 8388608;$tests=Read-S54Json $I.compiler.tests 8388608
    Assert-S54Compiler $I $Policy $normal $true;Assert-S54Compiler $I $Policy $tests $false
    Assert-S54Embedded $I $Policy $tests
    Assert-S54Equal $normal.build_inputs_before $tests.build_inputs_before
    Assert-S54Equal $I.compiler.normal_argv @('build','--package','kilogram-cli','--bin','kilogram-cli','--profile','verification-scale','--target','x86_64-pc-windows-msvc','--jobs','2','--offline','--locked','--message-format=json-render-diagnostics')
    Assert-S54Equal $I.compiler.tests_argv @('test','--package','kilogram-cli','--package','kilogram-transport-iroh','--profile','verification-scale','--target','x86_64-pc-windows-msvc','--jobs','2','--offline','--locked','--no-run','--message-format=json-render-diagnostics')
    $f=Read-S54Json $I.compiler.frozen_manifest 4194304;Assert-S54Wire $f 'FrozenManifest'
    if($f.format_version -ne 1 -or $f.classification -cne 'frozen-scale-test-artifacts' -or $f.profile -cne 'verification-scale' -or $f.target -cne 'x86_64-pc-windows-msvc'){throw 'S54 frozen manifest.'}
    Assert-S54SamePath $f.stable_directory (Join-Path $root 'target/verification-scale/deps');Assert-S54Equal $f.build_inputs $tests.build_inputs_before;Assert-S54Equal $f.artifacts $tests.stable_artifacts
    $identity=@($tests.units|ForEach-Object {$x=[ordered]@{};foreach($k in $_.Keys){if($k -cne 'fresh'){$x[$k]=$_[$k]}};$x});Assert-S54Equal $f.units_identity $identity
    $s=$I.selection;if($s.harness_ref -cne 'stable-migration' -or $s.named -ne 1 -or $s.native_markers -ne 1 -or $s.wrapper_markers -ne 1 -or $s.aggregates -ne 0){throw 'S54 exact whole test selection.'}
    $isolated=Read-S54Json $s.isolated 65536;Assert-S54Wire $isolated 'Isolated'
    Assert-S54Equal $isolated @{format_version=1;tests=@(@{package='kilogram-cli';target='migration_s5_process';kind='test';name=$script:S54Selector})}
    $list=[Text.Encoding]::UTF8.GetString((Read-S54Pin $s.list 1048576));if(@($list -split '\r?\n'|Where-Object {$_ -ceq ($script:S54Selector+': test')}).Count -ne 1){throw 'S54 prepared selector list.'}
}
function New-S54InventoryBytes($Specification,$Policy) {
    # Produces only in-memory public bytes. Actual final freeze/preparation is B.
    Assert-S54Inventory $Specification $Policy
    $bytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Specification -Depth 100 -Compress))
    if($bytes.Length -gt 2097152){throw 'S54 required inventory cap.'};return ,$bytes
}
function Test-S54Admission($Inputs,$Policy,[string]$State='Fresh',$Owner=$null) {
    if($Inputs -isnot [Collections.IDictionary] -or $Inputs.Count -ne 4){throw 'S54 exact input pin set.'}
    foreach($n in @('inventory','activation','launch','capture')){Assert-S54SamePath $Inputs[$n].path (Join-Path $Policy.Root ".tmp/$($script:S54PolicyPurpose.Namespace)/$script:S54Attempt/$(if($n -ceq 'inventory'){'inputs.json'}else{"$n.json"})")}
    $raw=Read-S54Pin $Inputs.inventory 2097152;$i=ConvertFrom-S54Strict $raw 2097152;Assert-S54Inventory $i $Policy -State $State -Owner $Owner
    $a=Read-S54Json $Inputs.activation 16384;$l=Read-S54Json $Inputs.launch 16384;$c=Read-S54Json $Inputs.capture 4096
    Assert-S54Wire $a 'Activation';Assert-S54Wire $l 'Launch';Assert-S54Wire $c 'Capture'
    if($State -cne 'Fresh'){Assert-S54StartedBinding $i $Inputs $Owner $c}
    foreach($n in @('inventory','activation','launch','capture')){Assert-S54SamePath $Inputs[$n].path (Join-Path $i.identity.run $(if($n -ceq 'inventory'){'inputs.json'}else{"$n.json"}))}
    if($a.schema -ne 1 -or $a.classification -cne $script:S54PolicyPurpose.ActivationClass -or $a.case -cne $i.identity.case -or $a.attempt -cne $i.identity.attempt -or $a.run_id -cne $i.identity.run_id -or $a.selector -cne $i.identity.selector -or $a.run -cne $i.identity.run -or $a.implementation -cne $i.source.implementation -or $a.execution_mode -cne 'NativeToken' -or $a.retry -cne 'root_reallocation_only'){throw 'S54 external activation identity.'}
    Assert-S54PinEqual $a.inventory $Inputs.inventory;Assert-S54Equal $a.deadline $i.deadline
    $now=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();if($a.not_before_utc_ms -ge $a.not_after_utc_ms -or $now -lt $a.not_before_utc_ms -or $now -ge $a.not_after_utc_ms){throw 'S54 activation validity.'}
    if($l.schema -ne 1 -or $l.classification -cne 's54_original_launch_binding' -or $l.case -cne $a.case -or $l.attempt -cne $a.attempt -or $l.run_id -cne $a.run_id -or $l.run -cne $a.run -or $l.start_tick_ms -le 0 -or $l.start_utc_ms -gt $now -or $l.start_utc_ms -lt $a.not_before_utc_ms -or $l.start_utc_ms -ge $a.not_after_utc_ms){throw 'S54 launch binding.'}
    Assert-S54PinEqual $l.inventory $Inputs.inventory;Assert-S54PinEqual $l.activation $Inputs.activation;Assert-S54PinEqual $l.capture $Inputs.capture
    foreach($pair in @(@('controller_tick_ms',($script:S54PolicyPurpose.Deadline.controller*1000)),@('work_tick_ms',($script:S54PolicyPurpose.Deadline.work*1000)),@('outer_tick_ms',($script:S54PolicyPurpose.Deadline.outer*1000)))){if([decimal]$l[$pair[0]] -ne ([decimal]$l.start_tick_ms+[decimal]$pair[1])){throw 'S54 original numeric tick relation.'}}
    if($c.version -ne 1 -or $c.attempt -cne $a.attempt -or $c.owner_pid -le 0 -or $c.launcher_pid -le 0 -or $c.owner_pid -eq $c.launcher_pid -or $c.owner_pid -ne $l.owner_pid -or $c.launcher_pid -ne $l.launcher_pid -or [decimal]$c.whole_utc_ms -ne ([decimal]$l.start_utc_ms+$script:S54PolicyPurpose.Deadline.outer*1000) -or $now -ge $c.whole_utc_ms){throw 'S54 capture/clock binding.'}
    Assert-S54SamePath $c.harness_path (Join-Path $Policy.Root $script:S54Images[5].path)
    foreach($n in @('inventory','activation','launch','capture')){$null=Read-S54Pin $Inputs[$n] 2097152}
    return [ordered]@{classification=$Policy.Scope;inventory_sha256=(Get-S54Sha $raw);clock='numeric binding only; safe clock/runtime connection requires Slice B';whole_attempt_verdict='not_established'}
}
# CLOSED_WIRE_TABLE is inserted from the adjacent reviewed Rust wire definitions.
$script:S54Schema=@{
 Pin=@{path='p';bytes='u';sha256='s'}
 Identity=@{case='s';attempt='s';selector='s';run_id='s';workspace='p';run='p';output='p';activation='p';launch='p';capture='p'}
 Deadline=@{controller='u';work='u';outer='u';origin='s';clock='s'}
 Source=@{s53_product='s';closure_publication='s';implementation='s';observed_head='s';files='aPin'}
 Image=@{role='s';pin='Pin'}
 Environment=@{name='s';present='b';value_sha256='ns'}
 Config=@{path='p';exists='b';sha256='ns'}
 BuildTool=@{name='s';path='p';sha256='s';shim_path='p';shim_sha256='s'}
 SourceDigest=@{path='s';sha256='s'}
 Package=@{id='s';name='s'}
 DepKind=@{kind='ns';target='ns'}
 Dependency=@{name='s';pkg='s';dep_kinds='aDepKind'}
 Graph=@{id='s';features='as';deps='aDependency'}
 BuildInputs=@{format_version='u';profile='s';target='s';packages='as';cargo_features='as';tools='aBuildTool';rustc='as';cargo='as';environment='aEnvironment';configs='aConfig';source_inputs='aSourceDigest';package_names='aPackage';resolved_graph='aGraph'}
 Profile=@{opt_level='s';debuginfo='j';debug_assertions='b';overflow_checks='b';test='b'}
 Unit=@{package_id='s';package='s';kind='as';target='s';features='as';profile='Profile';filenames='ap';executable='np';class='s';fresh='b'}
 CargoTarget=@{kind='as';crate_types='as';name='s';src_path='p';edition='s';doc='b';doctest='b';test='b'}
 CargoArtifact=@{reason='s';package_id='s';manifest_path='p';target='CargoTarget';profile='Profile';features='as';filenames='ap';executable='np';fresh='b'}
 Artifact=@{package='s';kind='s';target='s';path='p';bytes='u';sha256='s';profile='s'}
 CompilerCapture=@{classification='s';profile='s';target='s';expected_manifest_sha256='ns';build_inputs_before='BuildInputs';build_inputs_after='BuildInputs';units='aUnit';raw_compiler_artifact_json='araw';stable_artifacts='aArtifact';frozen_guard_verified='b'}
 Compiler=@{normal='Pin';tests='Pin';normal_argv='as';tests_argv='as';frozen_manifest='Pin';embedded='EmbeddedInputs'}
 EmbeddedInputs=@{before='aPin';after='aPin';dep_info='aPin'}
 FrozenManifest=@{format_version='u';classification='s';profile='s';target='s';stable_directory='p';build_inputs='BuildInputs';units_identity='aj';artifacts='aArtifact'}
 Resolution=@{name='s';path='p'}
 Workers=@{present='b';value='ns';available='u';effective='u'}
 Runtime=@{metadata_shell='Pin';memory_shell='Pin';resolutions='aResolution';environment='aEnvironment';configs='aConfig';workers='Workers';cargo_jobs='u';priority='s';offline='b';locked='b';skip_cache='b';architecture='s'}
 Selection=@{isolated='Pin';list='Pin';harness_ref='s';named='u';native_markers='u';wrapper_markers='u';aggregates='u'}
 Inventory=@{schema='u';classification='s';identity='Identity';deadline='Deadline';source='Source';compiler='Compiler';images='aImage';runtime='Runtime';selection='Selection'}
 Activation=@{schema='u';classification='s';inventory='Pin';case='s';attempt='s';run_id='s';run='p';selector='s';implementation='s';execution_mode='s';deadline='Deadline';not_before_utc_ms='u';not_after_utc_ms='u';retry='s'}
 Launch=@{schema='u';classification='s';inventory='Pin';activation='Pin';capture='Pin';case='s';attempt='s';run_id='s';run='p';owner_pid='u';launcher_pid='u';start_tick_ms='u';start_utc_ms='u';controller_tick_ms='u';work_tick_ms='u';outer_tick_ms='u'}
 Capture=@{version='u';attempt='s';owner_pid='u';launcher_pid='u';harness_path='p';whole_utc_ms='u'}
 Started=@{version='u';classification='s';case='s';attempt='s';run_id='s';selector='s';inventory='Pin';activation='Pin';owner_pid='u';nonce='s'}
 Claim=@{version='u';classification='s';case='s';attempt='s';run_id='s';owner_pid='u';launcher_pid='u';harness_pid='u';nonce='s';inventory_sha256='s';launch_sha256='s'}
 ClockReply=@{version='u';classification='s';case='s';attempt='s';run_id='s';owner_pid='u';launcher_pid='u';harness_pid='u';nonce='s';inventory_sha256='s';launch_sha256='s';request_sha256='s';origin_tick_ms='u';sampled_tick_ms='u';remaining_controller_ms='u'}
 FunctionalMarker=@{version='u';classification='s';whole_attempt_verdict='s';case='s';attempt='s';run_id='s';selector='s';inventory_sha256='s';inventory_blake3='s';assertions_sha256='s';assertions_bytes='u'}
 IsolatedEntry=@{package='s';target='s';kind='s';name='s'}
 Isolated=@{format_version='u';tests='aIsolatedEntry'}
}
Set-S54Purpose $Purpose
$script:S54Helpers=@('scripts/run-cargo-tests-stable.ps1','scripts/cargo-frozen-artifacts.ps1','scripts/cargo-resource-policy.ps1','scripts/migration-s5-l1-deadline.ps1','scripts/migration-s53-metadata.ps1','scripts/migration-s53-runtime-dependencies.ps1','scripts/migration-s5-l1-test-result.ps1','scripts/invoke-migration-s54-whole.ps1','scripts/test-migration-s54-admission.ps1','scripts/migration-s54-outer.ps1','scripts/test-migration-s54-whole.ps1','apps/kilogram-cli/tests/support/migration_s54_outer_fixture.ps1','scripts/migration-s54-preparation.ps1','scripts/test-migration-s54-repair.ps1','scripts/test-migration-s54-feasibility.ps1','scripts/test-migration-s54-preparation-order.ps1','scripts/test-migration-s54-diagnostics.ps1')
$script:S54Embedded=@('apps/kilogram-cli/tests/support/migration_s54_capture_fixture.ps1','apps/kilogram-cli/tests/support/migration_s54_memory_fixture.ps1','ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.candidate.json')
$script:S54Images=@(
 @{role='legacy-cli';path='target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe';package='';kind='';target=''},
 @{role='current-cli';path='target/compatibility/long-lived-dialog-migration/kilogram-cli.exe';package='kilogram-cli';kind='bin';target='kilogram-cli'},
 @{role='relay';path='target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe';package='';kind='';target=''},
 @{role='stable-cli-bin';path='target/verification-scale/deps/kilogram-cli-bin-kilogram-cli-tests.exe';package='kilogram-cli';kind='bin';target='kilogram-cli'},
 @{role='stable-cli-lib';path='target/verification-scale/deps/kilogram-cli-lib-kilogram_cli-tests.exe';package='kilogram-cli';kind='lib';target='kilogram_cli'},
 @{role='stable-migration';path='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';package='kilogram-cli';kind='test';target='migration_s5_process'},
 @{role='stable-runtime2';path='target/verification-scale/deps/kilogram-cli-test-runtime2_process-tests.exe';package='kilogram-cli';kind='test';target='runtime2_process'},
 @{role='stable-iroh-lib';path='target/verification-scale/deps/kilogram-transport-iroh-lib-kilogram_transport_iroh-tests.exe';package='kilogram-transport-iroh';kind='lib';target='kilogram_transport_iroh'},
 @{role='stable-iroh-relay';path='target/verification-scale/deps/kilogram-transport-iroh-test-local_relay_transport-tests.exe';package='kilogram-transport-iroh';kind='test';target='local_relay_transport'}
)
. (Join-Path $PSScriptRoot 'migration-s54-outer.ps1')
. (Join-Path $PSScriptRoot 'migration-s54-preparation.ps1')
if($MyInvocation.InvocationName -ne '.') {
    switch($Action) {
        'Run' {Invoke-S54Whole -InventorySha256 $InventorySha256 -ActivationSha256 $ActivationSha256}
        'Prepare' {
            if($PreparationRunId -cnotmatch '^[0-9a-f]{64}$' -or $PreparationRunId -ceq ('0'*64)){throw 'S54 requires an explicit root preparation identity.'}
            Invoke-S54Preparation $PreparationRunId
        }
        default {throw 'S54 requires an explicit reviewed Prepare or externally activated Run action.'}
    }
}
