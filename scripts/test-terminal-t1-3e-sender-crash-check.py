"""Real crash adapter/validators, fake terminal+host IO. No product persistence claim."""
import base64,copy,hashlib,json,runpy,sys,time,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1];E=Path(sys.argv[1]).resolve();E.mkdir(parents=True,exist_ok=False)
S=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-sender-crash.py'));R,M,D=S['R'],S['M'],S['D']
sys.argv=[sys.argv[0],str(E/'prior-fixture'),str(E/'unused-plan.json')]
F=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-sender-restart-check.py'));sys.argv=sys.argv[:1]

def save(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n',encoding='utf-8')

def diagnostics(instance,nonce='7'*32):
    common=dict(schema=1,run_nonce=nonce,pid=10,runtime_pid=11,runtime_instance=instance)
    values=[]
    for kind in ['setup_open','begin','import_begin','import_end','profile_begin','prepared','spawned','descriptor','status']:
        v=common|dict(kind='startup_'+kind,data={})
        if kind!='status':v.update(runtime_pid=None,runtime_instance=None)
        values.append(v)
    values.extend([common|dict(kind='connection_final',data=dict(was_running=False,child_query_failed=False,ack_attempted=False,ack=False,
        forced=False,exit_success=False,exit=137,eof=True,drain_count=2,drain_eof=[True,True],drain_failed=[False,False])),
        common|dict(kind='worker_final',data=dict(running_ok=True,saved_ok=True,stop_graceful=False,stop_forced=False,reaped=True,exit=137,eof=True))])
    return dict(schema=1,run_nonce=nonce,rejected=[],records=[dict(name=v['kind'].replace('_','-')+'-10.json',
        bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),data=base64.b64encode(raw).decode()) for v in values for raw in [json.dumps(v).encode()]])

class Terminal(F['Terminal']):
    def __init__(self,*args):
        super().__init__(*args);self.requests={};self.queued=[];self.closed=None
        self.mailbox=SimpleNamespace(generation='8'*32)
        self.local_owner=SimpleNamespace(sample=lambda end:(dict(blocked=None,result=None),dict(state='expected_crash_reconciled',exit=1)))
    def result(self,op,args,end):
        assert op=='quit' and self.field.crash is not None
        self.live=False;self.closed='failed';self.operations.append(dict(op=op));self.field.log.append(('postcrash-quit',0))
        return dict(ok=False,code='cleanup_unconfirmed')
    def collect(self,end):return dict(records=[])

class FakeHost:
    def __init__(self,f,index):self.f=f;self.index=index;self.name='AB'[index];self.lab_identity=dict(worker_pid=42,worker_started=123)
    def call(self,op,**args):
        f=self.f
        if op=='lab-crash-descriptor':return dict(sha256=('c' if f.pair[0].number==1 and f.fault!='descriptor_not_replaced' else 'd')*64,
            profile_name='profile-'+('a'*64)+'.json',profile_sha256=('0' if f.pair[0].number==1 and f.fault=='profile_changed' else 'b')*64)
        if op=='put':
            raw=base64.b64decode(args['data']);assert hashlib.sha256(raw).hexdigest()==args['sha256'];f.transferred=json.loads(raw)
            return dict(sha256=args['sha256'])
        if op=='lab-runtime-crash':
            assert not f.pair[1].live and f.transferred['sender_instance']==f.pair[0].instance
            if f.injected is None:
                f.injected=copy.deepcopy(args['fault']);f.fault_actions+=1;f.log.append(('runtime-crash',0))
                if f.fault=='lost_reply':raise TimeoutError('synthetic lost reply AFTER injection')
            assert args['fault']==f.injected
            return dict(request=f.injected['request'],runtime_pid=11,runtime_started=222,runtime_instance=f.pair[0].instance,exit=124 if f.fault=='wrong_exit' else 137)
        if op=='receipt':return dict(child_exit=3,mailbox_eof=True,mailbox_quit=False,membership=True,forced=False,before=[],after=[],
            descriptor=True,work_expired=False,error='Mailbox launch, stop or exchange failed')
        if op=='lab-diagnostic-read':
            reply=diagnostics(f.old_pair[0].instance)
            if f.fault=='missing_parent':reply['records']=[r for r in reply['records'] if not r['name'].startswith('worker-final')]
            return reply
        raise AssertionError(op)

