"""Focused mailbox correlation/preemption and pre-Session diagnostic boundaries; no live nodes."""
import base64,hashlib,json,runpy,subprocess,sys,time,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1]
M=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-admission-diagnostic.py'))
T=M['D']['MailboxTerminal'];E=Path(sys.argv[1]).resolve()
PS=r'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
class Clock:
    value=0
    def now(self):return self.value
    def sleep(self,n):self.value+=n
class Mailbox:
    def __init__(self,clock,connect_at=20,connect_ok=True,quit_at=5,uncertain=False,eof=False,failed=False,uncertain_before=False):
        self.clock=clock;self.connect_at=connect_at;self.connect_ok=connect_ok;self.quit_at=quit_at;self.uncertain=uncertain;self.eof=eof;self.failed=failed
        self.sent={};self.submits=[];self.scheduled=[];self.public=[dict(v=1,type='ready')];self.quit_submitted=False
        self.uncertain_before=uncertain_before
    def request(self,end,action='read',cursor=0,sequence=0,payload=b''):
        self.clock.value+=1
        if action=='submit':
            doc=json.loads(payload);self.submits.append((sequence,payload))
            if self.uncertain_before:self.uncertain_before=False;raise subprocess.TimeoutExpired('submit-before-delivery',15)
            if sequence in self.sent:assert self.sent[sequence]==payload
            else:
                self.sent[sequence]=payload;op=doc['op'];reply=dict(v=1,type='result',id=doc['id'],ok=True)
                if op=='connect':
                    at=self.connect_at;reply.update(ok=self.connect_ok,**({'value':dict(mode='owner',runtime_instance='a'*64,contexts=['c0','c1'])} if self.connect_ok else {'code':'cancelled'}))
                elif op=='quit':at=max(self.quit_at,self.clock.value);reply['value']=dict(connected=False);self.quit_submitted=True
                else:at=self.clock.value;reply.update(value=dict(status=dict(mode='owner')),monitor=dict(epoch=1,contexts=[]))
                self.scheduled.append((at,reply))
            if self.uncertain:self.uncertain=False;raise subprocess.TimeoutExpired('submit',15)
        elif action=='stop':self.failed=True
        ready=sorted([r for r in self.scheduled if r[0]<=self.clock.value],key=lambda r:r[0])
        for item in ready:self.public.append(item[1]);self.scheduled.remove(item)
        frames=self.public[cursor:cursor+8]
        closed=self.quit_submitted and self.clock.value>=max(self.quit_at,self.connect_at)+1 and not self.scheduled
        if self.eof and self.quit_submitted and self.clock.value>=self.quit_at+1:closed=True
        return dict(state='pending',cursor=cursor+len(frames),records=frames,terminal=('failed' if self.failed else 'clean') if closed else None),frames
def terminal(clock,**kwargs):
    owner=SimpleNamespace(wait=lambda end:0,check_alive=lambda end:None)
    host=SimpleNamespace(command=lambda *a,**k:['unused'])
    term=T(host,'owner-a-1','chat',200,interactive=SimpleNamespace(start=lambda *a,**k:owner))
    term.mailbox=Mailbox(clock,**kwargs);term.line(10)
    return term
def encoded(value,name):
    raw=json.dumps(value).encode();return dict(name=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),data=base64.b64encode(raw).decode())
