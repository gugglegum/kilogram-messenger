"""Native bounded readiness child, real interactive token/DPAPI, mocked policy boundary only."""
import hashlib
import base64
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / 'scripts'
PS = str(Path(os.environ['WINDIR']) / 'System32/WindowsPowerShell/v1.0/powershell.exe')
C = runpy.run_path(str(SCRIPTS / 'test-terminal-t1-3e-lab-control.py'))


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def main():
    evidence = Path(sys.argv[1]).resolve()
    assert ROOT in evidence.parents and not evidence.exists()
    home = evidence / 'host/ordinary-agent'
    home.mkdir(parents=True)
    identity = json.loads(subprocess.check_output([PS, '-NoProfile', '-NonInteractive', '-Command',
        '@{host=[Environment]::MachineName;sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value}|ConvertTo-Json -Compress']))
    for p in SCRIPTS.glob('test-terminal-t1-3e-lab-*'):
        if p.suffix in ('.ps1', '.cs'):
            shutil.copyfile(p, home / p.name)
    shutil.copyfile(SCRIPTS / 'test-terminal-t1-dpapi.ps1', home / 'dpapi-context.ps1')
    installer = home / 'test-terminal-t1-3e-lab-install.ps1'
    original = installer.read_text(encoding='utf-8')
    needle = 'function Assert-LabReadOnly($Cfg,[string]$Pin) {'
    injection = '''
    if(Test-Path -LiteralPath (Join-Path $Cfg.base 'fail.flag')){throw 'injected_policy_drift'}
    if(Test-Path -LiteralPath (Join-Path $Cfg.base 'hang.flag')){Start-Sleep -Seconds 80}
    return @{state='verified';baseline_sha256=$Pin;read_only=$true}
'''
    assert original.count(needle) == 1
    installer.write_text(original.replace(needle, needle + injection), encoding='utf-8', newline='\n')
    (evidence / 'instrumentation.json').write_text(json.dumps({'scope': __doc__, 'boundary': needle,
        'canonical_sha256': sha(SCRIPTS / installer.name), 'instrumented_sha256': sha(installer), 'injection': injection}, indent=2)+'\n')
    config = home / 'lab.json'
    base = home.parent
    config.write_text(json.dumps({'v': 1, 'campaign': 'ready-check', 'side': 'A', **identity, 'base': str(base),
        'ip': '127.0.0.1', 'peer': '127.0.0.2', 'interface_alias': 'fixture', 'interface_index': 1,
        'interface_guid': str(uuid.uuid4()), 'suite': 'field', 'max_work_seconds': 60}), encoding='utf-8')
    agent_file = home / 'test-terminal-t1-3e-lab-agent.ps1'
    agent = C['Agent']([PS], base, sha(agent_file), sha(config))
    cases = {}
    with (evidence / 'agent.stdout.log').open('wb') as out, (evidence / 'agent.stderr.log').open('wb') as err:
        process = subprocess.Popen([PS, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File',
            str(agent_file), '-Action', 'Run', '-Config', str(config)], stdin=subprocess.DEVNULL, stdout=out, stderr=err,
            creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
        try:
            end = time.monotonic() + 20
            while time.monotonic() < end:
                assert process.poll() is None, 'agent exited before ready'
                if (home / 'control/ready.json').exists():
                    ready = agent.ready()
                    break
                time.sleep(.1)
            else:
                raise TimeoutError('agent ready')
            session = ready['session']
            def check():
                value = agent.call('check', baseline_sha256='a'*64)
                assert value['dpapi'] is True and value['logon_type'] == 2 and value['windows_session'] > 0
                assert value['sid'] == identity['sid'] and value['session'] == session
                return value
            cases['real_interactive_dpapi_success'] = check()
            script = f"""
. '{installer}' -Mode Library
$cfg=Lab-Read '{config}'
$spec=Lab-TaskSpec $cfg
$script:taskObject=[pscustomobject]@{{Xml=$spec.xml;descriptor=$spec.sddl}}
$script:taskObject|Add-Member -MemberType ScriptMethod -Name GetSecurityDescriptor -Value {{param($flags) $this.descriptor}}
$script:taskObject|Add-Member -MemberType ScriptMethod -Name Run -Value {{param($arguments) return $null}}
function Find-LabTask([string]$Name){{$script:taskObject}}
Invoke-LabTaskProbe $cfg $spec ('a'*64)|ConvertTo-Json -Compress
"""
            proof = subprocess.run([PS, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand',
                base64.b64encode(script.encode('utf-16le')).decode()], capture_output=True, timeout=35)
            (evidence / 'task-probe.stderr.log').write_bytes(proof.stderr)
            assert proof.returncode == 0, proof.stderr[:1000]
            cases['setup_probe_real_agent_mock_task_run'] = json.loads(proof.stdout)
            for leaf in ('fail', 'hang'):
                flag = base / (leaf + '.flag')
                flag.write_text('copied fixture only')
                start = time.monotonic()
                try:
                    try:
                        check()
                    except RuntimeError as error:
                        assert 'LAB_READINESS_FAILED' in str(error), repr(error)
                    else:
                        raise AssertionError('expected probe failure')
                finally:
                    flag.unlink()
                elapsed = time.monotonic() - start
                assert elapsed < 24 and (leaf != 'hang' or elapsed >= 14), elapsed
                status = agent.call('status', attempt='unused')
                assert status['active'] is None and status['blocked'] is None and process.poll() is None
                assert agent.ready()['session'] == session
                cases[leaf + '_bounded_idle_retry'] = {'seconds': elapsed, 'retry': check()}
            agent.call('stop')
            assert process.wait(timeout=8) == 0
            (evidence / 'checks.json').write_text(json.dumps({'scope': __doc__, 'cases': cases, 'count': len(cases),
                'host_mutations': 0, 'agent_sha256': sha(agent_file)}, indent=2)+'\n')
            print(json.dumps({'passed': len(cases), 'host_mutations': 0}))
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)


if __name__ == '__main__':
    main()
