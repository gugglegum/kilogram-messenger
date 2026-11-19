"""Exercise the real bounded host reader with synthetic rejection records; no nodes."""
import base64
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PS = r'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'


def main():
    folder = Path(sys.argv[1]).resolve()
    folder.mkdir(exist_ok=False)
    run = folder / 'runs/one'
    control = folder / 'ordinary-agent/control'
    diag = run / 'setup/lab-diagnostics-v1'
    diag.mkdir(parents=True)
    control.mkdir(parents=True)
    nonce, session = 'a' * 32, 'b' * 32

    def write(path, value):
        path.write_text(json.dumps(value), encoding='utf-8')

    write(run / 'attempt.json', dict(attempt='one', generation=nonce, session=session, pins={}))
    write(run / 'started.json', dict(generation=nonce, pid=123, started=134395772085441302))
    write(control / 'ready.json', dict(session=session))
    write(control / 'state.json', dict(session=session, active=None, blocked=None))
    write(diag / 'enabled.json', dict(schema=1, run_nonce=nonce))

    def record(kind, data, pid=42):
        return dict(schema=1, kind=kind, run_nonce=nonce, pid=pid,
                    runtime_pid=42, runtime_instance='c' * 64, data=data)

    histogram = dict(AnnouncementFuture=3, AnnouncementExpired=1,
                     AnnouncementDuration=0, Consent=0, Unknown=0)
    counters = dict(ListenerTlsError=2, ListenerPrerequisiteRejected=1,
                    ListenerActualTlsError=1, ListenerDeadline=0)
    good = record('runtime_final', dict(schema=1, enabled=True, elapsed_ms=100,
                                       discovery_rejections=histogram, counters=counters))
    write(diag / 'runtime-final-42.json', good)
    # An older record with no rejection histogram remains valid.
    write(diag / 'connection-final-44.json', record('connection_final', dict(ok=True), 44))
    write(diag / 'runtime-stopping-42.json', record('runtime_stopping',
          dict(discovery_rejections=dict(AnnouncementFuture='synthetic forbidden text'))))
    write(diag / 'runtime-network-stopped-42.json', record('runtime_network_stopped',
          dict(discovery_rejections=dict(ArbitraryFailure=1))))
    # Even valid fixed numeric keys must obey the binding to the runtime process.
    write(diag / 'runtime-final-43.json', record('runtime_final', good['data'], 43))
    request = dict(op='lab-diagnostic-read', base=str(folder), attempt='one',
                   agent_session=session, agent_generation=nonce, worker_pid=123,
                   worker_started=134395772085441302, pins={}, diagnostic_chat_pid=44,
                   diagnostic_runtime_pid=42, diagnostic_runtime_instance='c' * 64)
    write(folder / 'request.json', request)
    host = str(ROOT / 'scripts/test-terminal-t1-1f-host.ps1').replace("'", "''")
    req = str(folder / 'request.json').replace("'", "''")
    script = folder / 'read.ps1'
    script.write_text(
        'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"; '
        f"& ([scriptblock]::Create([IO.File]::ReadAllText('{host}'))) -Request ([IO.File]::ReadAllText('{req}'))",
        encoding='utf-8')
    result = subprocess.run([PS, '-NoProfile', '-NonInteractive', '-ExecutionPolicy',
                             'Bypass', '-File', str(script)], capture_output=True,
                            timeout=25, creationflags=subprocess.CREATE_NO_WINDOW | subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    (folder / 'stdout.bin').write_bytes(result.stdout)
    (folder / 'stderr.bin').write_bytes(result.stderr)
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    reply = json.loads(result.stdout)
    assert len(reply['records']) == 2 and len(reply['rejected']) == 3
    accepted = {r['name']: json.loads(base64.b64decode(r['data'], validate=True)) for r in reply['records']}
    assert accepted['runtime-final-42.json'] == good
    assert b'synthetic forbidden text' not in result.stdout
    assert b'ArbitraryFailure' not in result.stdout
    outcome = dict(schema=1, actual_reader=True, accepted=2, rejected=3,
                   histogram_roundtrip=True, older_record_accepted=True,
                   arbitrary_string_and_unknown_key_rejected=True, runtime_binding_enforced=True)
    write(folder / 'result.json', outcome)
    print(json.dumps(outcome))


if __name__ == '__main__':
    main()
