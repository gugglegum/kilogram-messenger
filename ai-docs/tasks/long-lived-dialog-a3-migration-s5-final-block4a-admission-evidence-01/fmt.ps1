$ErrorActionPreference='Stop'
& cargo fmt --all
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
& cargo fmt --all -- --check
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
foreach($name in @('scripts/invoke-migration-s54-whole.ps1','scripts/test-migration-s54-admission.ps1')){
    $tokenList=$null;$parseErrors=$null
    $null=[System.Management.Automation.Language.Parser]::ParseFile((Join-Path (Get-Location) $name),[ref]$tokenList,[ref]$parseErrors)
    if($parseErrors.Count){throw ($parseErrors|Out-String)}
}
exit 0
