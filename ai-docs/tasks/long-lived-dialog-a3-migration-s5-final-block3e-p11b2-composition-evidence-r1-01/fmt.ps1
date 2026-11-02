$ErrorActionPreference='Stop'
& cargo fmt --all
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
& cargo fmt --all -- --check
exit $LASTEXITCODE
