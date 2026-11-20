"""Actual adapter/validators with fake terminal+host IO; not a product persistence test."""
import copy,hashlib,json,runpy,sys,time,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1];S=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-sender-restart.py'));O=S['O'];M,N,D=S['M'],S['N'],S['D']
E=Path(sys.argv[1]).resolve();PLAN=Path(sys.argv[2]).resolve();E.mkdir(parents=True,exist_ok=False);sys.argv=sys.argv[:1]
def save(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n',encoding='utf-8')

class Terminal:
    mode='chat'
    def __init__(self,field,index,number):
        self.field=field;self.index=index;self.number=number;self.tag='owner-a-1' if number==0 else 'owner-b-1';self.instance='abcd'[index+number*2]*64
        self.contexts=field.contexts;self.epoch=None;self.reported_epoch=[2,3,7,9][index+number*2];self.events=[];self.operations=[];self.live=True;self.status_reads=0;self.recovery_reads=0
    def status(self,end):
        assert self.live,'Attempted to inspect an offline terminal';self.operations.append(dict(op='status'));self.status_reads+=1
        instance=self.instance;epoch=self.reported_epoch
        if self.index==0 and self.number==1 and self.status_reads>1:
            if self.field.fault=='old_status_instance':instance=self.field.old_pair[0].instance
            if self.field.fault=='changed_epoch':epoch-=1
        return (dict(mode='owner',runtime_instance=instance,contexts=self.contexts,live_admitted=all(t.live for t in self.field.pair)),
            dict(epoch=epoch,contexts=[dict(slot=i,context=c,state='watching') for i,c in enumerate(self.contexts)]))
    def command_log(self):return self.operations
    def call(self,op,end=None,**args):
        assert self.live,'IPC to stopped process';self.operations.append(dict(op=op));f=self.field;f.log.append(('ipc',self.index,self.number,op))
        if op=='send':
            assert (self.index,self.number) in [(0,0),(1,1)],'Sender recovery must not submit';key=(args['context'],args['request_id']);assert key not in f.requests
            row=dict(event_id=f'{len(f.requests)+1:064x}',text=args['text'],role=f.roles[self.index],ordinal=1)
            f.stored[self.index][args['context']].append(row);f.requests[key]=dict(state='materialized',event_id=row['event_id'],sender=self.index,row=row)
            if f.pair[1-self.index].live:f.transfer(key)
            return {k:v for k,v in f.requests[key].items() if k in ['state','event_id']}
        if op=='request_status':
            value=copy.deepcopy(f.requests[(args['context'],args['request_id'])]);value={k:v for k,v in value.items() if k in ['state','event_id']}
            if self.index==0 and self.number==1 and not f.pair[1].live:
                self.recovery_reads+=1
                if f.fault=='second_unknown' and self.recovery_reads>=3:value=dict(state='unknown',event_id=None)
                elif f.fault=='unknown':value=dict(state='unknown',event_id=None)
                elif f.fault=='queued':value['state']='queued'
                elif f.fault=='request_io_failure':raise OSError('request sentinel')
                elif f.fault=='changed_event':value['event_id']='f'*64
                elif f.fault=='premature_delivered':value['state']='delivered'
                elif f.fault=='changed_request':f.offline_rows[0]['key']['request_id']='0'*32
                elif f.fault=='changed_frozen_context':f.expected_pending[0]['key']['context']='f'*64
            return value
        if op=='history':
            rows=copy.deepcopy(f.stored[self.index][args['context']]);value=dict(context=args['context'],finished=True,items=rows)
            if self.index==0 and self.number==1 and not f.pair[1].live:
                if f.fault=='missing_history':value['items']=[]
                elif f.fault=='extra_history':rows.append(copy.deepcopy(rows[0]))
                elif f.fault=='changed_text':rows[0]['text']='wrong'
                elif f.fault=='changed_ordinal':rows[0]['ordinal']+=1
                elif f.fault=='changed_role':rows[0]['role']=1-rows[0]['role']
                elif f.fault=='wrong_context_reply':value['context']='f'*64
                elif f.fault=='unfinished':value['finished']=False
                elif f.fault=='history_io_failure':raise OSError('history sentinel')
            return value
        raise AssertionError('Unexpected IPC '+op)

class Model(S['SenderRestart']):
    def __init__(self,label,fault=None):
        self.evidence=E/label;self.evidence.mkdir();self.attempt='hint-sender-01';self.end=time.monotonic()+1000;self.contexts=['a'*64,'b'*64];self.roles=[1,0]
        self.messages=[];self.offline_rows=[];self.expected_pending=[];self.recovery_proofs=[];self.processes=[];self.capture_errors=[];self.log=[];self.requests={};self.stored=[{c:[] for c in self.contexts} for _ in (0,1)]
        self.hosts=[SimpleNamespace(name=s) for s in ['A','B']];self.pair=[None,None];self.old_pair=None;self.sender_restarts=0;self.recipient_restarts=0;self.delivery_history_passed=False;self.notification_coverage=dict(evaluated=False);self.command_coverage=None
        self.rollback_started=False;self.fault=fault;self.anchor_present=set()
        self.pair=[self.make_term(i,0) for i in (0,1)]
    def make_term(self,index,number):
        term=Terminal(self,index,number);self.processes.append((index,term));return term
    def start(self,index,tag,mode,end=None):
        if index==1:assert len(self.recovery_proofs)==2 and all(p['passed'] for p in self.recovery_proofs),'B started before both proofs'
        self.log.append(('start',index));return self.make_term(index,1)
    def connect(self,index,warm):
        self.log.append(('connect',index));self.pair[index].epoch=None
        if index==1:
            for key in self.requests:self.transfer(key)
    def transfer(self,key):
        req=self.requests[key];receiver=self.pair[1-req['sender']];rows=self.stored[1-req['sender']][key[0]]
        if not any(r['event_id']==req['event_id'] for r in rows):
            rows.append(copy.deepcopy(req['row']));receiver.events.append(dict(event_id=req['event_id'],context=key[0],text=req['row']['text'],runtime_instance=receiver.instance,epoch=receiver.reported_epoch))
        req['state']='delivered'
    def sample(self,index,term,end):
        instance=term.instance
        if self.fault=='old_snapshot_instance' and index==0 and term.number==1:instance=self.old_pair[0].instance
        return dict(runtime_instance=instance,slots=[dict(context=c,own_locator_revision=1+term.number) for c in self.contexts]),dict(port=50001+index+term.number*100)
    def stop(self,index,term):
        term.live=False;self.log.append(('stop',index,term.number));term.operations.append(dict(op='quit'))
        save(self.evidence/f'{self.hosts[index].name}-{term.tag}-job.json',dict(child_exit=0,mailbox_quit=True,mailbox_eof=True,membership=True,forced=False,before=[],after=[],descriptor=False,work_expired=False,error=None))
    def port_gone(self,index,*args):self.log.append(('port_gone',index))
    def probe(self,*a):raise AssertionError('network gate is an I/O boundary')
    def gate(self,*args,**kwargs):
        assert self.pair[1].number==1 and self.pair[1].live
        return dict(admitted=True,last_complete_runtime={side:dict(doc=self.sample(i,self.pair[i],self.end)[0]) for i,side in enumerate('AB')})

class Checks(unittest.TestCase):
    def functional(self,field):
        with patch.object(M['Diagnostic'],'functional',lambda self:self.log.append(('initial_admitted',))),patch.dict(M,admission_gate=field.gate),patch.dict(D['prior'],observe_incoming=lambda term,row,end:D['prior']['incoming_result'](term.events,row)):
            field.functional()

    def test_actual_functional_preserves_order_and_uses_real_local_history_and_delivery(self):
        field=Model('valid');self.functional(field)
        self.assertIs(Model.histories,D['Field'].histories);self.assertIs(Model.deliver,O['OfflineRecipient'].deliver)
        self.assertIs(Model.pending_persisted,O['OfflineRecipient'].pending_persisted)
        self.assertEqual([x for x in field.log if x[0]=='start'],[('start',0),('start',1)])
        self.assertEqual(len(field.messages),4);self.assertTrue(field.delivery_history_passed)
        for proof in field.recovery_proofs:self.assertTrue(proof['passed']);self.assertEqual(proof['runtime_instance'],field.pair[0].instance)
        for row in field.messages[2:]:self.assertEqual((row['receiver_instance'],row['receiver_epoch']),(field.pair[0].instance,7))
        self.assertTrue(all(r['receiver_instance']==field.pair[1].instance for r in field.messages[:2]))
        self.assertEqual([len(c['rows']) for h in json.loads((field.evidence/'sender-final-history.json').read_bytes()) for c in h['contexts']],[2,2,2,2])
        counts=field.command_verdict();self.assertLess(max(c['commands'] for c in counts),192)
        self.assertTrue(all(c['operations'].get('send',0)==0 for c in counts if c['instance']==field.pair[0].instance))
        verdict=O['notification_verdict'](field.messages,field.processes,field.anchor_present);self.assertEqual((verdict['required'],verdict['observed'],verdict['not_covered']),(2,4,0))
        save(E/'sequence-proof.json',dict(synthetic_io=True,product_persistence_proved=False,sequence=field.log,commands=counts))

    def test_recovery_failures_never_start_recipient_or_resubmit(self):
        scenarios=['unknown','second_unknown','queued','request_io_failure','changed_event','premature_delivered','changed_request','changed_frozen_context','missing_history','extra_history','changed_text','changed_ordinal','changed_role','wrong_context_reply','unfinished','history_io_failure','old_status_instance','changed_epoch','old_snapshot_instance']
        for fault in scenarios:
            field=Model('failure-'+fault,fault)
            with self.subTest(fault=fault),self.assertRaises((RuntimeError,OSError,KeyError)):self.functional(field)
            self.assertNotIn(('start',1),field.log);self.assertEqual(field.recipient_restarts,0)
            self.assertFalse(any(r['op'] in ['send','retry'] for r in field.pair[0].operations))
            if fault=='second_unknown':
                self.assertTrue(json.loads((field.evidence/'sender-recovered-1.json').read_bytes())['passed']);self.assertFalse(json.loads((field.evidence/'sender-recovered-2.json').read_bytes())['passed'])

    def test_final_drains_bind_replies_to_new_A_and_reject_all_old_or_duplicate_events(self):
        for fault in ['none','old_A','old_B','epoch','payload_instance','context','text','duplicate','unexpected']:
            field=Model('drain-'+fault)
            def lifecycle(self):
                self.functional()
                for i,t in self.processes:
                    if t.live:self.stop(i,t)
                reply=copy.deepcopy(self.pair[0].events[0])
                if fault=='old_A':self.old_pair[0].events.append(reply)
                elif fault=='old_B':self.old_pair[1].events.append(copy.deepcopy(self.pair[1].events[0]))
                elif fault=='epoch':self.pair[0].events[0]['epoch']=2
                elif fault=='payload_instance':self.pair[0].events[0]['runtime_instance']=self.old_pair[0].instance
                elif fault=='context':self.pair[0].events[0]['context']='f'*64
                elif fault=='text':self.pair[0].events[0]['text']='wrong'
                elif fault=='duplicate':self.pair[0].events.append(reply)
                elif fault=='unexpected':self.pair[0].events.append(dict(event_id='f'*64))
            with patch.object(M['Diagnostic'],'functional',lambda self:None),patch.object(M['Diagnostic'],'run',lifecycle),patch.dict(M,admission_gate=field.gate),patch.dict(D['prior'],observe_incoming=lambda term,row,end:D['prior']['incoming_result'](term.events,row)):
                if fault=='none':field.run()
                else:
                    with self.assertRaises(RuntimeError):field.run()
            result=json.loads((field.evidence/'sender-restart-result.json').read_bytes());self.assertEqual(result['passed'],fault=='none');self.assertTrue(result['delivery_history']['passed']);self.assertTrue(result['notification_coverage']['evaluated'])
            self.assertEqual(len(json.loads((field.evidence/'notifications-observed.json').read_bytes())['processes']),4)
            self.assertFalse((field.evidence/'offline-recipient-result.json').exists())

    def test_final_command_guard_rejects_retry_resend_or_overflow(self):
        for fault in ['retry','send','quota']:
            field=Model('commands-'+fault);self.functional(field)
            field.pair[0].operations.extend([dict(op='status')]*193 if fault=='quota' else [dict(op=fault)])
            with self.assertRaises(RuntimeError):field.command_verdict()

    def test_generic_incarnation_reader_keeps_A_and_B_scopes_separate(self):
        field=object.__new__(S['SenderRestart']);field.evidence=E/'collector';field.end=time.monotonic()+60;field.capture_errors=[];field.generations=['g','h'];field.incarnations={};calls=[]
        def make_call(index):
            def call(op,**kw):calls.append((index,kw));return kw
            return call
        field.hosts=[SimpleNamespace(name=side,call=make_call(i)) for i,side in enumerate('AB')]
        for index in (0,1):
            for number in (0,1):field.incarnations[index,str(number)]=dict(chat_pid=10+index*100+number*2,snapshot=dict(doc=dict(runtime_pid=11+index*100+number*2,runtime_instance='abcd'[index+number*2]*64)),collected=False)
        def decode(reply,nonce,chat,runtime,instance):
            self.assertEqual((chat,runtime,instance),(reply['diagnostic_chat_pid'],reply['diagnostic_runtime_pid'],reply['diagnostic_runtime_instance']));return {},dict(complete=True)
        with patch.dict(M,shutdown_records=decode):
            for index in (0,1):
                for number in (0,1):field.collect_shutdown(index,SimpleNamespace(mode='chat',tag=str(number),instance='abcd'[index+number*2]*64))
        self.assertFalse(field.capture_errors)
        for index,request in calls:
            self.assertEqual(len(request['diagnostic_previous']),int(request['diagnostic_chat_pid']%100==12))
            if request['diagnostic_previous']:self.assertEqual(request['diagnostic_previous'][0]['diagnostic_chat_pid'],10+index*100)

    def test_expired_shared_budget_cannot_start_a_new_incarnation(self):
        field=Model('budget-expired');field.end=time.monotonic()-1
        with self.assertRaises((RuntimeError,TimeoutError)):self.functional(field)
        self.assertFalse(any(x[0]=='start' for x in field.log))

    def test_plan_rejects_changed_scope_bounds_sources_and_images(self):
        plan=json.loads(PLAN.read_bytes())
        plan['sources']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in S['SOURCES']}
        S['validate'](plan,ROOT)
        mutations=[('purpose','offline-recipient-v1'),('attempts',['hint-offline-03']),
            ('work_seconds',1001),('connect_seconds',86),('admission_seconds',91),('reconnect_seconds',91),
            ('probe_seconds',61),('minimum_call_seconds',1),('additional_network_seconds',91),
            ('additional_network_mode','unbounded'),('original_window_origin','after_connect')]
        mutations += [('execution.'+k,v) for k,v in [('evidence','.tmp/other'),('controller_seconds',1801),
            ('outer_seconds',1861),('priority','Normal'),('max_parallel_hosts',3),('runtime_review_required',False)]]
        mutations += [('sources.scripts/test-terminal-t1-3e-sender-restart.py','0'*64),('build_manifest_sha256','0'*64),('pins.kilogram-cli.exe','0'*64)]
        rejected=[]
        for key,value in mutations:
            altered=copy.deepcopy(plan);target=altered;parts=key.split('.',1)
            for part in parts[:-1]:target=target[part]
            target[parts[-1]]=value
            with self.subTest(key=key),self.assertRaises((RuntimeError,AssertionError,ValueError,KeyError)):S['validate'](altered,ROOT)
            rejected.append(key)
        missing=copy.deepcopy(plan);del missing['sources']['scripts/test-terminal-t1-3e-sender-restart-check.py']
        with self.assertRaises((RuntimeError,AssertionError,ValueError,KeyError)):S['validate'](missing,ROOT)
        rejected.append('missing_check_source')
        save(E/'plan-validation.json',dict(validated=True,rejected=rejected,host_mutations=0,images_unmodified=True))

if __name__=='__main__':unittest.main(verbosity=2)