class Checks(unittest.TestCase):
    def test_late_connect_success_before_status_is_preserved(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now),patch.object(time,'sleep',c.sleep):
            t=terminal(c)
            with self.assertRaises(RuntimeError):t.result('connect',dict(mode='start'),4)
            result=t.result('status',{},50)
            self.assertTrue(result['ok']);self.assertEqual([json.loads(b)['op'] for _,b in t.mailbox.submits],['connect','status'])
            self.assertTrue(t.requests['t12e-1']['reply']['ok']);self.assertEqual(t.requests['t12e-2']['reply']['id'],'t12e-2')
            self.assertEqual(t.command_log()[0]['reply'],t.requests['t12e-1']['reply']);self.assertIsNone(t.command_log()[1]['reply'])
            self.assertFalse(any(r['outstanding'] for r in t.command_log()))
    def test_late_cancelled_connect_before_status_is_not_status(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now):
            t=terminal(c,connect_ok=False)
            with self.assertRaises(RuntimeError):t.result('connect',dict(mode='start'),4)
            self.assertTrue(t.result('status',{},50)['ok']);self.assertEqual(t.requests['t12e-1']['reply']['code'],'cancelled')
    def test_late_connect_success_and_cancelled_both_orders_with_quit(self):
        for ok in (False,True):
            for at in (5,20):
                c=Clock()
                with self.subTest(ok=ok,connect_at=at),patch.object(time,'monotonic',c.now),patch.object(time,'sleep',c.sleep):
                    t=terminal(c,connect_ok=ok,connect_at=at,quit_at=10)
                    with self.assertRaises(RuntimeError):t.result('connect',dict(mode='start'),4)
                    t.quit();self.assertTrue(t.quit_verified)
                    self.assertEqual(t.requests['t12e-1']['reply']['ok'],ok);self.assertTrue(t.requests['t12e-2']['reply']['ok'])
                    self.assertEqual([json.loads(b)['op'] for _,b in t.mailbox.submits],['connect','quit'])
    def test_transport_uncertainty_reuses_same_connect_sequence_and_bytes(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now):
            t=terminal(c,uncertain=True)
            with self.assertRaises(subprocess.TimeoutExpired):t.result('connect',dict(mode='start'),40)
            self.assertTrue(t.result('connect',dict(mode='start'),50)['ok'])
            self.assertEqual(len(t.requests),1);self.assertEqual(t.mailbox.submits[0],t.mailbox.submits[1])
            self.assertEqual(t.command_log()[0]['errors'][0]['stage'],'submit')
    def test_different_connect_payload_is_rejected_while_unknown(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now):
            t=terminal(c)
            with self.assertRaises(RuntimeError):t.result('connect',dict(mode='start'),4)
            with self.assertRaisesRegex(RuntimeError,'different request'):t.result('connect',dict(mode='attach'),50)
            self.assertEqual(len(t.mailbox.submits),1)
    def test_quit_reconciles_uncertain_connect_delivery_without_skipping_sequence(self):
        for before in (False,True):
            c=Clock()
            with self.subTest(before_delivery=before),patch.object(time,'monotonic',c.now),patch.object(time,'sleep',c.sleep):
                t=terminal(c,uncertain=not before,uncertain_before=before,connect_at=10)
                with self.assertRaises(subprocess.TimeoutExpired):t.result('connect',dict(mode='start'),40)
                t.quit();self.assertTrue(t.quit_verified)
                self.assertEqual([seq for seq,_ in t.mailbox.submits],[1,1,2]);self.assertEqual(len(t.requests),2)
    def test_owner_agent_transport_cannot_overrun_its_wait_deadline(self):
        c=Clock();C=M['N']['C'];agent=C['Agent']([], 'fixture','a'*64,'b'*64);agent.session='c'*32
        request=agent.request('status',attempt='fixture');seen=[]
        def invoke(action,row,timeout):seen.append(timeout);return dict(v=1,id=row['id'],session=agent.session,ok=True,value={})
        agent.invoke=invoke
        with patch.object(time,'monotonic',c.now):
            self.assertEqual(agent.exchange(request,deadline=3),{});self.assertEqual(seen,[3])
            with self.assertRaisesRegex(TimeoutError,'tiny-budget'):agent.exchange(request,deadline=1)
            self.assertEqual(seen,[3])
    def test_status_reconciles_uncertain_delivery_with_the_original_connect(self):
        for before in (False,True):
            c=Clock()
            with self.subTest(before_delivery=before),patch.object(time,'monotonic',c.now):
                t=terminal(c,uncertain=not before,uncertain_before=before)
                with self.assertRaises(subprocess.TimeoutExpired):t.result('connect',dict(mode='start'),40)
                self.assertTrue(t.result('status',{},50)['ok'])
                self.assertEqual([seq for seq,_ in t.mailbox.submits],[1,1,2]);self.assertEqual(len(t.requests),2)
    def test_unknown_and_duplicate_reply_are_rejected(self):
        for duplicate in (False,True):
            c=Clock()
            with self.subTest(duplicate=duplicate),patch.object(time,'monotonic',c.now):
                t=terminal(c,connect_at=2);row=t.result('connect',dict(mode='start'),30)
                t.mailbox.scheduled.append((c.value,dict(row,id=row['id'] if duplicate else 'unknown')))
                with self.assertRaisesRegex(RuntimeError,'unknown or duplicate'):t.collect(50)
    def test_eof_cannot_acknowledge_outstanding_connect_or_clean_quit(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now),patch.object(time,'sleep',c.sleep):
            t=terminal(c,eof=True)
            with self.assertRaises(RuntimeError):t.result('connect',dict(mode='start'),4)
            with self.assertRaisesRegex(RuntimeError,'unresolved'):t.quit()
            self.assertFalse(t.quit_verified);self.assertTrue(t.command_log()[0]['outstanding'])
    def test_failed_terminal_is_not_clean_even_with_quit_result(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now),patch.object(time,'sleep',c.sleep):
            t=terminal(c,connect_at=2,failed=True);t.result('connect',dict(mode='start'),40)
            with self.assertRaisesRegex(RuntimeError,'streams failed'):t.quit()
            self.assertFalse(t.quit_verified)
    def test_close_fallback_does_not_convert_failed_terminal_to_clean(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now),patch.object(time,'sleep',c.sleep):
            t=terminal(c,connect_at=2,failed=True);t.result('connect',dict(mode='start'),40)
            self.assertFalse(t.close());self.assertFalse(t.quit_verified);self.assertTrue(t.stopped)
    def test_interleaved_events_are_retained_while_reconciling(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now):
            t=terminal(c)
            for at,char in [(10,'a'),(15,'b')]:t.mailbox.scheduled.append((at,dict(v=1,type='event',event='incoming',context='c0',runtime_instance='a'*64,epoch=1,event_id=char*64,text='synthetic fixture')))
            with self.assertRaises(RuntimeError):t.result('connect',dict(mode='start'),4)
            t.result('status',{},50);self.assertEqual([e['event_id'][0] for e in t.events],['a','b'])
    def test_wait_result_has_no_hidden_55_second_cap(self):
        c=Clock()
        with patch.object(time,'monotonic',c.now):
            t=terminal(c,connect_at=70);self.assertTrue(t.result('connect',dict(mode='start'),85)['ok']);self.assertGreaterEqual(c.value,70)
    def test_early_checkpoint_is_chat_bound_without_runtime_instance(self):
        nonce='a'*32;v=dict(schema=1,kind='startup_begin',run_nonce=nonce,pid=44,runtime_pid=None,runtime_instance=None,data=dict(elapsed_ms=0,ok=None,cancelled=False,spawned=False))
        reply=dict(schema=1,run_nonce=nonce,records=[encoded(v,'startup-begin-44.json')],rejected=[])
        records,coverage=M['shutdown_records'](reply,nonce,44,0,None);self.assertEqual(len(records),1);self.assertIn('startup_begin',coverage['startup_kinds']);self.assertFalse(coverage['complete'])
        for bad in [dict(v,pid=45),dict(v,runtime_instance='b'*64,runtime_pid=42)]:
            with self.assertRaises(RuntimeError):M['shutdown_records'](dict(reply,records=[encoded(bad,'startup-begin-44.json')]),nonce,44,0,None)
    def test_connect_reply_requires_real_owner_instance_and_context_binding(self):
        field=object.__new__(M['Diagnostic']);field.pair=[SimpleNamespace()];field.contexts=['c0','c1'];field.instances=[set()]
        with self.assertRaisesRegex(RuntimeError,'reply binding'):field.bind_connect(0,dict(connected=False))
        value=dict(mode='owner',contexts=field.contexts,runtime_instance='a'*64)
        field.bind_connect(0,value);field.bind_connect(0,value)
        self.assertEqual(field.pair[0].instance,'a'*64);self.assertEqual(len(field.instances[0]),1)
    def test_original_90_gate_does_not_pass_from_new_diagnostic_origin(self):
        c=Clock();c.value=91
        def probe(end):c.value+=2;return [dict(side=s,runtime_complete=True,runtime_received=c.value,live=True) for s in 'AB']
        original=M['admission_gate'](probe,lambda rows:True,started=0,end=400,now=c.now,sleep=c.sleep)
        self.assertFalse(original['admitted']);origin=c.value
        later=M['admission_gate'](probe,lambda rows:True,started=origin,end=400,now=c.now,sleep=c.sleep)
        self.assertTrue(later['admitted']);self.assertEqual(original['deadline'],90);self.assertNotEqual(later['deadline'],90)

    def functional_window_case(self,case,end=1000):
        c=Clock();field=object.__new__(M['Diagnostic']);field.evidence=E/('functional-window-'+case);field.evidence.mkdir()
        field.generations=['a'*32,'b'*32];field.processes=[];field.transfers=[];field.capture_errors=[];field.latest=[None,None];field.end=end
        field.hosts=[SimpleNamespace(name=side,call=lambda op,nonce=nonce:dict(schema=1,run_nonce=nonce,enabled=True)) for side,nonce in zip('AB',field.generations)]
        field.start=lambda *args:SimpleNamespace();field.receipt=lambda *args:None;field.select_route=lambda *args:None
        field.connect=lambda *args:setattr(c,'value',max(c.value,91))
        calls=[]
        def probe(limit):
            calls.append(dict(sent=c.value,deadline=limit));c.value+=1
            if case=='overrun' and len(calls)==1:c.value=limit+1
            if case=='thrown_error' and len(calls)==1:raise RuntimeError('synthetic after-deadline capture failure')
            rows=[dict(side=side,runtime_complete=True,runtime_received=c.value,monitor_complete=True,monitor_received=c.value,
                error=None,doc=dict(live=len(calls)>1),point=dict(address=side),chat_status=dict(mode='owner'),monitor=dict(contexts=[])) for side in 'AB']
            if len(calls)==1:
                if case=='returned_error':rows[0]['error']=dict(type='SyntheticCaptureError')
                elif case=='missing_host':rows.pop()
                elif case=='duplicate_host':rows[1]['side']='A'
                elif case=='missing_runtime':rows[0]['runtime_complete']=False
                elif case=='missing_monitor':rows[0]['monitor_complete']=False
                elif case=='missing_monitor_data':rows[0].pop('monitor')
                elif case=='host_timing':rows[0]['monitor_received']=limit+1
                elif case=='recorded_capture_error':field.capture_errors.append(dict(side='A',error=dict(type='SyntheticEarlierCaptureError')))
            return rows
        field.probe=probe
        actual_gate=M['admission_gate'];scope=M['Diagnostic'].functional.__globals__
        def gate(*args,**kwargs):return actual_gate(*args,**kwargs,now=c.now,sleep=c.sleep)
        with patch.object(time,'monotonic',c.now),patch.dict(scope,admission_gate=gate),patch.dict(M['D']['prior'],onboard=lambda *args:['c0','c1']),patch.dict(M['D'],live_pair=lambda docs,points:all(doc['live'] for doc in docs)):
            with self.assertRaises(M['AdmissionFailure']):field.functional()
        report=json.loads((field.evidence/'admission-result.json').read_bytes())
        self.assertFalse(report['original_window']['admitted']);self.assertEqual(report['original_window']['deadline'],90)
        self.assertEqual(report['original_window']['raw_cause']['type'],'AdmissionDeadline')
        self.assertEqual(report['original_window']['raw_cause']['message'],'four authenticated live leases not observed before deadline')
        self.assertFalse(report['normal_scenario_accepted'])
        return report,calls

    def test_additional_window_functional_capture_failures_skip(self):
        cases={'returned_error':'snapshot_host_capture_error','thrown_error':'snapshot_capture_error','overrun':'snapshot_budget_overrun',
            'missing_host':'snapshot_host_set_incomplete','duplicate_host':'snapshot_host_set_incomplete','missing_runtime':'snapshot_runtime_incomplete',
            'missing_monitor':'snapshot_monitor_incomplete','missing_monitor_data':'snapshot_monitor_incomplete','host_timing':'snapshot_host_timing_invalid',
            'recorded_capture_error':'capture_errors_recorded'}
        for case,reason in cases.items():
            with self.subTest(case=case):
                report,calls=self.functional_window_case(case)
                self.assertIsNone(report['network_diagnostic']);self.assertEqual(report['network_diagnostic_skip_reason'],reason)
                self.assertEqual(len(calls),1)

    def test_additional_window_functional_complete_negative_is_eligible(self):
        report,calls=self.functional_window_case('complete_negative')
        self.assertIsNone(report['network_diagnostic_skip_reason']);self.assertEqual(len(calls),2)
        self.assertFalse(report['original_window']['after_deadline']['hosts'][0]['doc']['live'])
        self.assertEqual(report['network_diagnostic']['label'],'post_original_window_network_diagnostic_only')
        self.assertGreater(report['network_diagnostic']['origin'],90);self.assertTrue(report['network_diagnostic']['result']['admitted'])

    def test_additional_window_functional_keeps_cleanup_reserve(self):
        report,calls=self.functional_window_case('cleanup_reserve',end=100)
        self.assertEqual(report['network_diagnostic_skip_reason'],'cleanup_reserve');self.assertIsNone(report['network_diagnostic'])
        self.assertEqual(len(calls),1)
    def test_old_B_ledger_confirms_delivery_barrier_not_an_internal_startup_cause(self):
        folder=ROOT/'.tmp/terminal-t1-3e/admission-diagnostic-owner-evidence-01/B'
        command=json.loads((folder/'receipts/mailbox-owner-a-1/command-0002.json').read_bytes())
        result=json.loads((folder/'receipts/mailbox-owner-a-1/public-0003.json').read_bytes())
        metadata=json.loads((folder/'collection.json').read_bytes())
        self.assertEqual(command['op'],'connect');self.assertEqual(result['id'],command['id']);self.assertEqual(result['code'],'cancelled')
        names={r['name'] for r in metadata['mailbox_files']}
        self.assertNotIn('command-0003.json',names);self.assertNotIn('command-0004.json',names)
        old=(ROOT/'.tmp/terminal-t1-3e/admission-diagnostic-review-02/package/sources/scripts/test-terminal-t1-2e-mailbox.cs').read_text()
        self.assertIn('seq == 1 || PlainFile(Numbered(root, "done", seq - 1))',old)
        self.assertIn('pending == null && cursor > 0',old)
        # Missing delivered commands and this gate support a cleanup confounder;
        # they cannot recover B's missing runtime checkpoint or inner error.
        self.assertIn('receipts/owner-a-1-socket.json',[r['path'] for r in metadata['files'] if r.get('missing')])
    def test_actual_host_reader_accepts_chat_only_stages_and_rejects_forged_binding(self):
        folder=E/'startup-reader';run=folder/'runs/one';control=folder/'ordinary-agent/control';diag=run/'setup/lab-diagnostics-v1'
        diag.mkdir(parents=True);control.mkdir(parents=True);nonce='a'*32;session='b'*32
        def write(p,v):p.write_text(json.dumps(v),encoding='utf-8')
        write(run/'attempt.json',dict(attempt='one',generation=nonce,session=session,pins={}))
        write(run/'started.json',dict(generation=nonce,pid=123,started=134395772085441302))
        write(control/'ready.json',dict(session=session));write(control/'state.json',dict(session=session,active=None,blocked=None))
        write(diag/'enabled.json',dict(schema=1,run_nonce=nonce))
        envelope=dict(schema=1,kind='startup_begin',run_nonce=nonce,pid=44,runtime_pid=None,runtime_instance=None,data=dict(elapsed_ms=0,ok=None,cancelled=False,spawned=False))
        write(diag/'startup-begin-44.json',envelope)
        write(diag/'startup-spawned-44.json',dict(envelope,kind='startup_spawned',runtime_pid=42))
        write(diag/'startup-status-44.json',dict(envelope,kind='startup_status',runtime_pid=42,runtime_instance='c'*64))
        write(diag/'startup-setup-open-45.json',dict(envelope,kind='startup_setup_open',pid=45))
        write(diag/'startup-import-begin-44.json',dict(envelope,kind='startup_import_begin',runtime_pid=42,runtime_instance='c'*64))
        write(diag/'startup-import-end-44.json',dict(envelope,kind='startup_import_end',data=dict(ok='private fixture marker')))
        req=dict(op='lab-diagnostic-read',base=str(folder),attempt='one',agent_session=session,agent_generation=nonce,worker_pid=123,worker_started=134395772085441302,pins={},diagnostic_chat_pid=44,diagnostic_runtime_pid=0,diagnostic_runtime_instance=None)
        write(folder/'request.json',req)
        script=folder/'read.ps1';script.write_text("Import-Module \"$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1\"; & ([scriptblock]::Create([IO.File]::ReadAllText('"+str(ROOT/'scripts/test-terminal-t1-1f-host.ps1')+"'))) -Request ([IO.File]::ReadAllText('"+str(folder/'request.json')+"'))",encoding='utf-8')
        result=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(script)],capture_output=True,timeout=25)
        (folder/'stdout.bin').write_bytes(result.stdout);(folder/'stderr.bin').write_bytes(result.stderr)
        self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'));reply=json.loads(result.stdout)
        self.assertEqual(len(reply['records']),3);self.assertEqual(len(reply['rejected']),3);self.assertNotIn(b'private fixture marker',result.stdout)
        records,coverage=M['shutdown_records'](reply,nonce,44,0,None);self.assertEqual(len(records),3);self.assertFalse(coverage['complete'])
    def test_native_csharp_bridge_correlates_preempting_quit_and_rejects_others(self):
        folder=E/'native-bridge';folder.mkdir()
        code=r'''
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path '@SOURCE@' -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'
$type=[KilogramTestMailbox];$flags=[Reflection.BindingFlags]'Instance,NonPublic';$publish=$type.GetMethod('PublicFrame',$flags);$next=$type.GetMethod('Next',$flags)
function Frame($box,$doc){$json=$doc|ConvertTo-Json -Depth 8 -Compress;$null=$publish.Invoke($box,[object[]]@($json.PSObject.BaseObject,$false))}
function Submit($root,$seq,$op){$bytes=[Text.Encoding]::UTF8.GetBytes((@{v=1;id=('id'+$seq);op=$op;args=@{}}|ConvertTo-Json -Compress));$h=[Security.Cryptography.SHA256]::Create();try{$digest=([BitConverter]::ToString($h.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()}finally{$h.Dispose()};[KilogramTestMailbox]::Exchange($root,('a'*32),'submit',$seq,$digest,0,[IO.StringReader]::new([Convert]::ToBase64String($bytes)+"`n"))}
$cases=@()
foreach($connectOk in @($false,$true)){foreach($quitFirst in @($false,$true)){
 $root=Join-Path '@FOLDER@' ('case-'+$quitFirst+'-'+$connectOk);$box=[KilogramTestMailbox]::new($root,('a'*32),([Environment]::TickCount+120000));Frame $box @{v=1;type='ready'}
 $current=[Diagnostics.Process]::GetCurrentProcess();$null=Submit $root 1 'connect';$null=$next.Invoke($box,[object[]]@($current.PSObject.BaseObject))
 $refused=$false;try{$null=Submit $root 2 'status'}catch{$refused=$true};if(!$refused -or (Test-Path -LiteralPath (Join-Path $root 'command-0002.json'))){throw 'Unrelated pending command accepted'}
 $null=Submit $root 2 'quit';$null=$next.Invoke($box,[object[]]@($current.PSObject.BaseObject))
 $order=if($quitFirst){@(2,1)}else{@(1,2)}
 foreach($id in $order){Frame $box @{v=1;type='result';id=('id'+$id);ok=($id -eq 2 -or $connectOk);code=if($id -eq 1){'cancelled'}else{'fixture'};value=@{connected=$false}};if($id -eq 1 -and !$quitFirst -and $box.QuitAcknowledged){throw 'Connect success mistaken for quit acknowledgement'}}
 if(!$box.QuitAcknowledged -or !(Test-Path -LiteralPath (Join-Path $root 'done-0001.json')) -or !(Test-Path -LiteralPath (Join-Path $root 'done-0002.json'))){throw 'Wrong completed sequence/quit acknowledgement'}
 $duplicate=$false;try{Frame $box @{v=1;type='result';id='id1';ok=$false;code='cancelled'}}catch{$duplicate=$true};if(!$duplicate){throw 'Duplicate reply accepted'}
 $unknown=$false;try{Frame $box @{v=1;type='result';id='unknown';ok=$true}}catch{$unknown=$true};if(!$unknown){throw 'Unknown reply accepted'}
 $cases+=@{quit_first=$quitFirst;connect_ok=$connectOk;unknown_rejected=$unknown;duplicate_rejected=$duplicate;unrelated_rejected=$refused;both_done=$true}
}}
@{cases=$cases;networking=$false}|ConvertTo-Json -Depth 5 -Compress
'''.replace('@SOURCE@',str(ROOT/'scripts/test-terminal-t1-2e-mailbox.cs')).replace('@FOLDER@',str(folder))
        script=folder/'check.ps1';script.write_text(code,encoding='utf-8')
        result=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(script)],capture_output=True,timeout=35)
        (folder/'stdout.bin').write_bytes(result.stdout);(folder/'stderr.bin').write_bytes(result.stderr)
        self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'));self.assertEqual(len(json.loads(result.stdout)['cases']),4)
if __name__=='__main__':
    E.mkdir(parents=True,exist_ok=False)
    names=[name for name in unittest.defaultTestLoader.getTestCaseNames(Checks) if '--eligibility-only' not in sys.argv or name.startswith('test_additional_window_') or name=='test_original_90_gate_does_not_pass_from_new_diagnostic_origin']
    with (E/'test-output.txt').open('x',encoding='utf-8') as log:result=unittest.TextTestRunner(stream=log,verbosity=2).run(unittest.TestSuite(Checks(name) for name in names))
    (E/'checks.json').write_text(json.dumps(dict(tests=result.testsRun,names=names,ok=result.wasSuccessful(),failures=len(result.failures),errors=len(result.errors),live_nodes=False),indent=2));print((E/'test-output.txt').read_text(),flush=True)
    sys.exit(0 if result.wasSuccessful() else 1)
