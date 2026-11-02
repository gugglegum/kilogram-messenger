param([Parameter(Mandatory)][string]$ReadyFile)
$ErrorActionPreference='Stop'
# Benign resident fixture only: no profile/store/network or grandchildren.
[IO.File]::WriteAllText($ReadyFile,'ready')
[Threading.Thread]::Sleep(30000)
