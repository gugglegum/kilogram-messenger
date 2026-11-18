"""Run the interactive Setup gate on fresh A-only copies under ordinary Windows."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import time


SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parent
CONTROL = runpy.run_path(str(SCRIPTS / "test-terminal-t1-3e-interactive-setup-control.py"))
SOURCE = SCRIPTS / "test-terminal-t1-3e-interactive-setup.ps1"
LAUNCHER = SCRIPTS / "test-terminal-t1-3e-interactive-setup.cmd"
SYSTEM = Path(os.environ["SystemRoot"]) / "System32"
WINPS = SYSTEM / "WindowsPowerShell/v1.0/powershell.exe"
CMD = SYSTEM / "cmd.exe"
MARKER = b"fixture_private_stderr"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def winps(source):
    source = ("$ErrorActionPreference='Stop'\n"
              "$ProgressPreference='SilentlyContinue'\n"
              'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"\n'
              + source)
    encoded = base64.b64encode(source.encode("utf-16-le")).decode()
    result = subprocess.run([str(WINPS), "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded],
                            capture_output=True, timeout=12, check=True)
    assert not result.stderr, "unexpected PowerShell stderr"
    return json.loads(result.stdout)


def mailbox(folder, action):
    source = CONTROL["mailbox_program"](action).replace(CONTROL["FOLDER"], str(folder))
    source = source.replace("PAUL_HP_OMEN", os.environ["COMPUTERNAME"])
    source = "$ErrorActionPreference='Stop'\n" + source
    return CONTROL["public_response"](json.dumps(winps(source)))


def put(folder, raw):
    pending = folder / "fixture.pending"
    pending.write_bytes(raw)
    pending.rename(folder / "request.json")


def wait_ready(folder, process):
    end = time.monotonic() + 12
    while time.monotonic() < end:
        ready = folder / "ready.json"
        if ready.is_file():
            return json.loads(ready.read_bytes())
        if process.poll() is not None:
            break
        time.sleep(0.05)
    raise RuntimeError("launcher ready receipt absent")


def prepare(folder, images, token, forced=False, wrong_pin=False):
    binary = folder / "bin"
    binary.mkdir(parents=True)
    for name, path in images.items():
        if name == "kilogram-bootstrap.exe" and wrong_pin:
            (binary / name).write_bytes(b"synthetic tamper")
        else:
            os.link(path, binary / name)
    source = SOURCE.read_text(encoding="utf-8").replace(CONTROL["FOLDER"], str(folder)).replace(
        "'PAUL_HP_OMEN'", f"'{token['host']}'").replace(
        "'S-1-5-21-2671144489-3236106012-1032288181-1006'", f"'{token['sid']}'")
    source = source.replace("$wait.ElapsedMilliseconds -lt 300000", "$wait.ElapsedMilliseconds -lt 18000")
    source = source.replace("$wait.ElapsedMilliseconds -ge 300000", "$wait.ElapsedMilliseconds -ge 18000")
    if forced:
        fake = """
[Console]::Error.WriteLine('fixture_private_stderr')
[Console]::WriteLine('{"v":1,"type":"ready"}')
$null=[Console]::ReadLine()
Start-Sleep -Seconds 40
"""
        encoded = base64.b64encode(fake.encode("utf-16-le")).decode()
        source = source.replace("$info.WorkingDirectory = $Folder", f"""
        $info.FileName = '{WINPS}'
        $info.Arguments = '-NoProfile -NonInteractive -EncodedCommand {encoded}'
        $info.WorkingDirectory = $Folder