class Model(S['SenderCrash']):
    def __init__(self,label,fault=None):
        F['Model'].__init__(self,label,fault);self.crash=None;self.crash_classified=False;self.fault_actions=0;self.injected=None
        self.hosts=[FakeHost(self,i) for i in (0,1)];self.agents=[SimpleNamespace(session='9'*32),None];self.generations=['7'*32,'6'*32]
        self.agents[0].ready=lambda:dict(crash_policy_version='owned-runtime-crash-v1',crash_policy_sha256='a'*64,crash_descendants_version='passive-natural-drain-v1')
        self.plan=dict(pins={'kilogram-cli.exe':'a'*64},nodes=[dict(crash_policy_sha256='a'*64)]);self.receipts=set()
        self.incarnations={(0,'owner-a-1'):dict(chat_pid=10,snapshot=dict(doc=dict(runtime_pid=11)),collected=False)}
        save(self.evidence/'A-owner-a-1-socket.json',dict(pid=11,started=222,image='synthetic.exe'))
    def make_term(self,index,number):
        term=Terminal(self,index,number);self.processes.append((index,term));return term
    def start(self,index,tag,mode,end=None):
        if index==0:assert self.crash_classified,'A restarted before fault cleanup'
        return F['Model'].start(self,index,tag,mode,end)
    transfer=F['Model'].transfer;sample=F['Model'].sample
    stop=F['Model'].stop;port_gone=F['Model'].port_gone;probe=F['Model'].probe;gate=F['Model'].gate

def functional(field):
    with patch.object(M['Diagnostic'],'functional',lambda self:None),patch.object(R['SenderRestart'],'connect',F['Model'].connect),patch.dict(M,admission_gate=field.gate),patch.dict(D['prior'],observe_incoming=lambda term,row,end:D['prior']['incoming_result'](term.events,row)):
        field.functional()

