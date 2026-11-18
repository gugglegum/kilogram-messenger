"""Focused native ordinary-agent/Job/mailbox lifecycle checks; peers and EXEs are synthetic."""
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
import threading
import sys
from datetime import datetime, timezone
import uuid

SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parent
C = runpy.run_path(str(SCRIPTS / 'test-terminal-t1-3e-lab-control.py'))
M = runpy.run_path(str(SCRIPTS / 'test-terminal-t1-2e-mailbox.py'))
PS = str(Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe')
ENV = {k: v for k, v in os.environ.items() if k not in ('SSH_CONNECTION', 'SSH_TTY')}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(text):
    prelude = "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';"
    prelude += 'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";'
    return [PS, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand',
            base64.b64encode((prelude + text).encode('utf-16-le')).decode()]


def ps(text):
    p = subprocess.run(command(text), capture_output=True, env=ENV, timeout=30)
    if p.returncode or p.stderr:
        raise RuntimeError(f'check PowerShell exit {p.returncode}: {p.stderr[:1600]!r}')
    return json.loads(p.stdout)


def reject(body, reason):
    try:
        body()
    except RuntimeError as e:
        if reason in str(e):
            return
        raise
    raise AssertionError('not rejected: ' + reason)


class Host:
    def __init__(self, base, attempt, pins):
        self.base, self.attempt, self.pins = base, attempt, pins

    def command(self, op, **args):
        request = {'base': str(self.base), 'attempt': self.attempt, 'pins': self.pins, 'op': op, **args}
        wire = json.dumps(request, separators=(',', ':')).replace("'", "''")
        return command(f"& '{self.base / 'bin/host.ps1'}' -Request '{wire}'")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('evidence', type=Path)
    parser.add_argument('--startup-only', action='store_true', help='Focused launch-exception/retry coverage for source review r1')
    options=parser.parse_args()
    evidence = options.evidence.resolve()
    assert ROOT in evidence.parents and not evidence.exists()
    evidence.mkdir(parents=True)
    source_pins={p.name:sha(p) for p in SCRIPTS.glob('test-terminal-t1-3e-lab-*') if p.is_file()}
    (evidence/'source-pins.json').write_text(json.dumps(source_pins,indent=2)+'\n',encoding='utf-8')
    token = ps("@{host=[Environment]::MachineName;sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value} | ConvertTo-Json -Compress")
    base = evidence / 'host'
    home = base / 'ordinary-agent'
    binary = base / 'bin'
    home.mkdir(parents=True)
    binary.mkdir()
    for name in ('agent.ps1', 'common.ps1', 'job.cs', 'install.ps1'):
        shutil.copyfile(SCRIPTS / ('test-terminal-t1-3e-lab-' + name), home / ('test-terminal-t1-3e-lab-' + name))
    if options.startup_only:
        agent_file=home/'test-terminal-t1-3e-lab-agent.ps1'
        original=agent_file.read_text(encoding='utf-8')
        instrumented=original
        injections={
            '    $p = [Diagnostics.Process]::Start($info)':
                f"    if([IO.File]::Exists('{evidence/'before-create.fault'}')){{[IO.File]::Delete('{evidence/'before-create.fault'}');throw 'injected_before_create'}}\n    $p = [Diagnostics.Process]::Start($info)",
            '    $p.StandardInput.Close()':
                f"    $p.StandardInput.Close()\n    if([IO.File]::Exists('{evidence/'partial-create.fault'}')){{[IO.File]::Delete('{evidence/'partial-create.fault'}');throw 'injected_after_create_before_return'}}",
            '                                $job.CleanupChildren(6000)':
                f"                                if([IO.File]::Exists('{evidence/'cleanup-uncertain.fault'}')){{throw 'injected_cleanup_uncertain'}}\n                                $job.CleanupChildren(6000)",
        }
        for old,new in injections.items():
            assert instrumented.count(old)==1,old
            instrumented=instrumented.replace(old,new)
        agent_file.write_text(instrumented,encoding='utf-8',newline='\n')
        (evidence/'launch-fault-instrumentation.json').write_text(json.dumps({'scope':'copied launcher boundary only; no product test hooks',
            'canonical_sha256':sha(SCRIPTS/agent_file.name),'instrumented_sha256':sha(agent_file),'replacements':injections},indent=2)+'\n',encoding='utf-8')
    shutil.copyfile(SCRIPTS / 'test-terminal-t1-dpapi.ps1', home / 'dpapi-context.ps1')
    sources = {'host.ps1': 'test-terminal-t1-1f-host.ps1', 'deadline.ps1': 'migration-s5-l1-deadline.ps1',
               'mailbox.cs': 'test-terminal-t1-2e-mailbox.cs', 'setup-pipe.cs': 'test-terminal-t1-2e-setup-pipe.cs',
               'mailbox-fake.ps1': 'test-terminal-t1-2e-mailbox-fake.ps1'}
    for name, source in sources.items():
        shutil.copyfile(SCRIPTS / source, binary / name)
    for name in ('kilogram-cli.exe', 'kilogram-bootstrap.exe'):
        (binary / name).write_bytes(b'Not executable; synthetic suite never launches this file.\n')
    pins = {p.name: sha(p) for p in binary.iterdir()}
    config = home / 'lab.json'
    config.write_text(json.dumps({'v': 1, 'campaign': 'focused', 'side': 'A', **token, 'base': str(base),
                                 'ip': '127.0.0.1', 'peer': '127.0.0.2', 'interface_alias': 'synthetic',
                                 'interface_index': 1, 'interface_guid': str(uuid.uuid4()), 'suite': 'synthetic',
                                 'max_work_seconds': 120}), encoding='utf-8')
    agent = C['Agent']([PS], str(base), sha(home / 'test-terminal-t1-3e-lab-agent.ps1'), sha(config))
    process = None
    logs = []
    probes=[]
    def cutoff():
        if process is not None and process.poll() is None:
            process.kill()
        os._exit(124)
    timer = threading.Timer(240 if options.startup_only else 420, cutoff)
    timer.daemon = True
    timer.start()

    def start(previous=None):
        nonlocal process
        out = (evidence / f'agent-{len(logs)}.stdout.log').open('wb')
        err = (evidence / f'agent-{len(logs)}.stderr.log').open('wb')
        logs.append((out, err))
        process = subprocess.Popen(command(f"& '{home / 'test-terminal-t1-3e-lab-agent.ps1'}' -Action Run -Config '{config}'"),
                                   env=ENV, stdin=subprocess.DEVNULL, stdout=out, stderr=err)
        deadline = time.monotonic() + 16
        ready = home / 'control/ready.json'
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError('agent exited: ' + (evidence / f'agent-{len(logs)-1}.stderr.log').read_text(errors='replace'))
            if ready.exists():
                try:
                    row = json.loads(ready.read_bytes())
                except PermissionError:  # Windows Replace may briefly mark the old name delete-pending.
                    time.sleep(.05)
                    continue
                if row['session'] != previous:
                    return agent.ready()
            time.sleep(.1)
        raise TimeoutError('agent ready')

    def begin(attempt, budget=75):
        generation = uuid.uuid4().hex
        request = agent.request('begin', attempt=attempt, generation=generation, work_seconds=budget, pins=pins, baseline_sha256='')
        value = agent.exchange(request)
        assert agent.exchange(request) == value  # Lost-reply retry is idempotent.
        return generation, value

    def terminal(attempt, generation, mode='chat'):
        return M['MailboxTerminal'](Host(base, attempt, pins), 'setup' if mode == 'setup' else 'owner-one', mode,
                                    time.monotonic() + 65, interactive=C['Attempt'](agent, attempt, generation),
                                    fake_mode='valid' if mode == 'setup' else 'echo')

    results = {'scope': 'native local Windows Jobs and existing mailbox/Setup bridge; synthetic child only', 'cases': {}}
    try:
        if options.startup_only:
            ready=start();identity=(process.pid,agent.session)
            for marker,attempt,retry,created in (
                ('before-create.fault','failed-before','retry-before',False),
                ('partial-create.fault','failed-partial','retry-partial',True)):
                (evidence/marker).write_text('one launch',encoding='utf-8')
                generation=uuid.uuid4().hex
                request=agent.request('begin',attempt=attempt,generation=generation,work_seconds=45,pins=pins,baseline_sha256='')
                reject(lambda:agent.exchange(request),'LAB_LAUNCH_FAILED')
                reject(lambda:agent.exchange(request),'LAB_LAUNCH_FAILED')
                failure=agent.wait(attempt)
                diagnosis=json.loads((base/'runs'/attempt/'launch-error.json').read_bytes())
                assert failure['primary_error']=='worker_launch_failed' and failure['cleanup']['empty_job'] and failure['cleanup']['error'] is None,failure
                assert diagnosis['process_created'] is created,diagnosis
                if created: assert failure['cleanup']['forced'] is True
                assert (process.pid,agent.session)==identity,'agent must remain alive in the same session'
                results['cases'][attempt]=failure
                generation,worker=begin(retry,45)
                term=terminal(retry,generation)
                assert term.line(term.deadline)['type']=='ready'
                assert term.result('echo',{'text':'retry-without-restart'},term.deadline)['value']['text']=='retry-without-restart'
                term.quit();agent.call('end',attempt=retry,generation=generation)
                result=agent.wait(retry)
                assert result['state']=='closed' and result['cleanup']['empty_job'],result
                assert (process.pid,agent.session)==identity
                results['cases'][retry]=result
            (evidence/'partial-create.fault').write_text('one launch',encoding='utf-8')
            (evidence/'cleanup-uncertain.fault').write_text('refuse proof',encoding='utf-8')
            reject(lambda:agent.call('begin',attempt='uncertain',generation=uuid.uuid4().hex,work_seconds=8,pins=pins,baseline_sha256=''),'LAB_LAUNCH_FAILED')
            status=agent.call('status',attempt='uncertain')
            assert status['blocked']=='launch_ownership_unconfirmed' and not status['result']['cleanup']['empty_job'],status
            reject(lambda:agent.call('begin',attempt='must-not-start',generation=uuid.uuid4().hex,work_seconds=8,pins=pins,baseline_sha256=''),'LAB_BLOCKED')
            results['cases']['uncertain_cleanup_blocks_admission']=status['result']
            old_session=agent.session
            process.kill();process.wait(timeout=6)
            reject(lambda:start(old_session),'LAB_RECONCILE_UNKNOWN')
            proof=ps(f"Add-Type -Path '{home/'test-terminal-t1-3e-lab-job.cs'}';@{{empty=(@([KilogramLabJob]::Inspect('Local\\Kilogram-Lab-{old_session}')).Count -eq 0)}}|ConvertTo-Json -Compress")
            assert proof=={'empty':True},proof
            results['cases']['blocked_state_survives_restart_and_owned_job_empty']=proof
            results['scope']='focused native launch failures and retries; faults injected only in copied launch/cleanup boundaries'
            (evidence/'checks.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
            print(json.dumps({'result':'PASS','cases':len(results['cases']),'limit_seconds':240,'evidence':str(evidence)}))
            return
        guard_check=ps(f"Add-Type -Path '{home / 'test-terminal-t1-3e-lab-job.cs'}';"+r'''
$job=[KilogramLabJob]::new('Local\Kilogram-Lab-'+[guid]::NewGuid().ToString('N'))
$child=Start-Process -FilePath "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe" -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 30') -WindowStyle Hidden -PassThru
$guard=$job.Bound($child,500)
try {
    if(!$child.WaitForExit(5000)){throw 'guard failed'}
    $exit=$child.ExitCode;$expired=$guard.Expired;$guard.Dispose();$child.Dispose()
    $empty=@($job.Children()).Count -eq 0;$job.Dispose()
    @{exit=$exit;expired=$expired;empty=$empty}|ConvertTo-Json -Compress
} catch {if(!$child.HasExited){$child.Kill()};throw}
''')
        assert guard_check=={'exit':124,'expired':True,'empty':True},guard_check
        results['cases']['native_startup_outer_guard']=guard_check
        ready = start()
        assert not list((base / 'runs').iterdir()), 'logon must start idle'
        # Two sequential attempts through the same actual agent, including protected Setup filtering.
        for attempt, mode in (('first', 'setup'), ('second', 'chat')):
            generation, worker = begin(attempt)
            if attempt == 'first':
                reject(lambda: agent.call('begin', attempt='duplicate', generation=uuid.uuid4().hex, work_seconds=20, pins=pins, baseline_sha256=''), 'LAB_ACTIVE')
                reject(lambda: agent.call('update', stage=uuid.uuid4().hex, pins=pins), 'LAB_ACTIVE')
                reject(lambda: agent.call('owner', attempt=attempt, generation='0'*32, tag='bad', owner_generation=uuid.uuid4().hex, mode='chat', fake_mode='echo'), 'LAB_GENERATION')
            term = terminal(attempt, generation, mode)
            assert term.line(term.deadline)['type'] == 'ready'
            if mode == 'setup':
                assert term.result('create', {}, term.deadline)['value']['state'] == 'recovery_available'
            else:
                assert term.result('echo', {'text': 'ordinary-check'}, term.deadline)['value']['text'] == 'ordinary-check'
            term.quit()
            agent.call('end', attempt=attempt, generation=generation)
            result = agent.wait(attempt)
            assert result['state'] == 'closed' and result['cleanup']['empty_job'], result
            results['cases'][attempt] = result
        reject(lambda: agent.call('begin', attempt='first', generation=uuid.uuid4().hex, work_seconds=20, pins=pins, baseline_sha256=''), 'LAB_DUPLICATE_ATTEMPT')
        # No subsequent controller commands until after the local watchdog has expired.
        generation, worker = begin('lost-controller', 12)
        lost_term=terminal('lost-controller', generation)
        assert lost_term.line(lost_term.deadline)['type'] == 'ready'
        time.sleep(13)
        result = agent.wait('lost-controller')
        assert result['primary_error'] == 'work_timeout' and result['cleanup']['empty_job'] and result['cleanup']['forced'], result
        results['cases']['controller_loss'] = result
        # A failed owner must clean up and admit another attempt without restarting the agent.
        generation, worker = begin('failure')
        bad_term=terminal('failure',generation)
        assert bad_term.line(bad_term.deadline)['type']=='ready'
        # A valid mailbox frame with an unsupported synthetic-child operation causes a real child exit 1.
        bad_term.mailbox.request(bad_term.deadline,'submit',bad_term.cursor,1,M['frame'](1,'unsupported',{}))
        result = agent.wait('failure')
        assert result['state'] == 'failed' and result['primary_error']=='LAB_OWNER_FAILED' and result['cleanup']['empty_job'], result
        results['cases']['failure_cleaned'] = result
        generation, worker = begin('after-failure')
        term = terminal('after-failure', generation)
        assert term.line(term.deadline)['type'] == 'ready'
        term.quit(); agent.call('end', attempt='after-failure', generation=generation)
        assert agent.wait('after-failure')['state'] == 'closed'
        results['cases']['failure_recovery'] = True
        # Crash the exact agent while a real mailbox owner exists; inherited Job kills its descendants.
        generation, worker = begin('agent-crash')
        term = terminal('agent-crash', generation)
        assert term.line(term.deadline)['type'] == 'ready'
        old_session = agent.session
        process.kill(); process.wait(timeout=5)
        time.sleep(.5)
        new_ready = start(old_session)
        assert new_ready['session'] != old_session
        result = agent.call('status', attempt='agent-crash')['result']
        assert result['primary_error'] == 'agent_lost' and result['cleanup']['empty_job'], result
        results['cases']['agent_restart_reconciliation'] = result
        stale = agent.request('status', attempt='agent-crash'); stale['session'] = old_session
        reject(lambda: agent.exchange(stale), 'LAB_SESSION')
        generation, worker = begin('after-crash')
        term = terminal('after-crash', generation)
        assert term.line(term.deadline)['type'] == 'ready'
        term.quit(); agent.call('end', attempt='after-crash', generation=generation)
        assert agent.wait('after-crash')['state'] == 'closed'
        results['cases']['restart_recovery'] = True
        # Idle replacement actually changes one stable image. Active rejection was checked above.
        stage = uuid.uuid4().hex
        directory = base / 'staging' / stage
        directory.mkdir(parents=True)
        for name in pins:
            shutil.copyfile(binary / name, directory / name)
        (directory / 'kilogram-cli.exe').write_bytes(b'updated synthetic bytes\n')
        new_pins = {name: sha(directory / name) for name in pins}
        assert agent.call('update', stage=stage, pins=new_pins)['state'] == 'updated'
        assert sha(binary / 'kilogram-cli.exe') == new_pins['kilogram-cli.exe']
        results['cases']['idle_update_and_active_refusal'] = True
        agent.call('stop')
        process.wait(timeout=6)
        assert process.returncode == 0
        # Boundary injection: an on-disk update intent interrupted after one real replacement.
        stage2=base/'staging'/uuid.uuid4().hex
        stage2.mkdir()
        original_pins={name:sha(binary/name) for name in pins}
        update_pins=dict(original_pins)
        replacement=b'interrupted synthetic update\n'
        update_pins['kilogram-cli.exe']=hashlib.sha256(replacement).hexdigest()
        os.replace(binary/'kilogram-cli.exe',stage2/'kilogram-cli.exe.original')
        (binary/'kilogram-cli.exe').write_bytes(replacement)
        (stage2/'update.json').write_text(json.dumps({'state':'updating','names':list(pins),
            'changed':['kilogram-cli.exe'],'originals':original_pins,'pins':update_pins}),encoding='utf-8')
        previous=agent.session
        start(previous)
        assert sha(binary/'kilogram-cli.exe')==original_pins['kilogram-cli.exe']
        assert json.loads((stage2/'update.json').read_bytes())['state']=='rolled_back'
        results['cases']['interrupted_update_reconciled_native_files']=True
        agent.call('stop');process.wait(timeout=6);assert process.returncode==0
        # Boundary injection: an unknown still-live native Job must block restart, without killing it.
        probe_nonce=uuid.uuid4().hex
        proof=evidence/'live-job-probe.ready'
        probe=subprocess.Popen(command(f"Add-Type -Path '{home / 'test-terminal-t1-3e-lab-job.cs'}';"
            f"$j=[KilogramLabJob]::new('Local\\Kilogram-Lab-{probe_nonce}');"
            f"[IO.File]::WriteAllText('{proof}','ready');Start-Sleep -Seconds 30"),env=ENV,
            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        probes.append(probe)
        end=time.monotonic()+10
        while not proof.exists() and time.monotonic()<end:
            assert probe.poll() is None
            time.sleep(.1)
        assert proof.exists()
        state=home/'control/state.json'
        state.write_text(json.dumps({'session':probe_nonce,'active':None,'owner_count':0,'blocked':None}),encoding='utf-8')
        previous=agent.session
        reject(lambda:start(previous),'LAB_RECONCILE_LIVE_JOB')
        assert probe.poll() is None,'unknown Job must never be terminated by reconciliation'
        probe.kill();probe.wait(timeout=5)
        start(previous)
        results['cases']['unknown_live_job_blocks_admission']=True
        # The actual teardown admission helper stops only an idle agent and holds both locks.
        teardown=ps(f". '{home / 'test-terminal-t1-3e-lab-common.ps1'}';Add-Type -Path '{home / 'test-terminal-t1-3e-lab-job.cs'}';"
            "$h=@{agent=$null;images=$null};try{Stop-LabIdleAgent '"+str(base)+"' $h;"
            "$blocked=$false;try{$f=[IO.File]::Open('"+str(home/'control/agent.lock')+"','OpenOrCreate','Write','None');$f.Dispose()}catch{$blocked=$true};"
            "@{serialized=$blocked;held=($null -ne $h.agent -and $null -ne $h.images)}|ConvertTo-Json -Compress}finally{foreach($f in $h.Values){if($f){$f.Dispose()}}}")
        process.wait(timeout=6);assert process.returncode==0 and teardown=={'serialized':True,'held':True},teardown
        results['cases']['native_idle_teardown_stop_and_locks']=teardown
        installer = evidence / 'installer-checks.json'
        p = subprocess.run([PS, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(SCRIPTS / 'test-terminal-t1-3e-lab-install-check.ps1'),
                            '-Evidence', str(installer)], capture_output=True, env=ENV, timeout=35)
        (evidence / 'installer.stdout.log').write_bytes(p.stdout)
        (evidence / 'installer.stderr.log').write_bytes(p.stderr)
        assert p.returncode == 0 and not p.stderr, (p.returncode, p.stderr[:1600])
        results['installer'] = json.loads(installer.read_bytes())
        results['negative_protocol_checks'] = ['duplicate begin', 'idempotent exact lost-reply retry', 'stale session', 'stale generation', 'update while active']
        (evidence / 'checks.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')
        print(json.dumps({'result': 'PASS', 'cases': len(results['cases']), 'installer_cases': results['installer']['count'], 'evidence': str(evidence)}))
    finally:
        timer.cancel()
        if process is not None and process.poll() is None:
            process.kill(); process.wait(timeout=6)
        for probe in probes:
            if probe.poll() is None:
                probe.kill();probe.wait(timeout=6)
        for pair in logs:
            for file in pair:
                file.close()
        failure=sys.exc_info()[1]
        (evidence/'completion.json').write_text(json.dumps({'utc':datetime.now(timezone.utc).isoformat(),
            'ok':failure is None,'error':str(failure) if failure else None,'agent_exit':process.returncode if process else None,
            'completed_cases':results['cases']},indent=2)+'\n',encoding='utf-8')


if __name__ == '__main__':
    main()
