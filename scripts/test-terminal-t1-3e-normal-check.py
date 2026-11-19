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
    def test_sftp_options_retain_pinned_ssh_identity(self):
        prefix=[r'C:\Windows\System32\OpenSSH\ssh.exe','-F','NUL','-T','-o','StrictHostKeyChecking=yes',
            '-o','UserKnownHostsFile=fixture','-o','BindAddress=192.168.0.140','-i','fixture key','test@192.168.0.152','powershell.exe']
        command=N['sftp_command'](prefix,'sftp.exe',Path('batch file'))
        self.assertEqual(command,['sftp.exe','-q','-b','batch file']+[x for x in prefix[1:-2] if x!='-T']+[prefix[-2]])
        with self.assertRaises(RuntimeError):N['sftp_command'](prefix[:-1]+['other.exe'],'sftp.exe',Path('batch'))

    def test_sftp_upload_dispatch_and_separate_failure_receipts(self):
        folder=EVIDENCE/'sftp-dispatch-inputs';folder.mkdir()
        for name in N['NAMES']:(folder/name).write_bytes(b'fixture')
        pins={name:sha(folder/name) for name in N['NAMES']};sizes={name:7 for name in pins};stage='d'*32
        agents=[None,SimpleNamespace(prefix=[r'C:\Windows\System32\OpenSSH\ssh.exe','-T','-o','StrictHostKeyChecking=yes','test@192.168.0.152','powershell.exe'])]
        node=dict(host='PAUL_HP_OMEN',sid='test-fixture',base=r'C:\Users\test\KilogramLab\terminal-t11f')
        results=[]
        for fail in (False,True):
            evidence=EVIDENCE/('sftp-dispatch-fail' if fail else 'sftp-dispatch-success');calls=[]
            def run(command,**kwargs):
                self.assertEqual(kwargs['stdin'],subprocess.DEVNULL);self.assertNotIn('input',kwargs);calls.append(command)
                count=len(calls)
                if count==2:
                    (evidence/stage/'sftp-client.json').write_text(json.dumps(dict(exit=7 if fail else 0,empty_job=True,failure='client' if fail else None)))
                    return subprocess.CompletedProcess(command,1 if fail else 0,b'',b'client-error' if fail else b'')
                if count==3 and fail:return subprocess.CompletedProcess(command,1,b'',b'verify-error')
                receipt=dict(stage=stage,pins=pins,sizes=sizes,server_process_absent=True,verified=count==3)
                return subprocess.CompletedProcess(command,0,json.dumps(receipt).encode(),b'')
            with patch.object(N['subprocess'],'run',run):
                if fail:
                    with self.assertRaisesRegex(RuntimeError,'SFTP client failed'):N['upload'](agents,1,stage,folder,pins,node,evidence=evidence,transfer=dict(exe='sftp.exe',sha256='a'*64))
                else:N['upload'](agents,1,stage,folder,pins,node,evidence=evidence,transfer=dict(exe='sftp.exe',sha256='a'*64))
            self.assertEqual(len(calls),3)
            receipt=json.loads((evidence/stage/'upload-result.json').read_bytes());self.assertEqual(bool(receipt['verification_error']),fail)
            self.assertEqual(receipt['client_exit'],1 if fail else 0);results.append(dict(injected_failure=fail,receipt=receipt))
        save('sftp-dispatch.json',dict(synthetic_transport=True,cases=results,bulk_stdin=False))

    def test_sftp_native_client_job_receipts(self):
        results=[]
        for case,child in [('success',"[Console]::Out.WriteLine('client-output');[Environment]::Exit(0)"),
            ('failure',"[Console]::Error.WriteLine('client-error');[Environment]::Exit(7)"),
            ('blocked','Start-Sleep -Seconds 30')]:
            logs=EVIDENCE/('sftp-client-'+case);logs.mkdir()
            cmd=[PS,'-NoProfile','-NonInteractive','-EncodedCommand',base64.b64encode(child.encode('utf-16-le')).decode()]
            text=N['sftp_client_script'](cmd,sha(Path(PS)),logs,2 if case=='blocked' else 8,4 if case=='blocked' else 10)
            outcome=ps(text);(logs/'wrapper.stderr.bin').write_bytes(outcome.stderr)
            self.assertEqual(outcome.returncode,0 if case=='success' else 1,outcome.stderr.decode(errors='replace'))
            receipt=json.loads((logs/'sftp-client.json').read_bytes());self.assertTrue(receipt['empty_job']);self.assertGreater(receipt['child_started'],0)
            self.assertEqual(receipt['exit'],dict(success=0,failure=7,blocked=124)[case])
            self.assertEqual(bool(receipt['failure']),case!='success')
            if case=='success':self.assertIn('client-output',(logs/'sftp.stdout.log').read_text())
            if case=='failure':self.assertIn('client-error',(logs/'sftp.stderr.log').read_text())
            results.append(dict(case=case,receipt=receipt))
        save('native-sftp-client.json',dict(cases=results,real_local_job=True,synthetic_sftp_payload=True,live_sftp=False))

    def test_sftp_stage_manifest_and_content_validation(self):
        identity=ps("@{host=[Environment]::MachineName;sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value}|ConvertTo-Json -Compress")
        self.assertEqual(identity.returncode,0);node=json.loads(identity.stdout)
        folder=EVIDENCE/'sftp-stage-fixture';(folder/'bin').mkdir(parents=True);node['base']=str(folder)
        (folder/'bin/deadline.ps1').write_bytes((ROOT/'scripts/migration-s5-l1-deadline.ps1').read_bytes())
        data={name:(bytes(range(256))*16384 if name.endswith('.exe') else b'synthetic-content') for name in N['NAMES']}
        pins={name:hashlib.sha256(value).hexdigest() for name,value in data.items()};sizes={name:len(value) for name,value in data.items()};stage='e'*32
        prepare=N['sftp_stage_script'](node,stage,pins,sizes);outcome=ps(prepare)
        self.assertEqual(outcome.returncode,0,outcome.stderr.decode(errors='replace'));self.assertFalse(json.loads(outcome.stdout)['verified'])
        self.assertNotEqual(ps(prepare).returncode,0)
        remote=folder/'staging'/stage
        for name,value in data.items():(remote/name).write_bytes(value)
        verify=N['sftp_stage_script'](node,stage,pins,sizes,True);outcome=ps(verify)
        self.assertEqual(outcome.returncode,0,outcome.stderr.decode(errors='replace'));self.assertTrue(json.loads(outcome.stdout)['server_process_absent'])
        (remote/'foreign').write_text('preserve');self.assertNotEqual(ps(verify).returncode,0);self.assertTrue((remote/'foreign').exists())
        (remote/'foreign').unlink();(remote/'host.ps1').write_bytes(b'wrong-content');self.assertNotEqual(ps(verify).returncode,0)
        save('native-sftp-stage.json',dict(content_validated=True,fresh_stage_required=True,foreign_refused_preserved=True,wrong_content_refused=True,live_sftp=False))

    def test_upload_exact_bytes_without_eof_and_bounded_failures(self):
        identity=ps("@{host=[Environment]::MachineName;sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value}|ConvertTo-Json -Compress")
        self.assertEqual(identity.returncode,0,identity.stderr)
        node=json.loads(identity.stdout);base=EVIDENCE/'upload-fixture';(base/'bin').mkdir(parents=True)
        (base/'bin/deadline.ps1').write_bytes((ROOT/'scripts/migration-s5-l1-deadline.ps1').read_bytes());node['base']=str(base)
        data=bytes(range(256))*1025;pin=hashlib.sha256(data).hexdigest();results=[]
        for case in ('open-stdin','wrong-pin','short','blocked'):
            stage=hashlib.md5(case.encode()).hexdigest()
            script=N['upload_script'](node,stage,'deadline.ps1','0'*64 if case=='wrong-pin' else pin,len(data))
            if case=='blocked':
                # Same production watchdog and read loop; shorten only the local fixture budget.
                self.assertEqual(script.count('::new(80,90)'),1);script=script.replace('::new(80,90)','::new(2,4)')
            command=[PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',N['encoded_script'](script)]
            self.assertLess(len(command[-1]),7400)
            started=time.monotonic();proc=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            try:
                if case!='blocked':proc.stdin.write(data[:17] if case=='short' else data);proc.stdin.flush()
                if case=='short':proc.stdin.close();proc.stdin=None
                proc.wait(timeout=20) # open-stdin, wrong-pin and blocked retain a live writer here.
                out,err=proc.communicate(timeout=5)
            finally:
                if proc.poll() is None:proc.kill();proc.wait(timeout=5)
                if proc.stdin is not None:proc.stdin.close()
            folder=base/'staging'/stage
            (EVIDENCE/f'upload-{case}.stdout.log').write_bytes(out);(EVIDENCE/f'upload-{case}.stderr.log').write_bytes(err)
            if case=='open-stdin':
                self.assertEqual(proc.returncode,0,err);self.assertFalse(err)
                receipt=json.loads(out);self.assertEqual(receipt['sha256'],pin);self.assertEqual(receipt['bytes'],len(data))
                self.assertEqual((folder/'deadline.ps1').read_bytes(),data)
                self.assertEqual(receipt,json.loads((folder/'deadline.ps1.upload-started.json').read_bytes()))
            else:
                self.assertEqual(proc.returncode,124 if case=='blocked' else 1,(case,err))
                if case!='blocked':self.assertIn(b'Stage pin' if case=='wrong-pin' else b'Short stage input',err)
            results.append(dict(case=case,exit=proc.returncode,elapsed=time.monotonic()-started,stdin_held_open=case!='short'))
        for length in (0,-1,128*1024*1024+1,True):
            with self.assertRaises(RuntimeError):N['upload_script'](node,'a'*32,'deadline.ps1',pin,length)
        save('native-upload.json',dict(cases=results,production_budget=[80,90],blocked_fixture_budget=[2,4],live_ssh=False))

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

    def test_native_preconnect_observer_without_cim(self):
        folder=EVIDENCE/'preconnect-fixture';(folder/'bin').mkdir(parents=True)
        (folder/'runs/one/setup').mkdir(parents=True)
        source=(ROOT/'scripts/test-terminal-t1-3e-observe.ps1').read_text(encoding='utf-8')
        self.assertNotIn('Get-CimInstance',source)
        # Exercise the entire production observer; redirect only A fixture paths/address.
        source=source.replace(r'E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f',str(folder)).replace("$ip = '192.168.0.140'","$ip = '127.0.0.1'")
        observer=folder/'observer.ps1';observer.write_text(source,encoding='utf-8')
        code=r'''
function Get-CimInstance { throw 'CIM must never be invoked' }
$base='@BASE@';$exe=Join-Path $base 'bin/kilogram-cli.exe';$observer=Join-Path $base 'observer.ps1'
Add-Type -TypeDefinition 'public static class Fixture {public static void Main(string[] args){System.Threading.Thread.Sleep(60000);}}' -OutputAssembly $exe -OutputType ConsoleApplication
$chat=$null;$runtime=$null
try {
 $chat=[Diagnostics.Process]::Start($exe,('chat "'+(Join-Path $base 'runs/one/setup')+'"'));$chat.PriorityClass='BelowNormal'
 $request=@{side='A';host=[Environment]::MachineName;attempt='one';op='preconnect';tag='owner-a-1'}|ConvertTo-Json -Compress
 $valid=(& $observer -Request $request)|ConvertFrom-Json
 if(!$valid.disconnected -or $valid.chat_pid -ne $chat.Id){throw 'Preconnect did not bind fixture Chat'}
 $runtime=[Diagnostics.Process]::Start($exe,('runtime-from-profile "'+(Join-Path $base 'runs/one/setup')+'"'));$runtime.PriorityClass='BelowNormal'
 $refused=$false;try{$null=& $observer -Request $request}catch{$refused=$true}
 if(!$refused){throw 'Preconnect accepted a runtime'}
 $runtime.Kill();$runtime.WaitForExit();$runtime.Dispose();$runtime=$null
 $chat.Kill();$chat.WaitForExit();$chat.Dispose();$chat=$null
 $missing=$false;try{$null=& $observer -Request $request}catch{$missing=$true}
 if(!$missing){throw 'Preconnect accepted absent Chat'}
 @{valid=$valid;runtime_refused=$refused;absent_chat_refused=$missing;networking=$false;cim=$false}|ConvertTo-Json -Depth 5 -Compress
} finally {foreach($child in @($runtime,$chat)){if($null -ne $child){if(!$child.HasExited){$child.Kill();$child.WaitForExit()};$child.Dispose()}}}
'''.replace('@BASE@',str(folder))
        result=ps(code);(EVIDENCE/'preconnect.stderr.log').write_bytes(result.stderr)
        self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'))
        save('native-preconnect.json',json.loads(result.stdout))

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
if(!$wrong){throw 'Wrong process incarnation accepted'}
$command=[T13eToken]::CommandLine($PID,$p.StartTime.ToFileTimeUtc());if($command -notmatch '-EncodedCommand'){throw 'Native command query'}
$wrongCommand=$false;try{$null=[T13eToken]::CommandLine($PID,($p.StartTime.ToFileTimeUtc()+1))}catch{$wrongCommand=$true}
if(!$wrongCommand){throw 'Wrong command process incarnation accepted'}
$value.native_command_line=$true;$value|ConvertTo-Json -Compress
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
        plan=dict(transfer={},nodes=[dict(side=s,base='fixture-'+s,session=s*32,pid=i+1,started=100+i,script_sha='a'*64,config_sha='b'*64)
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
        with patch.dict(scope,Agent=FakeAgent,Normal=FakeNormal,upload=lambda *args,**kwargs:None,inspect_installed=inspect),patch.dict(N['D'],pinned_ssh=lambda *args:[]):
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
