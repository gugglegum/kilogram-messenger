Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts/cargo-resource-policy.ps1'
$null=Set-KilogramCargoResourcePolicy -RequestedJobs 2
$env:CARGO_NET_OFFLINE='true'
$cargoArgs=@('clippy','--jobs','2','--package','kilogram-cli','--all-targets','--profile','verification-scale','--target','x86_64-pc-windows-msvc','--offline','--locked','--','-D','warnings')
& cargo @cargoArgs
exit $LASTEXITCODE
