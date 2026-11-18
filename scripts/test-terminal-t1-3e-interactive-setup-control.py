"""One-shot B Setup smoke control. Run status or submit through the reviewed SSH pin."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess


ROOT = Path(__file__).resolve().parents[1]
FOLDER = r"C:\Users\test\KilogramLab\terminal-setup-interactive-smoke-01"
CONFIG = ROOT / ".tmp/terminal-t1-3e/final-t13e06/field-config.json"
CONFIG_SHA = "32f6a6aaf59f7f4c1821f3e7f310e83b8a5f2b289047ad1fde0ad44b32c989aa"
PINS = {
    "kilogram-cli.exe": "736827d50c3cfb4f34282b6651dccc82f25131c4c86a8594105771a38cda1ac2",
    "kilogram-bootstrap.exe": "dac789937cdb1743db49face9a5df23ddad89c33e649658ae33ad95ce45a9e7a",
    "setup-pipe.cs": "a738983b332563b186a91e65a0880303989d4d2048568c94ef006a94e8a6183c",
    "deadline.ps1": "4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5",
    "dpapi-context.ps1": "dedc8bb4d57fe1572bfa5620492d778b783769c66aa70a26c57d17c19a987a1d",
}

GUARD = r"""
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$who=[Security.Principal.WindowsIdentity]::GetCurrent()
try {
    if ([Environment]::MachineName -cne 'PAUL_HP_OMEN' -or
        $who.User.Value -cne 'S-1-5-21-2671144489-3236106012-1032288181-1006' -or
        $env:SSH_CONNECTION -notmatch '^192\.168\.0\.140 \d+ 192\.168\.0\.152 22$' -or
        ([Security.Principal.WindowsPrincipal]::new($who)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Wrong SSH context'
    }
} finally { $who.Dispose() }
"""

MAILBOX = r"""
$folder='@FOLDER@'
$action='@ACTION@'
$pins='@PINS@' | ConvertFrom-Json

function Plain([string]$Path) {
    $entry=Get-Item -LiteralPath $Path -Force
    while ($entry) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Unexpected reparse point' }
        $entry=if ($entry -is [IO.FileInfo]) { $entry.Directory } else { $entry.Parent }
    }
}

function Read-Public([string]$Path) {
    Plain $Path
    $file=[IO.File]::OpenRead($Path)
    try {
        if ($file.Length -lt 2 -or $file.Length -gt 4096) { throw 'Unexpected receipt size' }
        $reader=[IO.BinaryReader]::new($file)
        return ([Text.UTF8Encoding]::new($false,$true).GetString($reader.ReadBytes([int]$file.Length)) | ConvertFrom-Json)
    } finally { $file.Dispose() }
}

