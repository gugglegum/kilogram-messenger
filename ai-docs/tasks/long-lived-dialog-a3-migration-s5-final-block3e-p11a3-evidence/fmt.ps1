$ErrorActionPreference='Stop'
& cargo fmt --all -- --check
exit $LASTEXITCODE
