$ErrorActionPreference='Stop'
& cargo fmt --all
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
& cargo fmt --all -- --check
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
foreach($name in @('scripts/invoke-migration-s54-whole.ps1','scripts/migration-s54-outer.ps1','scripts/test-migration-s54-whole.ps1','scripts/test-migration-s54-admission.ps1','scripts/run-cargo-tests-stable.ps1','scripts/migration-s5-l1-deadline.ps1','apps/kilogram-cli/tests/support/migration_s54_outer_fixture.ps1')){
    $tokenList=$null;$parseErrors=$null
    $null=[System.Management.Automation.Language.Parser]::ParseFile((Join-Path (Get-Location) $name),[ref]$tokenList,[ref]$parseErrors)
    if($parseErrors.Count){throw ($parseErrors|Out-String)}
}
exit 0
