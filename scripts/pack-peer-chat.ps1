# Fresh portable kit only. No installer, profile copying, deployment or updater.
param([Parameter(Mandatory)][string]$Out,[string]$Images='target/verification-scale',[Parameter(Mandatory)][string]$BuildReceipt)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$repo=Split-Path $PSScriptRoot -Parent;$Out=[IO.Path]::GetFullPath($Out);$Images=[IO.Path]::GetFullPath($Images)
if(!$Out.StartsWith(([IO.Path]::GetFullPath((Join-Path $repo '.tmp/usable-peer-chat'))+'\'),[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $Out)){throw 'Fresh own kit directory required'}
$receipt=Get-Content -LiteralPath $BuildReceipt -Raw|ConvertFrom-Json
if($receipt.exit -ne 0 -or !$receipt.source_unchanged -or $receipt.error -or $receipt.forced -or !$receipt.empty_owned_job){throw 'Successful source-bound clean build required'}
$names=@('kilogram-peer.exe','kilogram-cli.exe','kilogram-bootstrap.exe')
foreach($name in $names){if((Get-FileHash -LiteralPath (Join-Path $Images $name)).Hash -ine $receipt.images.$name){throw 'Image differs from build receipt'}}
[void][IO.Directory]::CreateDirectory($Out)
Add-Type -AssemblyName System.IO.Compression.FileSystem
foreach($side in @('PC','Laptop')){
    $kit=Join-Path $Out $side;$app=Join-Path $kit 'app';[void][IO.Directory]::CreateDirectory($app)
    foreach($name in $names){[IO.File]::Copy((Join-Path $Images $name),(Join-Path $app $name),$false)}
    foreach($name in @('start-peer-chat.cmd','test-peer-receiver.cmd','test-usable-peer-receiver.ps1','migration-s5-l1-deadline.ps1')){
        $source=Join-Path $PSScriptRoot $name;$destination=Join-Path $app $name
        # cmd.exe can truncate UTF-8 batch lines with LF-only input. Normalize
        # package bytes independently of Git's checkout/autocrlf configuration.
        if($name.EndsWith('.cmd')){[IO.File]::WriteAllText($destination,([IO.File]::ReadAllText($source) -replace '\r?\n',"`r`n"),[Text.UTF8Encoding]::new($false))}
        else{[IO.File]::Copy($source,$destination,$false)}
    }
    [IO.File]::Copy((Join-Path $repo 'ai-docs/usable-peer-chat-guide.md'),(Join-Path $kit 'README.md'),$false)
    $launch="@echo off`r`ncall `"%~dp0app\start-peer-chat.cmd`" `"%~dp0profile`"`r`nexit /b %errorlevel%`r`n"
    [IO.File]::WriteAllText((Join-Path $kit 'Start-Kilogram.cmd'),$launch,[Text.UTF8Encoding]::new($false))
    $pilot=if($side -ceq 'PC'){Join-Path $repo '.tmp/manual-pilot-pc/profile'}else{'C:\Users\test\KilogramLab\manual-pilot\profile'}
    $launch="@echo off`r`ncall `"%~dp0app\start-peer-chat.cmd`" `"$pilot`"`r`nexit /b %errorlevel%`r`n"
    [IO.File]::WriteAllText((Join-Path $kit 'Open-existing-pilot.cmd'),$launch,[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $kit 'Network-test.cmd'),"@echo off`r`ncall `"%~dp0app\test-peer-receiver.cmd`"`r`nexit /b %errorlevel%`r`n",[Text.UTF8Encoding]::new($false))
    $manifest=[ordered]@{}
    foreach($file in @(Get-ChildItem -LiteralPath $kit -File -Recurse|Sort-Object FullName)){$manifest[$file.FullName.Substring($kit.Length+1).Replace('\','/')]=(Get-FileHash -LiteralPath $file.FullName).Hash.ToLowerInvariant()}
    @{v=1;candidate=$true;side=$side;build_receipt_sha256=(Get-FileHash -LiteralPath $BuildReceipt).Hash.ToLowerInvariant();pack_script_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant();source_inventory_sha256=$receipt.source_inventory_sha256;files=$manifest;personal_profiles_included=$false}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $kit 'SHA256.json') -Encoding UTF8
    [IO.Compression.ZipFile]::CreateFromDirectory($kit,(Join-Path $Out ('Kilogram-'+$side+'.zip')))
}
Get-FileHash -LiteralPath (Join-Path $Out 'Kilogram-PC.zip'),(Join-Path $Out 'Kilogram-Laptop.zip') -Algorithm SHA256