function Exchange {
    Plain $folder
    $ready=Join-Path $folder 'ready.json'
    $request=Join-Path $folder 'request.json'
    $accepted=Join-Path $folder 'request.claimed.json'
    $result=Join-Path $folder 'result.json'
    $dispatch=Join-Path $folder 'dispatch.claim'
    if ([IO.File]::Exists($result)) { return @{state='finished';result=(Read-Public $result)} }
    if (![IO.File]::Exists($ready)) { return @{state='not_ready'} }
    $r=Read-Public $ready
    if ($r.v -isnot [int] -or $r.v -ne 1 -or $r.host -cne 'PAUL_HP_OMEN' -or
        $r.nonce -isnot [string] -or $r.nonce -cnotmatch '\A[0-9a-f]{32}\z' -or
        $r.sid_verified -isnot [bool] -or !$r.sid_verified -or $r.logon_type -ne 2 -or
        $r.session_zero -ne $false -or $r.administrative -ne $false -or $r.profile_hive_loaded -ne $true -or
        @($r.pins.PSObject.Properties).Count -ne @($pins.PSObject.Properties).Count) { throw 'Unexpected ready receipt' }
    foreach ($pin in $pins.PSObject.Properties) {
        if ($r.pins.PSObject.Properties[$pin.Name].Value -cne $pin.Value) { throw 'Ready SHA differs' }
    }
    if ([IO.File]::Exists($accepted)) { return @{state='consumed';nonce=$r.nonce} }
    if ([IO.File]::Exists($request)) { return @{state='requested';nonce=$r.nonce} }
    if ([IO.File]::Exists($dispatch)) { return @{state='dispatch_used';nonce=$r.nonce} }
    if ($action -ceq 'status') { return @{state='ready';nonce=$r.nonce} }

    try { ([IO.File]::Open($dispatch,[IO.FileMode]::CreateNew)).Dispose() }
    catch {
        if (![IO.File]::Exists($dispatch)) { throw }
        return @{state='dispatch_used';nonce=$r.nonce}
    }
    if ([IO.File]::Exists($result) -or [IO.File]::Exists($accepted)) {
        return @{state='dispatch_used';nonce=$r.nonce}
    }
    $temp="$request.$([guid]::NewGuid().ToString('N')).pending"
    $data=[Text.Encoding]::ASCII.GetBytes(('{"v":1,"op":"setup_smoke","nonce":"' + $r.nonce + '"}') + [char]10)
    $file=[IO.File]::Open($temp,[IO.FileMode]::CreateNew)
    try {
        try { $file.Write($data,0,$data.Length); $file.Flush($true) } finally { $file.Dispose() }
        [IO.File]::Move($temp,$request)
    } finally { [IO.File]::Delete($temp) }
    return @{state='requested';nonce=$r.nonce}
}
(Exchange) | ConvertTo-Json -Depth 6 -Compress
"""

FLAGS = frozenset({
    "begin_ok", "create_ok", "status_ok", "quit_ok", "child_started", "job_member", "job_empty",
    "filtered_eof", "work_expired", "forced_cleanup",
})
ERRORS = frozenset({
    None, "stale_files", "mailbox_timeout", "malformed_request", "stale_nonce", "setup_exists",
    "mailbox_rejected", "job_start", "bridge_start", "child_start", "protected_exchange",
    "public_result", "job_cleanup", "forced_cleanup",
})
CODES = frozenset({
    None, "helper_unavailable", "helper_failed", "helper_timeout", "cleanup_unconfirmed", "cancelled",
    "setup_invalid", "invalid_state", "not_open", "invalid_request", "other_public_error",
})


def nonce(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{32}", value) is not None


def public_response(raw):
    if len(raw) > 8192:
        raise ValueError("oversized SSH response")
    data = json.loads(raw)
    state = data["state"]
    if state == "not_ready" and set(data) == {"state"}:
        return data
    if state in ("ready", "requested", "consumed", "dispatch_used"):
        if set(data) == {"state", "nonce"} and nonce(data["nonce"]):
            return data
    if state != "finished" or set(data) != {"state", "result"}:
        raise ValueError("unexpected public response")
    result = data["result"]
    if set(result) != FLAGS | {"v", "nonce", "outcome", "error", "create_code", "child_exit"}:
        raise ValueError("unexpected result keys")
    if (type(result["v"]) is not int or result["v"] != 1 or not nonce(result["nonce"])
            or result["outcome"] not in ("passed", "failed") or result["error"] not in ERRORS
            or result["create_code"] not in CODES or any(type(result[key]) is not bool for key in FLAGS)
            or (result["child_exit"] is not None and type(result["child_exit"]) is not int)):
        raise ValueError("unexpected result values")
    if result["outcome"] == "passed" and (
        result["error"] is not None or result["create_code"] is not None or result["child_exit"] != 0
        or result["forced_cleanup"] or result["work_expired"]
        or not all(result[key] for key in FLAGS - {"forced_cleanup", "work_expired"})
    ):
        raise ValueError("inconsistent success receipt")
    return data


def mailbox_program(action):
    if action not in ("status", "submit"):
        raise ValueError("unsupported mailbox action")
    return (MAILBOX.replace("@FOLDER@", FOLDER).replace("@ACTION@", action)
            .replace("@PINS@", json.dumps(PINS, separators=(",", ":"))))


def ssh_arguments(action):
    raw = CONFIG.read_bytes()
    if hashlib.sha256(raw).hexdigest() != CONFIG_SHA:
        raise ValueError("SSH configuration SHA differs")
    driver = runpy.run_path(str(ROOT / "scripts/test-terminal-t1-1f-driver.py"))
    ssh = driver["pinned_ssh"](json.loads(raw)["ssh"], "192.168.0.140", "192.168.0.152")
    packed = base64.b64encode(gzip.compress((GUARD + mailbox_program(action)).encode("utf-8"), mtime=0)).decode()
    loader = (
        f"$m=[IO.MemoryStream]::new([Convert]::FromBase64String('{packed}'));"
        "$z=[IO.Compression.GZipStream]::new($m,[IO.Compression.CompressionMode]::Decompress);"
        "$s=[IO.StreamReader]::new($z,[Text.Encoding]::UTF8);& ([ScriptBlock]::Create($s.ReadToEnd()))"
    )
    encoded = base64.b64encode(loader.encode("utf-16-le")).decode()
    if len(encoded) >= 7400:
        raise ValueError("encoded command length")
    return ssh + ["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("status", "submit"))
    action = parser.parse_args().action
    try:
        reply = subprocess.run(ssh_arguments(action), stdin=subprocess.DEVNULL,
                               capture_output=True, timeout=25)
        if reply.returncode or reply.stderr:
            raise RuntimeError(f"SSH mailbox exit {reply.returncode}")
        print(json.dumps(public_response(reply.stdout), separators=(",", ":")))
    except (OSError, ValueError, TypeError, KeyError, RuntimeError, subprocess.SubprocessError):
        parser.exit(1, "Protected SSH mailbox action failed. No retry was attempted.\n")


if __name__ == "__main__":
    main()
