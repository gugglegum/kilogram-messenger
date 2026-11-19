"""Normal adapter boundary/lifecycle checks. Synthetic messaging; real local file/token/clock boundaries."""
import base64
import ctypes
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[1]
N=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-normal.py'))
PS=r'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
EVIDENCE=Path(sys.argv[1]).resolve()
if EVIDENCE.exists():raise RuntimeError('fresh evidence required')
EVIDENCE.mkdir(parents=True)
def save(name,value):
    with (EVIDENCE/name).open('x',encoding='utf-8') as f:json.dump(value,f,indent=2);f.write('\n')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def ps(text):
    text="$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';"+text
    text='Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";'+text
    p=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',base64.b64encode(text.encode('utf-16-le')).decode()],capture_output=True,timeout=35)
    return p

class Checks(unittest.TestCase):
    def test_local_deadline_binding_and_reserve(self):
        identity=dict(attempt='one',generation='a'*32,session='b'*32,worker_pid=12,worker_started=123)
        clock=identity|dict(start_tick=1000,tick=2000,work_tick=101000,whole_tick=116000)
        self.assertEqual(N['deadline'](clock,identity,100,10,12),49)
        for delta in ({'session':'x'},{'worker_started':124},{'tick':101000},{'start_tick':0},{'whole_tick':117000},{'work_tick':101001},{'tick':2.0}):
            with self.subTest(delta=delta),self.assertRaises(RuntimeError):N['deadline'](clock|delta,identity,100,10,12)
        save('clock-boundaries.json',dict(passed=True,local_deadline=True,no_synthetic_gate=True))

    def test_immutable_transport_retry_only(self):
        agent=N['Agent']([],r'C:\fixture','a'*64,'b'*64,ledger=EVIDENCE/'requests');agent.session='c'*32
        request=agent.request('begin',attempt='one');calls=[]
        def exchange(self,value):
            calls.append(json.dumps(value,sort_keys=True))
            if len(calls)==1:raise subprocess.TimeoutExpired('SSH',1)
            return {'accepted':True}
        with patch.object(N['C']['Agent'],'exchange',exchange):self.assertTrue(agent.exchange(request)['accepted'])
        self.assertEqual(calls[0],calls[1]);self.assertEqual(len(list(agent.ledger.iterdir())),1)
        calls.clear()
        def denied(self,value):calls.append(value);raise RuntimeError('LAB_ACTIVE')
        with patch.object(N['C']['Agent'],'exchange',denied),self.assertRaisesRegex(RuntimeError,'LAB_ACTIVE'):agent.exchange(request)
        self.assertEqual(len(calls),1)

    def test_no_fault_entrypoint(self):
        instance=N['Normal'].__new__(N['Normal'])
        for name in ('gate','rollback'):
            with self.assertRaisesRegex(RuntimeError,'forbidden'):getattr(instance,name)()

    def test_native_host_admission_clock_and_token(self):
        who=ps("@{host=[Environment]::MachineName}|ConvertTo-Json -Compress")
        self.assertEqual(who.returncode,0);host=json.loads(who.stdout)['host']
        ctypes.windll.kernel32.GetTickCount64.restype=ctypes.c_ulonglong
        ticks=ctypes.windll.kernel32.GetTickCount64()
        results=[]
        for scenario in ('valid','session','generation','worker','active','blocked','pins'):
            base=EVIDENCE/scenario;run=base/'runs/one';control=base/'ordinary-agent/control'
            (run/'in').mkdir(parents=True);(run/'receipts').mkdir();control.mkdir(parents=True);(base/'bin').mkdir()
            for name in N['NAMES']:
                data=(ROOT/'scripts/migration-s5-l1-deadline.ps1').read_bytes() if name=='deadline.ps1' else b'fixture-only-not-executable'
                (base/'bin'/name).write_bytes(data)
            pins={name:sha(base/'bin'/name) for name in N['NAMES']}
            request=dict(base=str(base),attempt='one',op='lab-init',pins=pins,hostname=host,ip='127.0.0.1',ssh_peer=None,
                agent_session='a'*32,agent_generation='b'*32,worker_pid=123,worker_started=134395772085441302)
            entry=dict(attempt='one',session='a'*32,generation='b'*32,pins=pins,start_tick=ticks)
            started=dict(pid=123,started=request['worker_started'],generation='b'*32,work_tick=ticks+100000,whole_tick=ticks+115000)
            state=dict(session='a'*32,active='one',blocked=None)
            if scenario=='session':request['agent_session']='c'*32
            if scenario=='generation':request['agent_generation']='c'*32
            if scenario=='worker':request['worker_started']+=1
            if scenario=='active':state['active']='other'
            if scenario=='blocked':state['blocked']='unknown'
            if scenario=='pins':entry['pins']=pins|{'extra':'f'*64}
            for path,value in ((run/'attempt.json',entry),(run/'started.json',started),(control/'state.json',state),(control/'ready.json',{'session':'a'*32})):
                path.write_text(json.dumps(value),encoding='utf-8')
            text=f"& '{ROOT/'scripts/test-terminal-t1-1f-host.ps1'}' -Request '{json.dumps(request)}'"
            outcome=ps(text);(base/'stderr.log').write_bytes(outcome.stderr)
            claim=base/'runs/one.claim.json'
            if scenario!='valid':
                self.assertNotEqual(outcome.returncode,0,scenario);self.assertFalse(claim.exists());results.append(dict(case=scenario,rejected_before_write=True));continue
            self.assertEqual(outcome.returncode,0,outcome.stderr.decode(errors='replace'));self.assertTrue(claim.exists())
            before={str(p.relative_to(base)):sha(p) for p in base.rglob('*') if p.is_file()}
            request['op']='lab-clock';outcome=ps(f"& '{ROOT/'scripts/test-terminal-t1-1f-host.ps1'}' -Request '{json.dumps(request)}'")
            self.assertEqual(outcome.returncode,0,outcome.stderr.decode(errors='replace'))
            clock=json.loads(outcome.stdout);self.assertEqual(clock['work_tick'],ticks+100000);self.assertGreaterEqual(clock['tick'],ticks)
            self.assertEqual(before,{str(p.relative_to(base)):sha(p) for p in base.rglob('*') if p.is_file()})
            results.append(dict(case=scenario,clock_read_only=True,admission=True))
        observer=(ROOT/'scripts/test-terminal-t1-3e-observe.ps1').read_text(encoding='utf-8');pos=observer.index('public static class T13eToken')
        token=observer[observer.rfind('using System;',0,pos):observer.index("\n'@",pos)]
        token_code="Add-Type -TypeDefinition @'\n"+token+"\n'@\n"+r'''
$p=Get-Process -Id $PID;$id=[T13eToken]::Read($PID,$p.StartTime.ToFileTimeUtc())
try{$value=@{sid=$id.User.Value;admin=([Security.Principal.WindowsPrincipal]::new($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)}}finally{$id.Dispose()}
$wrong=$false;try{$id=[T13eToken]::Read($PID,($p.StartTime.ToFileTimeUtc()+1));$id.Dispose()}catch{$wrong=$true}
if(!$wrong){throw 'Wrong process incarnation accepted'};$value|ConvertTo-Json -Compress
'''
        outcome=ps(token_code);self.assertEqual(outcome.returncode,0,outcome.stderr.decode(errors='replace'))
        self.assertFalse(json.loads(outcome.stdout)['admin'])
        save('native-boundaries.json',dict(cases=results,current_process_token=json.loads(outcome.stdout),wrong_incarnation_refused=True,
            token_mask='QUERY|DUPLICATE',real_task_or_firewall_mutations=0,synthetic_worker_files=True))

    def test_actual_functional_order_with_synthetic_terminals(self):
        class Term:
            def __init__(self,tag,owner):self.tag=tag;self.owner=owner;self.events=[];self.live=True;self.watch=False
            def call(self,op,end=None,**args):
                assert op=='retry';row=next(r for r in self.owner.messages if r['key']==args)
                return dict(state='delivered',event_id=row['event_id'])
        class Model(N['Normal']):
            def __init__(self):
                self.contexts=['0','1'];self.hosts=[SimpleNamespace(name='A'),SimpleNamespace(name='B')];self.evidence=EVIDENCE
                self.transfers=[];self.messages=[];self.processes=[];self.pair=[None,None];self.end=time.monotonic()+90;self.log=[]
            def start(self,i,tag,mode,end=None):
                term=Term(tag,self);self.processes.append((i,term));self.log.append(('start',i,tag));return term
            def receipt(self,*args,**kwargs):pass
            def select_route(self,*args):pass
            def connect(self,i,warm):self.log.append(('connect',i,warm))
            def sample(self,*args):return ({},{'port':50000})
            def port_gone(self,*args):pass
            def stop(self,i,term):term.live=False;self.log.append(('stop',i,term.tag))
            def admit(self,label,started):
                assert all(t.live for t in self.pair)
                for term in self.pair:term.watch=True
                self.log.append(('watching',label))
            def send(self,i,slot,label,end):
                assert self.pair[i].live and self.pair[i].watch
                row=dict(sender=i,key={'context':str(slot),'request_id':label},text=label);self.messages.append(row);self.log.append(('send',label));return row
            def check_pending(self,row,end,label,receiver_present=True):assert not self.pair[1].live;self.log.append(('pending',label))
            def deliver(self,row,end,retry=False):
                target=self.pair[1-row['sender']];assert target.live and target.watch
                row.update(event_id=row['text'],receiver_instance=target.tag,receiver_epoch=1)
                target.events.append(dict(event_id=row['event_id'],text=row['text'],context=row['key']['context'],runtime_instance=target.tag,epoch=1))
            def histories(self,*args):self.log.append(('history',args[0]))
        model=Model()
        with patch.dict(N['D']['prior'],{'onboard':lambda *args:['0','1']}):model.functional()
        self.assertEqual(len(model.messages),7)
        sequence=model.log;self.assertLess(sequence.index(('pending','offline')),sequence.index(('start',1,'owner-b-1')))
        self.assertEqual([row[1] for row in sequence if row[0]=='watching'],['A','B','C'])
        self.assertFalse(any(term.live for term in model.pair));save('functional-model.json',dict(synthetic_messages=True,sequence=sequence,messages=7))

    def execute_model(self,case,foreign=False,fail=False):
        original={name:'a'*64 for name in N['NAMES']};target={name:'b'*64 for name in N['NAMES']}
        plan=dict(nodes=[dict(side=s,base='fixture-'+s,session=s*32,pid=i+1,started=100+i,script_sha='a'*64,config_sha='b'*64)
            for i,s in enumerate('AB')],attempts=['fresh-one','fresh-two'],pins=target,originals=original,assets='assets',original_assets='originals',ssh={})
        instances=[];runs=[];updates=[]
        class FakeAgent:
            def __init__(self,prefix,base,*args,ledger):
                self.node=plan['nodes'][len(instances)];self.bins=dict(original);instances.append(self)
            def ready(self):return self.node
            def call(self,op,**args):
                if op=='update':self.bins=dict(args['pins']);updates.append((self.node['side'],args['pins']));return dict(state='updated')
                assert op=='status';return dict(active=None,blocked=None)
        class FakeNormal:
            def __init__(self,p,attempt,run,agents):
                self.attempt=attempt;self.agents=agents
            def run(self):
                runs.append((self.attempt,tuple(id(a) for a in self.agents)))
                if foreign:self.agents[0].bins['host.ps1']='f'*64
                if fail:raise RuntimeError('injected functional failure')
        def inspect(agent,node):return dict(active=None,blocked=None,readiness_error=None,bins=agent.bins)
        scope=N['execute'].__globals__
        with patch.dict(scope,Agent=FakeAgent,Normal=FakeNormal,upload=lambda *args:None,inspect_installed=inspect),patch.dict(N['D'],pinned_ssh=lambda *args:[]):
            if foreign or fail:
                with self.assertRaises(RuntimeError):N['execute'](plan,EVIDENCE,EVIDENCE/case)
            else:N['execute'](plan,EVIDENCE,EVIDENCE/case)
        return instances,runs,updates

    def test_two_attempts_same_agents_and_original_restore(self):
        agents,runs,updates=self.execute_model('two-attempts')
        self.assertEqual([r[0] for r in runs],['fresh-one','fresh-two']);self.assertEqual(runs[0][1],runs[1][1])
        self.assertEqual(len(updates),4)
        self.assertTrue(all(set(a.bins.values())=={'a'*64} for a in agents))

    def test_failed_first_attempt_stops_second_and_foreign_restore_refused(self):
        agents,runs,updates=self.execute_model('failed-first',fail=True)
        self.assertEqual(len(runs),1);self.assertEqual(len(updates),4)
        agents,runs,updates=self.execute_model('foreign-restore',foreign=True)
        self.assertEqual(agents[0].bins['host.ps1'],'f'*64)
        self.assertEqual(len([x for x in updates if x[0]=='A']),1)

if __name__=='__main__':
    suite=unittest.defaultTestLoader.loadTestsFromTestCase(Checks)
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    save('checks.json',dict(tests=result.testsRun,failures=len(result.failures),errors=len(result.errors),ok=result.wasSuccessful(),
        scope=__doc__,host_mutations=0))
    sys.exit(0 if result.wasSuccessful() else 1)
