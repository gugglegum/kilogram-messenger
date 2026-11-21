# Source-bound optimized build of the three portable product images.
param([Parameter(Mandatory)][string]$Evidence)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$repo=Split-Path $PSScriptRoot -Parent;$Evidence=[IO.Path]::GetFullPath($Evidence)
if(!$Evidence.StartsWith(([IO.Path]::GetFullPath((Join-Path $repo '.tmp/usable-peer-chat'))+'\'),[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $Evidence)){throw 'Fresh own build evidence required'}
[void][IO.Directory]::CreateDirectory($Evidence)
. "$PSScriptRoot/cargo-resource-policy.ps1";$jobs=Set-KilogramCargoResourcePolicy
function Source-Inventory {
    Push-Location $repo
    try {
        $files=@(& rg --files apps crates);if($LASTEXITCODE -ne 0){throw 'Source inventory'}
        $files+=@('Cargo.toml','Cargo.lock')
        foreach($optional in @('rust-toolchain.toml','.cargo/config.toml')){if(Test-Path -LiteralPath $optional){$files+=$optional}}
        $rows=[ordered]@{}
        foreach($name in @($files|Sort-Object -Unique)){$rows[$name.Replace('\','/')]=(Get-FileHash -LiteralPath $name).Hash.ToLowerInvariant()}
        return ($rows|ConvertTo-Json -Depth 4 -Compress)
    }finally{Pop-Location}
}
$before=Source-Inventory
[IO.File]::WriteAllText((Join-Path $Evidence 'source-inventory.json'),$before,[Text.UTF8Encoding]::new($false))
. "$PSScriptRoot/migration-s5-l1-deadline.ps1"
$watch=[KilogramS5Deadline]::new(3300,3360,[KilogramS5Deadline]::TickNow())
$child=$null;$exit=$null;$primary=$null;$forced=$false;$empty=$false;$same=$false;$images=[ordered]@{}
try {
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName=(Get-Command cargo -CommandType Application|Select-Object -First 1).Source
    $si.Arguments='build --locked --offline -p kilogram-cli -p kilogram-bootstrap --bin kilogram-peer --bin kilogram-cli --bin kilogram-bootstrap --profile verification-scale --jobs '+$jobs
    $si.WorkingDirectory=$repo;$si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardInput=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
    $child=[Diagnostics.Process]::Start($si);$watch.VerifyChild($child);$child.StandardInput.Close()
    $stdout=[IO.File]::Create((Join-Path $Evidence 'cargo.stdout.log'));$stderr=[IO.File]::Create((Join-Path $Evidence 'cargo.stderr.log'))
    try{
        $ot=$child.StandardOutput.BaseStream.CopyToAsync($stdout);$et=$child.StandardError.BaseStream.CopyToAsync($stderr)
        while(!$child.WaitForExit(200)){$watch.CheckWork()}
        $watch.CheckWork();$exit=$child.ExitCode
        if(!$ot.Wait(2000) -or !$et.Wait(2000)){throw 'Compiler output EOF'}
    }finally{$stdout.Dispose();$stderr.Dispose()}
    $same=$before -ceq (Source-Inventory)
    if($exit -ne 0 -or !$same){throw 'Build failed or source changed during compilation'}
    foreach($name in @('kilogram-peer.exe','kilogram-cli.exe','kilogram-bootstrap.exe')){$images[$name]=(Get-FileHash -LiteralPath (Join-Path $repo "target/verification-scale/$name")).Hash.ToLowerInvariant()}
}catch{$primary=$_.Exception.Message}
finally{
    $remaining=@($watch.Children());$details=@(foreach($id in $remaining){try{$entry=Get-Process -Id $id -ErrorAction Stop;@{pid=$id;name=$entry.ProcessName;path=$entry.Path;started=$entry.StartTime.ToFileTimeUtc()}}catch{@{pid=$id;error=$_.Exception.Message}}})
    @{pids=$remaining;details=$details}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $Evidence 'descendants.json') -Encoding UTF8
    $forced=$remaining.Count -gt 0;if($forced){$watch.CleanupChildren(6000)}
    $empty=@($watch.Children()).Count -eq 0;$watch.Finish();if($child){$child.Dispose()}
    @{v=1;exit=$exit;error=$primary;forced=$forced;empty_owned_job=$empty;source_unchanged=$same;source_inventory_sha256=(Get-FileHash -LiteralPath (Join-Path $Evidence 'source-inventory.json')).Hash.ToLowerInvariant();images=$images;profile='verification-scale';jobs=$jobs;priority='Idle';work_seconds=3300;whole_seconds=3360;build_script_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $Evidence 'build-receipt.json') -Encoding UTF8
}
if($primary -or $exit -ne 0 -or $forced -or !$empty -or !$same){throw ('Product build not confirmed: '+$primary)}
Get-Content -LiteralPath (Join-Path $Evidence 'build-receipt.json') -Raw
