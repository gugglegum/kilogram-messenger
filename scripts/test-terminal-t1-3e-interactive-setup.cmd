@echo off
setlocal DisableDelayedExpansion
set "KILOGRAM_SETUP_SMOKE_ENTRY=%~f0"
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -Command "$ErrorActionPreference='Stop'; try { [Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'; $folder=[IO.Path]::GetDirectoryName($env:KILOGRAM_SETUP_SMOKE_ENTRY); $path=[IO.Path]::Combine($folder,'setup-smoke.ps1'); $file=[IO.File]::OpenRead($path); $hash=[Security.Cryptography.SHA256]::Create(); try { if ([BitConverter]::ToString($hash.ComputeHash($file)).Replace('-','') -ine 'e1b649a21c0cbc7fc30f0492f35691d766ab49e006e7e6079f0fa22251192f89') { throw 'source pin' }; $source=[IO.File]::ReadAllText($path) } finally { $hash.Dispose(); $file.Dispose() }; exit (& ([ScriptBlock]::Create($source))) } catch { [Console]::WriteLine('Setup smoke could not start or save its result.'); exit 3 }"
set "KILOGRAM_SETUP_SMOKE_EXIT=%ERRORLEVEL%"
pause
exit /b %KILOGRAM_SETUP_SMOKE_EXIT%