class Checks(unittest.TestCase):
    def test_old_or_mixed_loaded_agent_policy_cannot_begin_scenario(self):
        for fault in ['version','digest','descendants']:
            field=Model('policy-'+fault)
            field.agents[0].ready=lambda:dict(crash_policy_version='old' if fault=='version' else 'owned-runtime-crash-v1',
                crash_policy_sha256='0'*64 if fault=='digest' else 'a'*64,
                crash_descendants_version='old' if fault=='descendants' else 'passive-natural-drain-v1')
            with patch.object(M['Diagnostic'],'run',side_effect=AssertionError('scenario must not begin')):
                with self.assertRaises(RuntimeError):field.run()
            self.assertEqual(field.fault_actions,0)

    def test_expired_shared_budget_cannot_inject_or_restart(self):
        field=Model('expired');field.end=time.monotonic()-1
        with self.assertRaises((RuntimeError,TimeoutError)):functional(field)
        self.assertEqual(field.fault_actions,0);self.assertFalse(any(x[0]=='start' for x in field.log))

    def test_real_sequence_and_lost_reply_never_reinject_or_resubmit(self):
        for fault in [None,'lost_reply']:
            field=Model('sequence-'+str(fault),fault);functional(field)
            self.assertEqual(field.fault_actions,1);self.assertTrue(field.crash_classified)
            self.assertIs(Model.functional,R['SenderRestart'].functional)
            self.assertIs(Model.histories,D['Field'].histories)
            self.assertEqual([x for x in field.log if x[0]=='start'],[('start',0),('start',1)])
            self.assertLess(field.log.index(('runtime-crash',0)),field.log.index(('postcrash-quit',0)))
            self.assertEqual([p['passed'] for p in field.recovery_proofs],[True,True])
            self.assertEqual(len(field.messages),4)
            self.assertTrue(all(r['receiver_instance']==field.pair[0].instance and r['receiver_epoch']==7 for r in field.messages[2:]))
            counts=field.command_verdict();self.assertEqual([r['operations'].get('send',0) for r in counts],[2,0,0,2])
            save(field.evidence/'sequence-proof.json',dict(synthetic_io=True,product_persistence_proved=False,commands=counts,sequence=field.log))

    def test_recovery_and_fault_failures_do_not_restart_B_or_send_old_requests(self):
        faults=['unknown','second_unknown','queued','request_io_failure','changed_event','premature_delivered','changed_request','changed_frozen_context',
            'missing_history','extra_history','changed_text','changed_ordinal','changed_role','wrong_context_reply','unfinished','history_io_failure',
            'old_status_instance','changed_epoch','old_snapshot_instance','wrong_exit','missing_parent','descriptor_not_replaced','profile_changed']
        for fault in faults:
            field=Model('failure-'+fault,fault)
            with self.subTest(fault=fault),self.assertRaises((RuntimeError,OSError,KeyError)):functional(field)
            self.assertNotIn(('start',1),field.log);self.assertEqual(field.recipient_restarts,0)
            self.assertTrue(all(r['op']!='retry' for _,t in field.processes for r in t.operations))
            if fault in ['wrong_exit','missing_parent']:self.assertNotIn(('start',0),field.log)
            if fault=='second_unknown':self.assertTrue(json.loads((field.evidence/'sender-recovered-1.json').read_bytes())['passed'])

    def test_crashed_diagnostic_exception_is_narrow_and_keeps_raw_incomplete(self):
        reply=diagnostics('a'*64);records,coverage=S['crashed_diagnostics'](reply,'7'*32,10,11,'a'*64)
        self.assertEqual(len(records),11);self.assertFalse(coverage['complete']);self.assertEqual(set(coverage['missing']),S['RUNTIME_FINALS'])
        for fault in ['nonce','pid','instance','extra_missing','startup_missing','rejected','bad_bytes','drain','wrong_exit']:
            altered=copy.deepcopy(reply)
            if fault=='nonce':altered['run_nonce']='0'*32
            elif fault in ['extra_missing','startup_missing']:altered['records'].pop(-1 if fault=='extra_missing' else 0)
            elif fault=='rejected':altered['rejected']=[dict(name='unknown',reason='bad')]
            else:
                record=next(r for r in altered['records'] if r['name'].startswith('connection-final'));v=json.loads(base64.b64decode(record['data']))
                if fault=='pid':v['pid']+=1
                elif fault=='instance':v['runtime_instance']='0'*64
                elif fault=='drain':v['data']['drain_eof'][0]=False
                elif fault=='wrong_exit':v['data']['exit']=124
                raw=json.dumps(v).encode();record.update(data=base64.b64encode(raw).decode(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
                if fault=='bad_bytes':record['sha256']='0'*64
            with self.subTest(fault=fault),self.assertRaises((RuntimeError,KeyError)):S['crashed_diagnostics'](altered,'7'*32,10,11,'a'*64)

    def test_final_notification_binding_and_raw_crash_result(self):
        for fault in ['none','old_A','old_B','epoch','instance','context','text','duplicate','unexpected']:
            field=Model('drain-'+fault)
            def lifecycle(self):
                self.functional()
                for i,t in self.processes:
                    if t.live:self.stop(i,t)
                reply=copy.deepcopy(self.pair[0].events[0])
                if fault=='old_A':self.old_pair[0].events.append(reply)
                elif fault=='old_B':self.old_pair[1].events.append(copy.deepcopy(self.pair[1].events[0]))
                elif fault=='epoch':self.pair[0].events[0]['epoch']=2
                elif fault=='instance':self.pair[0].events[0]['runtime_instance']=self.old_pair[0].instance
                elif fault=='context':self.pair[0].events[0]['context']='f'*64
                elif fault=='text':self.pair[0].events[0]['text']='wrong'
                elif fault=='duplicate':self.pair[0].events.append(reply)
                elif fault=='unexpected':self.pair[0].events.append(dict(event_id='f'*64))
                save(self.evidence/'result.json',dict(agents=dict(A=dict(owners_clean=2,owners_expected_crash=1),B=dict(owners_clean=3))))
            with patch.object(M['Diagnostic'],'functional',lambda self:None),patch.object(R['SenderRestart'],'connect',F['Model'].connect),patch.object(M['Diagnostic'],'run',lifecycle),patch.dict(M,admission_gate=field.gate),patch.dict(D['prior'],observe_incoming=lambda term,row,end:D['prior']['incoming_result'](term.events,row)):
                if fault=='none':field.run()
                else:
                    with self.assertRaises(RuntimeError):field.run()
            result=json.loads((field.evidence/'sender-crash-result.json').read_bytes());self.assertEqual(result['passed'],fault=='none')
            self.assertFalse((field.evidence/'sender-restart-result.json').exists())

if __name__=='__main__':unittest.main(verbosity=2)