""").replace("[KilogramS5Deadline]::new(90, 110,", "[KilogramS5Deadline]::new(5, 15,")
    raw = source.encode("utf-8")
    (folder / "setup-smoke.ps1").write_bytes(raw)
    anchor = sha(SOURCE.read_bytes()).encode()
    launcher = LAUNCHER.read_bytes()
    assert launcher.count(anchor) == 1, "launcher source SHA differs"
    (folder / "Run-Setup-smoke.cmd").write_bytes(launcher.replace(anchor, sha(raw).encode()))


def start(folder):
    return subprocess.Popen([str(CMD), "/d", "/c", str(folder / "Run-Setup-smoke.cmd")],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def finish(process, expected):
    output, error = process.communicate(input=b"\r\n", timeout=28)
    if process.returncode != expected:
        raise ValueError(f"unexpected launcher exit {process.returncode}: {output!r}")
    assert MARKER not in output and not error, "private fixture output escaped"


def case(work, images, token, name, expected, request=None, forced=False, stale=False, wrong_pin=False):
    folder = work / name
    prepare(folder, images, token, forced=forced, wrong_pin=wrong_pin)
    if stale:
        put(folder, b"stale fixture")
    p = start(folder)
    try:
        if request is not None:
            ready = wait_ready(folder, p)
            assert ready["logon_type"] == 2 and not ready["session_zero"] and ready["pins"] == CONTROL["PINS"]
            if request == "controller":
                assert mailbox(folder, "status") == {"state": "ready", "nonce": ready["nonce"]}
                assert mailbox(folder, "submit") == {"state": "requested", "nonce": ready["nonce"]}
            else:
                put(folder, request)
        finish(p, expected)
    finally:
        if p.poll() is None:
            p.communicate(input=b"\r\n", timeout=25)
    result = folder / "result.json"
    if wrong_pin:
        assert not result.exists() and not (folder / "ready.json").exists()
        assert not (folder / "setup").exists()
        return {"ready_not_published": True, "child_not_started": True}
    raw = result.read_bytes()
    checked = mailbox(folder, "status")
    assert checked == {"state": "finished", "result": json.loads(raw)}
    assert b"fixture_private_stderr" not in raw
    assert b"recovery_words" not in raw
    if name == "actual":
        assert checked["result"]["outcome"] == "passed"
        assert (folder / "setup").is_dir()
        assert mailbox(folder, "submit") == checked
        finish(start(folder), 2)
        assert result.read_bytes() == raw and not (folder / "request.json").exists()
    elif forced:
        row = checked["result"]
        assert row["outcome"] == "failed" and row["forced_cleanup"] and row["work_expired"]
        assert row["child_started"] and row["job_member"] and row["job_empty"]
    else:
        row = checked["result"]
        assert row["outcome"] == "failed" and row["error"] == name and not row["child_started"]
        assert not (folder / "setup").exists()
    return checked["result"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="a fresh Windows workspace subdirectory")
    work = parser.parse_args().evidence.resolve()
    if ROOT not in work.parents:
        parser.error("evidence must be inside the workspace")
    work.mkdir(parents=True)
    token = winps("""
$who=[Security.Principal.WindowsIdentity]::GetCurrent()
@{host=[Environment]::MachineName;sid=$who.User.Value} | ConvertTo-Json -Compress
""")
    assert token["host"] == os.environ["COMPUTERNAME"] and token["host"] != "PAUL_HP_OMEN"
    artifacts = {
        "kilogram-cli.exe": ROOT / "target/verification/kilogram-cli.exe",
        "kilogram-bootstrap.exe": ROOT / "target/verification/kilogram-bootstrap.exe",
        "setup-pipe.cs": SCRIPTS / "test-terminal-t1-2e-setup-pipe.cs",
        "deadline.ps1": SCRIPTS / "migration-s5-l1-deadline.ps1",
        "dpapi-context.ps1": SCRIPTS / "test-terminal-t1-dpapi.ps1",
    }
    seed = work / "copies"
    seed.mkdir()
    images = {}
    for name, original in artifacts.items():
        images[name] = Path(shutil.copyfile(original, seed / name))
        assert sha(images[name].read_bytes()) == CONTROL["PINS"][name]
    wrong_nonce = b'{"v":1,"op":"setup_smoke","nonce":"00000000000000000000000000000000"}\n'
    results = {
        "actual": case(work, images, token, "actual", 0, request="controller"),
        "stale_files": case(work, images, token, "stale_files", 1, stale=True),
        "stale_nonce": case(work, images, token, "stale_nonce", 1, request=wrong_nonce),
        "malformed_request": case(work, images, token, "malformed_request", 1,
                                  request=b'{"v":1,"op":"setup_smoke","path":"injected"}\n'),
        "wrong_pin": case(work, images, token, "wrong_pin", 3, wrong_pin=True),
        "forced_cleanup": case(work, images, token, "forced_cleanup", 1, request="controller", forced=True),
    }
    (work / "checks.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(results))


if __name__ == "__main__":
    main()
