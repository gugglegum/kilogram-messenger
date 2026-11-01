# Opt-in public build provenance and post-staging identity checks. No test invocation.
function Get-KilogramFrozenHash([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-KilogramFrozenDigest([string]$Value) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Value))).ToLowerInvariant()
}
function ConvertTo-KilogramFrozenJson($Value) { ConvertTo-Json -InputObject $Value -Depth 60 -Compress }
function New-KilogramCargoPackageMap($Packages) {
    $map=[Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach($p in @($Packages)) {
        if([string]::IsNullOrWhiteSpace($p.id) -or [string]::IsNullOrWhiteSpace($p.name)){throw 'Missing Cargo package ID/name.'}
        if($map.ContainsKey($p.id) -and $map[$p.id] -cne $p.name){throw 'Conflicting Cargo package ID mapping.'}
        $map[$p.id]=$p.name
    }
    return ,$map
}
function Get-KilogramCargoPackageName([string]$Id,$Map) {
    if(-not $Map.ContainsKey($Id)){throw 'Unknown Cargo package ID.'}
    $Map[$Id]
}
function Get-KilogramR6ReferenceRows($Lines,$PackageNames,$ExpectedImages,[string]$Workspace) {
    $map=New-KilogramCargoPackageMap $PackageNames;$rows=@()
    foreach($line in @($Lines)) {
        try{$m=$line|ConvertFrom-Json -AsHashtable}catch{continue}
        if($m.reason -cne 'compiler-artifact'){continue}
        $package=Get-KilogramCargoPackageName $m.package_id $map
        if(-not $m.profile.test -or -not $m.executable){continue}
        $stem=($package+'-'+(@($m.target.kind)-join '-')+'-'+$m.target.name+'-tests') -replace '[^A-Za-z0-9._-]','-'
        $expected=@($ExpectedImages|Where-Object path -CEQ "target/verification/deps/$stem.exe")
        $source=[IO.Path]::GetFullPath($m.executable)
        $prefix=[IO.Path]::GetFullPath((Join-Path $Workspace 'target/verification/deps'))+[IO.Path]::DirectorySeparatorChar
        if(-not $source.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase) -or $expected.Count -ne 1 -or (Get-Item -LiteralPath $source).Length -ne $expected[0].bytes -or (Get-KilogramFrozenHash $source) -cne $expected[0].sha256){throw 'Compiler reference does not match frozen r6 image.'}
        $rows+=[ordered]@{package_id=$m.package_id;package=$package;kind=@($m.target.kind);target=$m.target.name;source=$source;expected_path=$expected[0].path;bytes=$expected[0].bytes;sha256=$expected[0].sha256;features=@($m.features|Sort-Object);profile=$m.profile;fresh=$m.fresh}
    }
    if($rows.Count -ne 6 -or @($rows.expected_path|Sort-Object -Unique).Count -ne 6){throw 'Exactly six distinct frozen r6 references required.'}
    $rows
}
function Get-KilogramFrozenEnvironment($Entries) {
    $map=[Collections.Generic.Dictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($entry in @($Entries)) {
        $name=([string]$entry.Name).ToUpperInvariant()
        if($map.ContainsKey($name) -and $map[$name] -cne [string]$entry.Value){throw 'Conflicting Windows environment aliases.'}
        $map[$name]=[string]$entry.Value
    }
    $names=@('PATH','CARGO_HOME','RUSTUP_HOME','RUSTUP_TOOLCHAIN','CARGO_NET_OFFLINE','CARGO_INCREMENTAL','CC','CXX','AR','CFLAGS','CXXFLAGS','ARFLAGS','INCLUDE','LIB','LIBPATH','WINDOWSSDKDIR','VCTOOLSINSTALLDIR','VSINSTALLDIR')
    $names+=@($map.Keys|Where-Object {$_ -match '^(RUST|CARGO_(PROFILE_|BUILD_|TARGET_|ENCODED_|INCREMENTAL|FEATURE_|FEATURES|FLAGS))'})
    @($names|Sort-Object -Unique|ForEach-Object {
        $present=$map.ContainsKey($_);$value=if($present){$map[$_]}else{$null}
        if($present -and ($_ -match '^(RUSTFLAGS$|CARGO_ENCODED_RUSTFLAGS$|RUSTC($|_)|CARGO_PROFILE_|CARGO_BUILD_|CARGO_TARGET_DIR$|CARGO_TARGET_.*_(RUSTFLAGS|LINKER|RUNNER)$)' -or ($_ -ceq 'CARGO_INCREMENTAL' -and $value -cne '0'))){throw "Unproved frozen build override: $_"}
        [ordered]@{name=$_;present=$present;value_sha256=if($present){Get-KilogramFrozenDigest $value}else{$null}}
    })
}
function Write-KilogramFrozenJson([string]$Path,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 60)+[Environment]::NewLine)
    $file=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try{$file.Write($raw,0,$raw.Length);$file.Flush($true)}finally{$file.Dispose()}
}
function Get-KilogramFrozenBuildInputs {
    param([string]$Workspace,[string]$Profile,[string]$Target,[string[]]$Package,[string[]]$CargoFeatures)
    if($Profile -cne 'verification-scale' -or $Target -cne 'x86_64-pc-windows-msvc' -or (@($Package|Sort-Object)-join ',') -cne 'kilogram-cli,kilogram-transport-iroh' -or $CargoFeatures.Count){throw 'Frozen scale build selection differs.'}
    $environment=@(Get-KilogramFrozenEnvironment -Entries @(Get-ChildItem Env:))
    if($env:CARGO_NET_OFFLINE -cne 'true'){throw 'Frozen compile must be offline.'}
    $configs=@();$directory=[IO.DirectoryInfo]::new($Workspace)
    while($null -ne $directory){foreach($name in @('config','config.toml')){$configs+=Join-Path $directory.FullName ('.cargo/'+$name)};$directory=$directory.Parent}
    $cargoHome=if($env:CARGO_HOME){$env:CARGO_HOME}else{Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo'}
    $configs+=@(Join-Path $cargoHome 'config';Join-Path $cargoHome 'config.toml')
    $configRows=@($configs|Sort-Object -Unique|ForEach-Object {
        $exists=Test-Path -LiteralPath $_ -PathType Leaf
        if($exists -and (Get-Content -LiteralPath $_ -Raw) -match '(?im)(profile|rustflags|rustc|wrapper|include|\[env\]|linker|build\.target)'){throw 'Unproved profile-sensitive Cargo config; contents withheld.'}
        [ordered]@{path=[IO.Path]::GetFullPath($_);exists=$exists;sha256=if($exists){Get-KilogramFrozenHash $_}else{$null}}
    })
    $tools=@();foreach($name in @('cargo','rustc')){
        $selected=(& rustup which $name);if($LASTEXITCODE -ne 0){throw 'Selected compiler path unavailable.'}
        $selected=[IO.Path]::GetFullPath([string]$selected)
        $shim=(Get-Command $name -CommandType Application -ErrorAction Stop).Source
        $tools+=[ordered]@{name=$name;path=$selected;sha256=(Get-KilogramFrozenHash $selected);shim_path=$shim;shim_sha256=(Get-KilogramFrozenHash $shim)}
    }
    $rustc=@(& rustc -vV);if($LASTEXITCODE -ne 0 -or -not($rustc -ccontains 'release: 1.98.0') -or -not($rustc -ccontains "host: $Target")){throw 'Pinned compiler/host mismatch.'}
    $cargo=@(& cargo -vV);if($LASTEXITCODE -ne 0){throw 'Cargo version unavailable.'}
    $metadataRaw=@(& cargo metadata --manifest-path (Join-Path $Workspace 'Cargo.toml') --locked --offline --format-version 1 --filter-platform $Target)
    if($LASTEXITCODE -ne 0){throw 'Frozen Cargo graph unavailable.'}
    $metadata=($metadataRaw -join [Environment]::NewLine)|ConvertFrom-Json
    $null=New-KilogramCargoPackageMap $metadata.packages
    $packageNames=@($metadata.packages|Sort-Object id|ForEach-Object {[ordered]@{id=$_.id;name=$_.name}})
    $graph=@($metadata.resolve.nodes|Sort-Object id|ForEach-Object {[ordered]@{id=$_.id;features=@($_.features|Sort-Object);deps=@($_.deps|Sort-Object name,pkg|ForEach-Object {[ordered]@{name=$_.name;pkg=$_.pkg;dep_kinds=$_.dep_kinds}})}})
    $sourcePaths=@(& git -C $Workspace ls-files --cached --others --exclude-standard -- '*.rs' '*.c' '*.h' '*.cpp' 'Cargo.toml' '**/Cargo.toml' 'Cargo.lock' 'rust-toolchain*')
    $sourcePaths+=@('scripts/run-cargo-tests-stable.ps1','scripts/cargo-frozen-artifacts.ps1','scripts/cargo-resource-policy.ps1')
    $sourceRows=@($sourcePaths|Sort-Object -Unique|ForEach-Object {[ordered]@{path=$_;sha256=(Get-KilogramFrozenHash (Join-Path $Workspace $_))}})
    [ordered]@{format_version=1;profile=$Profile;target=$Target;packages=@($Package|Sort-Object);cargo_features=@($CargoFeatures|Sort-Object);tools=$tools;rustc=$rustc;cargo=$cargo;environment=$environment;configs=$configRows;source_inputs=$sourceRows;package_names=$packageNames;resolved_graph=$graph}
}
function Read-KilogramFrozenManifest([string]$Path,[string]$Sha256) {
    if(-not [IO.Path]::IsPathRooted($Path) -or $Sha256 -cnotmatch '^[a-f0-9]{64}$' -or (Get-Item -LiteralPath $Path).Length -gt 4MB -or (Get-KilogramFrozenHash $Path) -cne $Sha256){throw 'Exact frozen artifact manifest required.'}
    $manifest=Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json -AsHashtable
    if($manifest.format_version -ne 1 -or $manifest.classification -cne 'frozen-scale-test-artifacts' -or $manifest.artifacts.Count -ne 6){throw 'Frozen artifact manifest schema/set.'}
    $manifest
}
function Assert-KilogramFrozenInputs($Expected,$Actual) {
    if((ConvertTo-KilogramFrozenJson $Expected) -cne (ConvertTo-KilogramFrozenJson $Actual)){throw 'Frozen build provenance mismatch before executable use.'}
}
function Get-KilogramCompilerUnits($Lines,[string]$Workspace,[string]$Target,[string]$Profile,$PackageNames) {
    $packageMap=New-KilogramCargoPackageMap $PackageNames
    $units=@();$prefix=[IO.Path]::GetFullPath((Join-Path $Workspace "target/$Target/$Profile"))+[IO.Path]::DirectorySeparatorChar
    foreach($line in $Lines){
        try{$m=$line|ConvertFrom-Json -AsHashtable}catch{continue}
        if($m.reason -cne 'compiler-artifact'){continue}
        $files=@($m.filenames|ForEach-Object {[IO.Path]::GetFullPath($_)}|Sort-Object)
        $runtime=@($files|Where-Object {$_.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)}).Count -gt 0
        $hostKind=@($m.target.kind|Where-Object {$_ -in @('custom-build','proc-macro')}).Count -gt 0
        if($runtime -and -not $hostKind -and ($m.profile.opt_level -cne '3' -or $m.profile.debug_assertions -ne $true -or $m.profile.overflow_checks -ne $true)){throw 'Unproved runtime opt3/debug/overflow profile.'}
        if($m.fresh -isnot [bool]){throw 'Missing compiler fresh state.'}
        $packageName=Get-KilogramCargoPackageName $m.package_id $packageMap
        $units+=[ordered]@{package_id=$m.package_id;package=$packageName;kind=@($m.target.kind);target=$m.target.name;features=@($m.features|Sort-Object);profile=$m.profile;filenames=$files;executable=if($m.executable){[IO.Path]::GetFullPath($m.executable)}else{$null};class=if($runtime -and -not $hostKind){'runtime-target'}else{'host-build-or-proc-macro'};fresh=$m.fresh}
    }
    foreach($name in @('kilogram-cli','kilogram-state','kilogram-identity','kilogram-protocol','kilogram-transport-iroh','kilogram-ratchet','redb','ed25519-dalek','curve25519-dalek','chacha20poly1305','iroh','vodozemac')) {
        if(-not @($units|Where-Object {$_.class -ceq 'runtime-target' -and $_.package -ceq $name}).Count){throw "Missing runtime profile evidence: $name"}
    }
    @($units|Sort-Object package_id,target,{@($_.kind)-join ','},{@($_.filenames)-join ','})
}
function Get-KilogramFrozenUnitIdentity($Units) {
    @($Units|ForEach-Object {[ordered]@{package_id=$_.package_id;package=$_.package;kind=$_.kind;target=$_.target;features=$_.features;profile=$_.profile;filenames=$_.filenames;executable=$_.executable;class=$_.class}})
}
function Get-KilogramFrozenStableRows($Artifacts,[string]$Profile) {
    @($Artifacts|Sort-Object Package,Kind,Target,Path|ForEach-Object {
        [ordered]@{package=$_.Package;kind=$_.Kind;target=$_.Target;path=[IO.Path]::GetFullPath($_.Path);bytes=(Get-Item -LiteralPath $_.Path).Length;sha256=(Get-KilogramFrozenHash $_.Path);profile=$Profile}
    })
}
function Assert-KilogramFrozenArtifacts($Manifest,$Actual,[string]$Profile,[string]$Target,[string]$StableDirectory) {
    if($Profile -cne $Manifest.profile -or $Target -cne $Manifest.target -or [IO.Path]::GetFullPath($StableDirectory) -cne $Manifest.stable_directory -or @($Actual).Count -ne 6){throw 'Frozen profile/directory/artifact count mismatch.'}
    $expected=@($Manifest.artifacts|Sort-Object package,kind,target,path)
    $actualRows=@($Actual|Sort-Object package,kind,target,path)
    if((ConvertTo-KilogramFrozenJson $expected) -cne (ConvertTo-KilogramFrozenJson $actualRows)){throw 'Frozen staged package/kind/target/path/size/hash/profile mismatch.'}
}
