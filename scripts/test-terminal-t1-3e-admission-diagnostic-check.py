"""Local deterministic admission deadlines, evidence identity and serialized mailbox checks."""
import base64
import copy
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[1]
M=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-admission-diagnostic.py'))
EVIDENCE=Path(sys.argv[1]).resolve()
PS=r'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'

class Clock:
    def __init__(self,value=0):self.value=value
    def now(self):return self.value
    def sleep(self,seconds):self.value+=seconds

def frame(value,name=None):
    raw=json.dumps(value,separators=(',',':')).encode()
    result=dict(schema=1,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),data=base64.b64encode(raw).decode())
    if name:result['name']=name
    return result

def rows(at,live=False):
    return [dict(side=s,runtime_complete=True,runtime_received=at,live=live) for s in 'AB']

class Checks(unittest.TestCase):
    def gate(self,clock,probe,end=200):
        return M['admission_gate'](probe,lambda r:all(v['live'] for v in r),started=0,end=end,now=clock.now,sleep=clock.sleep)

    def test_just_before_boundary_passes_even_monitor_finishes_later(self):
        clock=Clock(85)
        def probe(end):clock.value=91;return rows(89.999,True)
        result=self.gate(clock,probe)
        self.assertTrue(result['admitted']);self.assertEqual(result['admitted_observed'],89.999)
        self.assertEqual(result['decision_at'],91)

    def test_at_and_after_boundary_never_pass(self):
        for at in (90,90.001,100):
            with self.subTest(at=at):
                clock=Clock(85)
                def probe(end):clock.value=at;return rows(at,True)
                result=self.gate(clock,probe)
                self.assertFalse(result['admitted']);self.assertEqual(result['after_deadline']['label'],'late_observation_not_admission')
                self.assertEqual(len(result['last_complete_runtime']),2)

    def test_slow_observer_retained_but_cannot_pass(self):
        clock=Clock(5)
        def probe(end):clock.value=end+1;return rows(6,True)
        result=self.gate(clock,probe)
        self.assertFalse(result['admitted']);self.assertEqual(result['raw_cause']['type'],'ObservationDeadline')
        self.assertEqual(result['last_complete_runtime']['A']['runtime_received'],6)
        self.assertTrue(result['after_deadline']['budget_exceeded'])

    def test_partial_observation_retains_both_previous_complete_samples(self):
        clock=Clock();calls=[]
        def probe(end):
            calls.append(end);clock.value+=3
            if len(calls)==1:return rows(clock.value)
            return [dict(side='A',runtime_complete=True,runtime_received=clock.value,live=True),dict(side='B',runtime_complete=False,error=dict(type='TimeoutExpired',timeout=3))]
        result=self.gate(clock,probe)
        self.assertFalse(result['admitted']);self.assertEqual(result['raw_cause']['type'],'IncompleteObservation')
        self.assertEqual(result['last_complete_runtime']['B']['runtime_received'],3)
        self.assertGreater(result['last_complete_runtime']['A']['runtime_received'],3)
        self.assertEqual(result['after_deadline']['label'],'after_failure_diagnostic_only')

    def test_tiny_phase_remainder_does_not_spawn_and_late_diagnostic_does_not_pass(self):
        clock=Clock(89.954);calls=[]
        def probe(end):calls.append(clock.value);clock.value+=3;return rows(clock.value,True)
        result=self.gate(clock,probe)
        self.assertEqual(calls,[90]);self.assertFalse(result['admitted'])
        self.assertEqual(result['raw_cause']['type'],'AdmissionDeadline')
        self.assertEqual(result['after_deadline']['label'],'after_deadline_diagnostic_only')
        with self.assertRaises(M['ObservationBudgetExhausted']):M['budget'](clock.value+.046,now=clock.now)

    def test_cleanup_reserve_is_hard_end(self):
        clock=Clock(29);calls=[]
        result=self.gate(clock,lambda end:calls.append(end),end=30)
        self.assertFalse(calls);self.assertFalse(result['admitted'])
        self.assertEqual(result['raw_cause']['type'],'CleanupReserve')
        self.assertEqual(result['after_deadline']['label'],'unavailable_cleanup_reserve')
        self.assertLessEqual(clock.value,30)

    def test_actual_mailbox_client_refuses_new_tiny_budget_process(self):
        client_class=M['N']['D']['MailboxTerminal'].__init__.__globals__['MailboxClient']
        host=SimpleNamespace(minimum_subprocess_seconds=2,command=lambda *a,**k:['never-started'])
        client=client_class(host,'test')
        with patch.object(subprocess,'run') as process:
            with self.assertRaisesRegex(RuntimeError,'tiny-budget'):client.request(time.monotonic()+.046)
            process.assert_not_called()

    def test_observation_count_is_bounded(self):
        clock=Clock()
        result=self.gate(clock,lambda end:rows(clock.value))
        self.assertEqual(len(result['rounds']),48);self.assertFalse(result['admitted'])
        self.assertEqual(result['raw_cause']['type'],'ObservationRoundCap')

    def test_raw_exception_and_post_failure_error_kept_separate(self):
        clock=Clock()
        def probe(end):clock.value+=2;raise subprocess.TimeoutExpired('observer',12,output=b'partial',stderr=b'raw')
        result=self.gate(clock,probe)
        self.assertEqual(result['raw_cause']['type'],'TimeoutExpired');self.assertEqual(result['raw_cause']['timeout'],12)
        self.assertEqual(result['after_deadline']['error']['stderr_sha256'],hashlib.sha256(b'raw').hexdigest())

    def test_full_status_exact_hash_and_runtime_binding(self):
        value=dict(version=2,runtime_pid=42,runtime_instance='a'*64,stats=dict(existing_counter=1))
        doc=dict(runtime_pid=42,runtime_instance='a'*64,full_status=frame(value))
        self.assertEqual(json.loads(M['full_status'](doc)),value)
        for field,changed in [('runtime_pid',43),('runtime_instance','b'*64)]:
            wrong=copy.deepcopy(doc);wrong[field]=changed
            with self.assertRaisesRegex(RuntimeError,'identity'):M['full_status'](wrong)
        wrong=copy.deepcopy(doc);wrong['full_status']['sha256']='0'*64
        with self.assertRaisesRegex(RuntimeError,'pin'):M['full_status'](wrong)
        wrong=copy.deepcopy(doc);wrong['full_status']['bytes']=49153
        with self.assertRaisesRegex(RuntimeError,'cap'):M['full_status'](wrong)

    def test_actual_full_status_observer_native_identity_and_cap(self):
        folder=EVIDENCE/'status-fixture';(folder/'bin').mkdir(parents=True);(folder/'runs/one/setup/connection').mkdir(parents=True)
        source=(ROOT/'scripts/test-terminal-t1-3e-observe.ps1').read_text()
        source=source.replace(r'E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f',str(folder)).replace("$ip = '192.168.0.140'","$ip = '127.0.0.1'")
        (folder/'observer.ps1').write_text(source,encoding='utf-8')
        code=r'''
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$base='@BASE@';$exe=Join-Path $base 'bin/kilogram-cli.exe';$child=$null
Add-Type -TypeDefinition 'public static class Fixture {public static void Main(string[] args){if(args.Length>1 && args[1]=="status2"){System.Console.Write(System.IO.File.ReadAllText(System.Reflection.Assembly.GetExecutingAssembly().Location+".json"));}else{System.Threading.Thread.Sleep(45000);}}}' -OutputAssembly $exe -OutputType ConsoleApplication
try{
 $child=[Diagnostics.Process]::Start($exe,'runtime-from-profile fixture');$child.PriorityClass='BelowNormal'
 $status=@{version=2;runtime_pid=$child.Id;runtime_instance=('a'*64);contexts=@();endpoint=$null;route=@{mode='direct_lan'};stats=@{existing_counter=7;lab_diagnostics_v1=@{schema=1;enabled=$true}};window=@{bytes=3};resources=@{live=1};backend=@{reads=2};root_generation=1}
 $json=ConvertTo-Json -InputObject $status -Depth 8 -Compress;[IO.File]::WriteAllText(($exe+'.json'),$json,[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText((Join-Path $base 'runs/one/setup/connection/ipc.json'),'fixture-not-a-real-descriptor')
 $request=@{side='A';host=[Environment]::MachineName;attempt='one';op='status';runtime_pid=$child.Id;runtime_started=$child.StartTime.ToFileTimeUtc();runtime_image=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant();full_status=$true}
 $observer=Join-Path $base 'observer.ps1';$reply=(& $observer -Request (ConvertTo-Json -InputObject $request -Compress))|ConvertFrom-Json
 if([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($reply.full_status.data)) -cne $json){throw 'Full original status lost'}
 $wrong=$false;$request.runtime_started++;try{$null=& $observer -Request (ConvertTo-Json -InputObject $request -Compress)}catch{$wrong=$true};$request.runtime_started--
 if(!$wrong){throw 'Wrong native incarnation accepted'}
 [IO.File]::WriteAllText(($exe+'.json'),('x'*49153));$cap=$false;try{$null=& $observer -Request (ConvertTo-Json -InputObject $request -Compress)}catch{$cap=$_.Exception.Message -match 'T13E_STATUS2_FAILED'}
 if(!$cap){throw 'Oversized status accepted'}
 @{reply=$reply;native_wrong_start_rejected=$wrong;status_cap_rejected=$cap;networking=$false}|ConvertTo-Json -Depth 15 -Compress
}finally{if($null -ne $child){if(!$child.HasExited){$child.Kill();$child.WaitForExit()};$child.Dispose()}}
'''.replace('@BASE@',str(folder))
        (folder/'check.ps1').write_text(code,encoding='utf-8')
        result=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(folder/'check.ps1')],capture_output=True,timeout=40)
        (folder/'stdout.bin').write_bytes(result.stdout);(folder/'stderr.bin').write_bytes(result.stderr)
        self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'))
        value=json.loads(result.stdout);raw=M['full_status'](value['reply'])
        self.assertEqual(json.loads(raw)['stats']['existing_counter'],7)
        self.assertTrue(value['status_cap_rejected']);self.assertTrue(value['native_wrong_start_rejected'])

    def test_shutdown_content_identity_and_missing_coverage(self):
        nonce='a'*32;instance='b'*64
        value=dict(schema=1,kind='connection_final',run_nonce=nonce,pid=44,runtime_pid=42,runtime_instance=instance,data=dict(ack=False,exit=1,eof=True,forced=False))
        reply=dict(schema=1,run_nonce=nonce,records=[frame(value,'connection-final-44.json')],rejected=[])
        records,coverage=M['shutdown_records'](reply,nonce,44,42,instance)
        self.assertEqual(len(records),1);self.assertFalse(coverage['complete']);self.assertIn('runtime_final',coverage['missing'])
        for key,new in [('run_nonce','c'*32),('pid',45),('runtime_pid',43),('runtime_instance','c'*64)]:
            wrong=copy.deepcopy(value);wrong[key]=new
            bad=dict(reply,records=[frame(wrong,'connection-final-44.json')])
            with self.subTest(key=key),self.assertRaises(RuntimeError):M['shutdown_records'](bad,nonce,44,42,instance)
        bad=dict(reply,records=[reply['records'][0]]*17)
        with self.assertRaisesRegex(RuntimeError,'scope'):M['shutdown_records'](bad,nonce,44,42,instance)

    def test_probe_serial_per_host_and_monitor_error_keeps_runtime(self):
        field=object.__new__(M['Diagnostic']);field.capture_errors=[];field.contexts=['c0','c1'];field.latest=[None,None];events=[]
        field.hosts=[SimpleNamespace(name=s) for s in 'AB'];field.pair=[]
        for i in (0,1):
            def status(end,index=i):
                events.append((index,'monitor'))
                if index:raise RuntimeError('monitor fixture failure')
                return dict(mode='owner',runtime_instance='a'*64,contexts=field.contexts),dict(state='watching')
            field.pair.append(SimpleNamespace(instance='a'*64,status=status))
        def sample(i,term,end):
            events.append((i,'runtime'));field.latest[i]=dict(received=time.monotonic());return {},{}
        field.sample=sample
        result=field.probe(time.monotonic()+24)
        for i in (0,1):self.assertEqual([e[1] for e in events if e[0]==i],['runtime','monitor'])
        self.assertTrue(all(row['runtime_complete'] for row in result));self.assertFalse(result[1]['monitor_complete'])
        self.assertEqual(len(field.capture_errors),1)

    def test_complete_status_without_publication_endpoint_is_retained(self):
        field=object.__new__(M['Diagnostic']);field.evidence=EVIDENCE/'before-publication';field.evidence.mkdir()
        field.plan=dict(pins={'kilogram-cli.exe':'a'*64});field.latest=[None,None];field.owner_tokens=[{},{}];field.roles=[1,0]
        host=SimpleNamespace(name='A',socket=lambda *args:True);field.hosts=[host]
        (field.evidence/'A-owner-a-1-socket.json').write_text(json.dumps(dict(pid=42,started=123,image='fixture.exe')))
        doc=dict(ready=True,runtime_pid=42,runtime_instance='b'*64,slots=[dict(role=1),dict(role=1)])
        field.observe=lambda *args,**kwargs:doc
        with patch.dict(M['D'],sample_endpoint=lambda *args:None):
            sample=field.sample(0,SimpleNamespace(tag='owner-a-1',instance='b'*64),time.monotonic()+24)
        self.assertEqual(sample,(doc,None));self.assertEqual(field.latest[0]['doc'],doc)
        self.assertIsNone(field.latest[0]['point'])

    def test_collection_failure_cannot_replace_application_cleanup_failure(self):
        field=object.__new__(M['Diagnostic']);field.evidence=EVIDENCE/'separate-cleanup';field.evidence.mkdir()
        field.processes=[];field.capture_errors=[];field.messages=[]
        def failed_run(_):
            (field.evidence/'result.json').write_text(json.dumps(dict(failure=dict(stage='admission'),cleanup=[dict(error='owner failed-stop')],agents=dict(B=dict(cleanup=dict(empty_job=True))))))
            raise RuntimeError('application cleanup failed')
        with patch.object(M['N']['Normal'],'run',failed_run):
            with self.assertRaisesRegex(RuntimeError,'application cleanup failed'):field.run()
        result=json.loads((field.evidence/'diagnostic-result.json').read_bytes())
        self.assertEqual(result['application_result']['cleanup'][0]['error'],'owner failed-stop')
        self.assertEqual(result['application_result']['agents']['B']['cleanup']['empty_job'],True)
        self.assertFalse(result['application_result'].get('clean_application_stop',False))

    def test_capture_failure_cannot_be_reported_as_complete_diagnostics(self):
        field=object.__new__(M['Diagnostic']);field.evidence=EVIDENCE/'incomplete-capture';field.evidence.mkdir()
        field.processes=[];field.capture_errors=[dict(side='B',error=dict(type='IncompleteShutdownDiagnostics'))];field.messages=[]
        def success(_):(field.evidence/'result.json').write_text(json.dumps(dict(failure=None,cleanup=[],agents={})))
        with patch.object(M['N']['Normal'],'run',success):
            with self.assertRaisesRegex(RuntimeError,'capture incomplete'):field.run()
        result=json.loads((field.evidence/'diagnostic-result.json').read_bytes())
        self.assertFalse(result['capture_complete']);self.assertIsNone(result['application_result']['failure'])

    def test_native_host_typed_receipts_completed_scope_and_rejection(self):
        base=EVIDENCE/'host-fixture';run=base/'runs/fresh-diag';control=base/'ordinary-agent/control';diag=run/'setup/lab-diagnostics-v1'
        control.mkdir(parents=True);diag.mkdir(parents=True)
        nonce='a'*32;session='b'*32;instance='c'*64;pins={'fixture':'d'*64};started=134395772085441302
        def write(path,value):path.write_text(json.dumps(value),encoding='utf-8')
        write(run/'attempt.json',dict(attempt='fresh-diag',generation=nonce,session=session,pins=pins))
        write(run/'started.json',dict(generation=nonce,pid=123,started=started))
        write(control/'ready.json',dict(session=session));write(control/'state.json',dict(session=session,active=None,blocked=None))
        write(diag/'enabled.json',dict(schema=1,run_nonce=nonce))
        value=dict(schema=1,kind='connection_final',run_nonce=nonce,pid=44,runtime_pid=42,runtime_instance=instance,data=dict(ack=False,exit=1,eof=True,forced=False))
        write(diag/'connection-final-44.json',value)
        write(diag/'worker-final-44.json',dict(value,kind='worker_final',data=dict(running_ok=False,stop_graceful=False)))
        write(diag/'runtime-final-42.json',dict(value,kind='runtime_final',pid=42,data=dict(network_ok=True,ipc_ok=True)))
        write(diag/'runtime-stopping-42.json',dict(value,kind='runtime_stopping',pid=42,data=dict(stage='private fixture text must never leave host')))
        req=dict(op='lab-diagnostic-read',base=str(base),attempt='fresh-diag',agent_session=session,agent_generation=nonce,worker_pid=123,worker_started=started,pins=pins,
            diagnostic_chat_pid=44,diagnostic_runtime_pid=42,diagnostic_runtime_instance=instance)
        def call(request):
            # Execute the actual source with only fresh local fixture metadata. No messenger/agent started.
            code='Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"; & '+"'"+str(ROOT/'scripts/test-terminal-t1-1f-host.ps1')+"' -Request '"+json.dumps(request).replace("'","''")+"'"
            result=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',base64.b64encode(code.encode('utf-16-le')).decode()],capture_output=True,timeout=25)
            label=str(len(list(base.glob('call-*.json')))+1)
            (base/f'call-{label}.stdout.bin').write_bytes(result.stdout);(base/f'call-{label}.stderr.bin').write_bytes(result.stderr)
            (base/f'call-{label}.json').write_text(json.dumps(dict(request=request,exit=result.returncode)))
            return result
        result=call(req);self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'))
        reply=json.loads(result.stdout);self.assertEqual(len(reply['records']),3);self.assertEqual(len(reply['rejected']),1)
        self.assertNotIn(b'private fixture text',result.stdout)
        wrong=call(dict(req,diagnostic_runtime_pid=43));self.assertEqual(wrong.returncode,0);self.assertEqual(len(json.loads(wrong.stdout)['records']),0)
        wrong=call(dict(req,agent_generation='e'*32));self.assertNotEqual(wrong.returncode,0)
        write(diag/'enabled.json',dict(schema=1,run_nonce='e'*32));wrong=call(req);self.assertNotEqual(wrong.returncode,0)
        (EVIDENCE/'host-fixture-success.json').write_bytes(result.stdout)
        fresh=base/'runs/fresh-enable';fresh.mkdir()
        write(fresh/'attempt.json',dict(attempt='fresh-enable',generation=nonce,session=session,pins=pins))
        write(fresh/'started.json',dict(generation=nonce,pid=123,started=started))
        write(control/'state.json',dict(session=session,active='fresh-enable',blocked=None))
        enable=dict(req,attempt='fresh-enable',op='lab-diagnostic-enable')
        missing=call(enable);self.assertNotEqual(missing.returncode,0)
        self.assertFalse((fresh/'setup').exists())
        (fresh/'setup').mkdir();write(fresh/'setup/setup.json',dict(test_fixture=True))
        success=call(enable);self.assertEqual(success.returncode,0,success.stderr.decode(errors='replace'))
        self.assertEqual(json.loads((fresh/'setup/lab-diagnostics-v1/enabled.json').read_bytes()),dict(schema=1,run_nonce=nonce))
        duplicate=call(enable);self.assertNotEqual(duplicate.returncode,0)

if __name__=='__main__':
    EVIDENCE.mkdir(parents=True,exist_ok=False)
    with (EVIDENCE/'test-output.txt').open('x',encoding='utf-8') as log:
        result=unittest.TextTestRunner(stream=log,verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(Checks))
    (EVIDENCE/'checks.json').write_text(json.dumps(dict(tests=result.testsRun,failures=len(result.failures),errors=len(result.errors),ok=result.wasSuccessful(),live_messenger=False,system_mutations=0),indent=2))
    print((EVIDENCE/'test-output.txt').read_text(),flush=True)
    sys.exit(0 if result.wasSuccessful() else 1)
