"""Actual inherited start/finally/receipt/collect chain with synthetic IO; no actors."""
import ast,copy,json,runpy,subprocess,sys,time
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1];OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=False)
S=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-sender-crash.py'));O,N,M,D=S['O'],S['N'],S['M'],S['D']
def save(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n',encoding='utf-8')
def old_class(rel,name,scope,base=None):
    raw=subprocess.check_output(['git','show','2441dc11d24883ab4a441065144729cc5197d763:'+rel],timeout=15)
    (OUT/(name+'.original.py')).write_bytes(raw);node=next(x for x in ast.parse(raw).body if isinstance(x,ast.ClassDef) and x.name==name)
    if base:node.bases=[ast.Name(id=base,ctx=ast.Load())]
    exec(compile(ast.fix_missing_locations(ast.Module(body=[node],type_ignores=[])),'<published-original-'+name+'>','exec'),scope);return scope[name]
oldO=old_class('scripts/test-terminal-t1-3e-offline-recipient.py','OfflineRecipient',dict(O))
scope=dict(S,OriginalOffline=oldO);oldS=old_class('scripts/test-terminal-t1-3e-sender-crash.py','SenderCrash',scope,'OriginalOffline')

def exercise(base,label,stage,index=0,tag='owner-b-1',bad_receipt=False,crashed_override=False):
    folder=OUT/label;folder.mkdir();calls=[];primary='injected-'+stage
    nodes=[dict(host=side,sid='sid-'+side,session=side.lower()*32,pid=i+10,started=134396000000000000+i,crash_policy_sha256='e'*64,baseline_sha256='f'*64) for i,side in enumerate('AB')]
    class Agent:
        def __init__(self,i):self.i=i;self.session=nodes[i]['session'];self.active='one'
        def ready(self):return nodes[self.i]|dict(crash_policy_version='owned-runtime-crash-v1',crash_policy_sha256='e'*64)
        def call(self,op,**kw):
            calls.append(('agent',self.i,op))
            if op=='begin':return dict(worker_pid=100+self.i,worker_start=134396000000000000+self.i)
            if op=='status':return dict(started=True,active=self.active,blocked=None)
            if op in ('cancel','end'):self.active=None;return {}
            raise AssertionError(op)
        def wait(self,*args):return dict(state='failed',cleanup=dict(empty_job=True,error=None,forced=False))
    class Host:
        def __init__(self,i):self.i=i;self.name='AB'[i];self.lab_identity={}
        def clock(self):return dict(attempt='one',generation=field.generations[self.i],session=field.agents[self.i].session,
            worker_pid=100+self.i,worker_started=134396000000000000+self.i,start_tick=1000,tick=2000,work_tick=1001000,whole_tick=1016000)
        def receipt(self,tag,**kwargs):
            calls.append(('receipt',self.i,tag))
            if bad_receipt:raise RuntimeError('injected-owner-clean-failure')
            save(folder/f'{self.name}-{tag}-job.json',dict(mailbox_eof=True,mailbox_quit=True))
        def call(self,op,**kwargs):
            calls.append(('diagnostic',self.i,op,kwargs))
            assert stage=='after-start' and kwargs['diagnostic_chat_pid']==12345,'unknown/stale PID diagnostic query'
            raise OSError('injected-diagnostic-IO')
    class Term:
        def __init__(self,host,tag,mode,end,**kwargs):self.tag=tag;self.mode=mode;self.deadline=end;self.events=[];self.instance=None;self.epoch=None
        def line(self,*args):
            if stage=='ready':raise OSError(primary)
            return dict(v=1,type='ready')
        def call(self,op,*args,**kwargs):
            assert op=='status'
            if stage=='status':raise OSError(primary)
            return dict(connected=False)
        def close(self):calls.append(('close',index,tag))
        def command_log(self):return []
    class Model(base):
        def __init__(self):
            self.evidence=folder;self.started=time.monotonic();self.end=self.phase_end=self.started+1000;self.attempt='one'
            self.plan=dict(nodes=nodes,work_seconds=1000,pins={'kilogram-cli.exe':'a'*64});self.agents=[Agent(i) for i in (0,1)];self.hosts=[Host(i) for i in (0,1)];self.generations=[None,None]
            self.processes=[];self.receipts=set();self.messages=[];self.transfers=[];self.latest=[None,None];self.chat_pids=[4444,5555];self.capture_errors=[];self.collected=set()
            self.incarnations={};self.crash=None;self.crash_classified=True;self.contexts=['1'*64,'2'*64];self.recovery_proofs=[];self.delivery_history_passed=False
            self.sender_restarts=self.recipient_restarts=0;self.notification_coverage=dict(evaluated=False);self.command_coverage=None
        def preflight(self,i):return dict(user=nodes[i]['sid'])
        def observe(self,*args,**kwargs):
            if stage=='preconnect':raise RuntimeError(primary)
            return dict(disconnected=True,descriptor=index==0 and tag=='owner-b-1',crash_descriptor_sha256='d'*64,sockets=[],runtime_pids=[],chat_pid=12345)
        def functional(self):
            if index==0 and tag=='owner-b-1':self.crash=dict(intent=dict(descriptor_sha256='d'*64))
            try:self.start(index,tag,'chat')
            except BaseException:
                if crashed_override:self.crash=dict(intent={})
                raise
            if stage=='unregistered':del self.incarnations[index,tag]
            raise RuntimeError(primary)
    field=Model();error=None
    with patch.dict(D,MailboxTerminal=Term):
        try:field.run()
        except Exception as e:error=dict(type=type(e).__name__,message=str(e))
    app=json.loads((folder/'result.json').read_bytes());assert app['failure']['message']==primary,(label,app)
    assert error and not json.loads((folder/'sender-crash-result.json').read_bytes())['passed']
    return field,app,error,calls

rows=[]
for stage in ('ready','status','preconnect'):
    for index,tag in [(0,'owner-a-1'),(0,'owner-b-1'),(1,'owner-a-1')]:
        label=stage+'-'+str(index)+'-'+tag
        old,app,error,calls=exercise(oldS,'original-'+label,stage,index,tag)
        assert error['type']=='KeyError' and not (old.evidence/'diagnostic-result.json').exists(),error
        field,app,error,calls=exercise(S['SenderCrash'],'corrected-'+label,stage,index,tag,bad_receipt=True)
        assert error['type']!='KeyError' and app['cleanup'][0]['error']=='injected-owner-clean-failure'
        diagnostic=json.loads((field.evidence/'diagnostic-result.json').read_bytes());assert not diagnostic['capture_complete'] and len(diagnostic['capture_errors'])==1
        coverage=json.loads((field.evidence/f'shutdown/{"AB"[index]}/{tag}/coverage.json').read_bytes());assert not coverage['complete'] and coverage['reason']=='PartialStartUnboundChat'
        assert field.incarnations[index,tag]['chat_pid'] is None and not any(c[0]=='diagnostic' for c in calls)
        rows.append(dict(case=label,original_masked=True,primary_preserved=True,separate_cleanup=True,incomplete_unknown_identity=True,no_replacement_PID=True))
field,app,error,calls=exercise(S['SenderCrash'],'known-identity-diagnostic-IO','after-start',0,'owner-b-1')
diag=json.loads((field.evidence/'diagnostic-result.json').read_bytes());assert app['failure']['message']=='injected-after-start' and not diag['capture_complete'] and diag['capture_errors'][0]['error']['type']=='OSError'
rows.append(dict(case='known-identity-diagnostic-IO',primary_preserved=True,diagnostic_error_separate=True))
field,app,error,calls=exercise(S['SenderCrash'],'crashed-override-unbound','ready',0,'owner-a-1',crashed_override=True)
assert field.capture_errors[0]['error']['type']=='PartialStartUnboundChat' and field.incarnations[0,'owner-a-1']['chat_pid'] is None
rows.append(dict(case='crashed-override-unbound',incomplete=True,no_false_expected_crash=True))
field,app,error,calls=exercise(S['SenderCrash'],'unregistered-invariant','unregistered',0,'owner-b-1')
assert field.capture_errors[0]['error']['type']=='UnregisteredChatScope' and not any(c[0]=='diagnostic' for c in calls)
rows.append(dict(case='unregistered-invariant',explicit_diagnostic_failure=True,no_blanket_suppression=True))
save(OUT/'checks.json',dict(ok=True,count=len(rows),cases=rows,actual_inherited_run_finally_receipt_collect=True,IO_synthetic=True,installed_actors=False))
print(json.dumps(dict(ok=True,cases=len(rows))),flush=True)
