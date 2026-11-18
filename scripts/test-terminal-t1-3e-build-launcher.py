"""Build a reviewed single-file T1.3e launch CMD. Never runs Windows UAC."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
import re

WINPS = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
HOSTS = {"A": "DESKTOP-PAUL-RY", "B": "PAUL_HP_OMEN"}
HEX = re.compile("[a-f0-9]{64}")
ATTEMPT = re.compile("[a-z0-9][a-z0-9-]{0,30}")
SID = re.compile(r"S-1-5-21-(\d+-){3}\d+")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def build(source, side, attempt, sid, firewall_hash, audit_hash, policy_hash):
    if not ATTEMPT.fullmatch(attempt) or not SID.fullmatch(sid) or side not in HOSTS:
        raise ValueError("invalid fixed launch identity")
    if not all(HEX.fullmatch(pin) for pin in (firewall_hash, audit_hash, policy_hash)):
        raise ValueError("invalid SHA-256 pin")
    marker = "::T13E_PAYLOAD "
    identity = f"{side}-{attempt}"
    # The CMD passes its self path to the ordinary interpreter. Only the pinned
    # payload bytes and fixed launch arguments are sent across UAC.
    start = (
        "$ErrorActionPreference='Stop'; "
        "try { "
        f"$id='{identity}'; $mode=$env:KILOGRAM_T13E_MODE; "
        "if ($mode -cnotin @('Apply','Rollback')) { throw 'Invalid launch mode' }; "
        f"$rows=@([IO.File]::ReadAllLines($env:KILOGRAM_T13E_SELF) | Where-Object {{ $_.StartsWith('{marker}') }}); "
        "if ($rows.Count -ne 1) { throw 'Missing unique launcher payload' }; "
        f"$zip=[Convert]::FromBase64String($rows[0].Substring({len(marker)})); "
        "$m=[IO.MemoryStream]::new($zip); "
        "$z=[IO.Compression.GZipStream]::new($m,[IO.Compression.CompressionMode]::Decompress); "
        "$buffer=[IO.MemoryStream]::new(); $z.CopyTo($buffer); $z.Dispose(); $bytes=$buffer.ToArray(); "
        "$sha=[Security.Cryptography.SHA256]::Create(); "
        "$actual=[BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-','').ToLowerInvariant(); $sha.Dispose(); "
        f"if ($actual -cne '{digest(source)}') {{ throw 'Launcher payload SHA-256 mismatch' }}; "
        "$payload=[Text.Encoding]::UTF8.GetString($bytes); "
        "$invoke='& {'+[Environment]::NewLine+$payload+[Environment]::NewLine+'} -Mode '+$mode+"
        f"' -Side {side} -Attempt {attempt} -TestSid {sid} -FirewallSha {firewall_hash} "
        f"-AuditFileSha {audit_hash} -PolicySha {policy_hash}'; "
        "$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($invoke)); "
        "if ($encoded.Length -gt 30000) { throw 'Elevated command line length' }; "
        f"$p=Start-Process -FilePath '{WINPS}' -Verb RunAs -WindowStyle Hidden -Wait -PassThru "
        "-ArgumentList @('-NoProfile','-NonInteractive','-EncodedCommand',$encoded); exit $p.ExitCode "
        "} catch { "
        "$problem=$_; $path=Join-Path ([IO.Path]::GetTempPath()) "
        "('Kilogram-'+$id+'-'+[Guid]::NewGuid().ToString('N')+'-launch-error.json'); "
        "$json=@{utc=[DateTime]::UtcNow.ToString('o');stage='ordinary_launcher';"
        "error=$problem.Exception.Message;hresult=$problem.Exception.HResult;id=$problem.FullyQualifiedErrorId} "
        "| ConvertTo-Json -Compress; "
        "$b=[Text.Encoding]::UTF8.GetBytes($json); $f=[IO.File]::Open($path,'CreateNew','Write','None'); "
        "try {$f.Write($b,0,$b.Length);$f.Flush($true)}finally{$f.Dispose()}; "
        "[Console]::Error.WriteLine('Launch error receipt: '+$path); exit 1 }"
    )
    encoded = base64.b64encode(gzip.compress(source, mtime=0)).decode("ascii")
    lines = [
        "@echo off",
        "setlocal DisableDelayedExpansion",
        'set "KILOGRAM_T13E_MODE=Apply"',
        'if "%~1"=="" goto start',
        'if /I not "%~1"=="rollback" exit /b 2',
        'set "KILOGRAM_T13E_MODE=Rollback"',
        ":start",
        'set "KILOGRAM_T13E_SELF=%~f0"',
        f"echo Kilogram T1.3e {identity}. Confirm the Windows UAC prompt.",
        f'"{WINPS}" -NoProfile -NonInteractive -Command "{start}"',
        "set EXIT_STATUS=%ERRORLEVEL%",
        "if not %EXIT_STATUS%==0 (echo Kilogram exited with code %EXIT_STATUS%. & pause)",
        "exit /b %EXIT_STATUS%",
        marker + encoded,
    ]
    if any(len(line) >= 7800 for line in lines):
        raise ValueError("generated CMD line length")
    return ("\r\n".join(lines) + "\r\n").encode("ascii")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--side", required=True, choices=tuple(HOSTS))
    parser.add_argument("--attempt", required=True)
    parser.add_argument("--audit-file", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    inputs = parser.parse_args()
    if inputs.audit_file.name != f"{inputs.side}-{inputs.attempt}-audit.json":
        raise ValueError("expected fresh Audit receipt with the same attempt")
    audit_data = inputs.audit_file.read_bytes()
    report = json.loads(audit_data)
    policy = report["policy"]
    if policy["side"] != inputs.side or policy["host"] != HOSTS[inputs.side]:
        raise ValueError("Audit scope differs from the requested host")
    base = Path(__file__).resolve().parent
    source = (base / "test-terminal-t1-3e-admin-capture.ps1").read_bytes()
    firewall_hash = digest((base / "test-terminal-t1-3e-firewall.ps1").read_bytes())
    cmd = build(source, inputs.side, inputs.attempt, policy["test_sid"], firewall_hash, digest(audit_data), report["sha256"])
    with inputs.output.open("xb") as out:
        out.write(cmd)
    print(digest(cmd), inputs.output)


if __name__ == "__main__":
    main()
