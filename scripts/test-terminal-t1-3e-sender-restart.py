"""Graceful sender and recipient restart; persisted requests are never resubmitted."""
import argparse,copy,json,re,runpy,time
from pathlib import Path
SCRIPTS=Path(__file__).resolve().parent
O=runpy.run_path(str(SCRIPTS/'test-terminal-t1-3e-offline-recipient.py'))
M,N,D=O['M'],O['N'],O['D'];require=O['require'];save=O['save']
PURPOSE='sender-restart-v1';ATTEMPT='hint-sender-01'
SOURCES=O['SOURCES']|{'scripts/test-terminal-t1-3e-sender-restart.py','scripts/test-terminal-t1-3e-sender-restart-check.py'}

def verify_pending(expected,proof,contexts,instance,epoch):
    require(len(expected)==2 and [r['slot'] for r in expected]==[0,1],'two canonical pending slots')
    require(len(contexts)==2 and len(set(contexts))==2 and all(D['HEX'].fullmatch(c or '') for c in contexts),'pending context scope')
    require(proof['runtime_instance']==instance and proof['epoch']==epoch and proof['contexts']==contexts,'pending proof incarnation')
    require(len(proof['samples'])==2,'pending proof cardinality')
    for frozen,sample in zip(expected,proof['samples']):
        key=frozen['key'];slot=frozen['slot']
        require(key['context']==contexts[slot] and re.fullmatch('[a-f0-9]{32}',key['request_id']) and D['HEX'].fullmatch(frozen['event_id']),'pending frozen identity')
        require(sample['slot']==slot and sample['key']==key,'pending request key changed')
        require(sample['request']['state']=='materialized' and sample['request'].get('event_id')==frozen['event_id'],'pending request missing, changed or prematurely delivered')
        rows=sample['history'];baseline=frozen['history']
        require(len(baseline)==1 and baseline[0]['event_id']==frozen['event_id'] and baseline[0]['text']==frozen['text'],'pending baseline history')
        require(rows==baseline,'pending persistent history changed')

class SenderRestart(O['OfflineRecipient']):
    def __init__(self,*args):
        super().__init__(*args);self.sender_restarts=0;self.expected_pending=[];self.recovery_proofs=[];self.old_pair=None;self.command_coverage=None

    def checkpoint(self,label):
        M['budget'](self.end)
        save(self.evidence/(label+'-budget.json'),dict(remaining_work_seconds=self.end-time.monotonic(),cleanup_reserve_seconds=60,shared_native_budget=True))

    def clean_stop(self,index,label):
        term=self.pair[index]
        doc,point=D['until'](lambda end:self.sample(index,term,end),60,self.end,'local runtime snapshot unavailable')
        self.stop(index,term);self.port_gone(index,point['port'],label,self.end)
        require(not self.capture_errors,'old incarnation typed diagnostics incomplete')
        receipt=json.loads((self.evidence/f'{self.hosts[index].name}-{term.tag}-job.json').read_bytes())
        require(receipt['child_exit']==0 and receipt['mailbox_quit'] and receipt['mailbox_eof'] and receipt['membership'] and
            not receipt['forced'] and not receipt['before'] and not receipt['after'] and not receipt['descriptor'] and not receipt['work_expired'] and receipt['error'] is None,'old incarnation graceful stop')
        save(self.evidence/(label+'-stopped.json'),dict(instance=term.instance,runtime=doc,endpoint=point,job=receipt))
        return doc

    def unchanged_pending_rows(self):
        require(len(self.messages)==len(self.offline_rows)==2,'old request resubmission or missing row')
        for row,frozen in zip(self.offline_rows,self.expected_pending):
            require(row['sender']==0 and row['slot']==frozen['slot'] and row['key']==frozen['key'] and
                row['text']==frozen['text'] and row['pending_event']==frozen['event_id'],'original pending request changed')

    def recovery_snapshot(self,number):
        limit=min(self.end,time.monotonic()+35);term=self.pair[0]
        proof=dict(observation=number,runtime_instance=term.instance,epoch=term.epoch,contexts=list(self.contexts),samples=[],passed=False)
        try:
            self.unchanged_pending_rows()
            require(self.pair[1] is self.old_pair[1] and self.recipient_restarts==0,'recipient returned before sender recovery proof')
            ready=self.monitor(0,limit,watching=True,warm=True)
            require(ready is not None and term is not self.old_pair[0] and term.instance!=self.old_pair[0].instance,'new sender must be locally watching')
            proof.update(runtime_instance=ready['runtime_instance'],epoch=term.epoch)
            for frozen in self.expected_pending:
                sample=dict(slot=frozen['slot'],key=copy.deepcopy(frozen['key']))
                proof['samples'].append(sample)
                sample['request']=term.call('request_status',limit,**frozen['key'])
                sample['history']=D['history'](term,frozen['key']['context'],limit)
            verify_pending(self.expected_pending,proof,self.contexts,term.instance,term.epoch)
            self.unchanged_pending_rows();proof['passed']=True
            return proof
        finally:save(self.evidence/f'sender-recovered-{number}.json',proof)

    def functional(self):
        M['Diagnostic'].functional(self) # Fresh Setup/initial Auto, not the old complete offline scenario.
        self.monitors('A',self.end,watching=True);self.initial_histories(self.end);self.old_pair=list(self.pair)
        old_receiver_doc=self.clean_stop(1,'recipient-offline')
        pending=[]
        for slot in (0,1):
            row=self.send(0,slot,f'offline-{slot}',self.end);first=self.pending_persisted(row)
            self.offline_rows.append(row);pending.append(dict(slot=slot,key=copy.deepcopy(row['key']),first=first))
        for row,proof in zip(self.offline_rows,pending):proof['second']=self.pending_persisted(row)
        self.expected_pending=[dict(slot=r['slot'],key=copy.deepcopy(r['key']),event_id=r['pending_event'],text=r['text'],history=copy.deepcopy(p['first']['sender_history'])) for r,p in zip(self.offline_rows,pending)]
        for name in ['first','second']:
            proof=dict(runtime_instance=self.pair[0].instance,epoch=self.pair[0].epoch,contexts=self.contexts,
                samples=[dict(slot=p['slot'],key=p['key'],request=p[name]['request'],history=p[name]['sender_history']) for p in pending])
            verify_pending(self.expected_pending,proof,self.contexts,self.pair[0].instance,self.pair[0].epoch)
        require(not self.old_pair[1].events,'offline recipient received a notification')
        save(self.evidence/'offline-pending.json',dict(recipient_stopped=True,sender_instance=self.pair[0].instance,requests=pending,expected=self.expected_pending))
        self.checkpoint('before-sender-restart');old_sender_doc=self.clean_stop(0,'sender-offline')
        self.pair[0]=self.start(0,'owner-b-1','chat');self.sender_restarts+=1;self.connect(0,True)
        require(self.pair[0].instance!=self.old_pair[0].instance,'sender runtime not fresh')
        D['until'](lambda end:self.monitor(0,end,watching=True,warm=True),30,self.end,'new sender local monitor unavailable')
        new_sender_doc,_=D['until'](lambda end:self.sample(0,self.pair[0],end),60,self.end,'reopened local runtime unavailable')
        require(new_sender_doc['runtime_instance']==self.pair[0].instance,'reopened sender snapshot incarnation')
        require(all(n['own_locator_revision']>o['own_locator_revision'] for n,o in zip(new_sender_doc['slots'],old_sender_doc['slots'])),'sender locator revision did not advance')
        self.recovery_proofs=[self.recovery_snapshot(1),self.recovery_snapshot(2)]
        require(len(self.recovery_proofs)==2 and all(p['passed'] for p in self.recovery_proofs),'two post-restart pending proofs required')
        self.checkpoint('before-recipient-return')
        self.pair[1]=self.start(1,'owner-b-1','chat');self.recipient_restarts+=1
        started=time.monotonic();self.connect(1,True)
        require(self.pair[1].instance!=self.old_pair[1].instance,'recipient runtime not fresh')
        self.monitors('B',self.end,watching=True)
        boundary=[dict(context=c,rows=D['history'](self.pair[1],c,self.end)) for c in self.contexts]
        self.anchor_present={r['event_id'] for h in boundary for r in h['rows']}
        save(self.evidence/'recipient-notification-boundary.json',dict(instance=self.pair[1].instance,epoch=self.pair[1].epoch,history=boundary))
        def admitted(rows):return all(r['point'] for r in rows) and D['live_pair']([r['doc'] for r in rows],[r['point'] for r in rows])
        gate=M['admission_gate'](self.probe,admitted,started=started,end=self.end);save(self.evidence/'reconnect-admission.json',gate)
        if not gate['admitted']:raise M['AdmissionFailure']('sender/recipient reconnect original90s gate failed')
        for index,old_doc in [(0,old_sender_doc),(1,old_receiver_doc)]:
            doc=gate['last_complete_runtime']['AB'[index]]['doc']
            require(doc['runtime_instance']==self.pair[index].instance and all(n['own_locator_revision']>o['own_locator_revision'] for n,o in zip(doc['slots'],old_doc['slots'])),'reconnect incarnation or revisions changed')
        for row in self.offline_rows:self.deliver(row,self.end,required_notification=row['pending_event'] not in self.anchor_present)
        self.histories('queued-delivered',self.end)
        for slot in (0,1):self.deliver(self.send(1,slot,f'reply-{slot}',self.end),self.end)
        self.histories('sender-final',self.end)
        require(len(self.messages)==4,'four exact messages required');self.delivery_history_passed=True

    def command_verdict(self):
        chats=[(i,t) for i,t in self.processes if t.mode=='chat']
        require(len(chats)==4 and all(sum(i==host for i,_ in chats)==2 for host in (0,1)),'four Chat scopes required')
        counts=[]
        for index,term in chats:
            rows=term.command_log();ops={op:sum(r['op']==op for r in rows) for op in {r['op'] for r in rows}}
            expected=2 if term is self.old_pair[0] or term is self.pair[1] else 0
            require(len(rows)<=192 and ops.get('retry',0)==0 and ops.get('send',0)==expected,'unexpected resend/retry or command quota')
            counts.append(dict(host='AB'[index],tag=term.tag,instance=term.instance,commands=len(rows),operations=ops))
        return counts

    def run(self):
        passed=False
        try:
            M['Diagnostic'].run(self) # Reuse actual native/application/typed cleanup without misleading old result labels.
            require(self.delivery_history_passed and self.sender_restarts==self.recipient_restarts==1,'sender recovery scenario incomplete')
            self.command_coverage=self.command_verdict()
            self.notification_coverage=dict(evaluated=True,required_passed=False,required=sum(O['notification_required'](r,self.anchor_present) for r in self.messages),observed=sum(len(t.events) for _,t in self.processes),not_covered=None)
            self.notification_coverage=O['notification_verdict'](self.messages,self.processes,self.anchor_present);passed=True
        finally:
            save(self.evidence/'notifications-observed.json',dict(schema=1,processes=[dict(host_index=i,tag=t.tag,instance=getattr(t,'instance',None),epoch=getattr(t,'epoch',None),events=t.events) for i,t in self.processes]))
            save(self.evidence/'sender-restart-result.json',dict(schema=1,purpose=PURPOSE,passed=passed,messages=self.messages,
                delivery_history=dict(passed=self.delivery_history_passed),notification_coverage=self.notification_coverage,command_coverage=self.command_coverage,
                sender_restarts=self.sender_restarts,recipient_restarts=self.recipient_restarts,post_sender_restart_proofs=self.recovery_proofs,
                expected_messages=4,no_resubmission_verified=self.command_coverage is not None,cleanup_in_application_and_diagnostic_receipts=True))

def validate(plan,root):
    require(plan['purpose']==PURPOSE and plan['attempts']==[ATTEMPT],'single reviewed sender-restart attempt')
    execution=plan['execution']
    require(execution['evidence']=='.tmp/terminal-t1-3e/sender-restart-live-01' and execution['controller_seconds']==1800 and execution['outer_seconds']==1860 and
        execution['priority']=='BelowNormal' and execution['max_parallel_hosts']==2 and execution['runtime_review_required'] is True,'sender execution scope and bounds')
    require(plan['work_seconds']==1000 and plan['connect_seconds']==85 and plan['admission_seconds']==plan['reconnect_seconds']==90 and plan['probe_seconds']==60 and plan['minimum_call_seconds']==2,'unchanged sender restart runtime limits')
    require(plan['additional_network_seconds']==90 and plan['additional_network_mode']=='diagnostic_only_after_original_window' and plan['original_window_origin']=='before_first_connect_submit','original admission window preserved')
    path=root/plan['build_manifest'];require(root in path.resolve().parents and N['digest'](path)==plan['build_manifest_sha256'],'unchanged accepted image manifest')
    build=json.loads(path.read_bytes());O['validate_candidate_build'](build,root)
    N['validate'](plan,root,attempt_count=1,source_paths=SOURCES,product_pins=build['images'])

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('manifest',type=Path);p.add_argument('--sha256',required=True);p.add_argument('--run',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
    require(N['digest'](a.manifest)==a.sha256,'reviewed sender plan');plan=json.loads(a.manifest.read_bytes());root=SCRIPTS.parent;validate(plan,root)
    if a.run:
        require(a.evidence is not None and a.evidence.resolve()==(root/plan['execution']['evidence']).resolve(),'fresh bound sender evidence')
        N['execute'](plan,root,a.evidence.resolve(),normal_factory=SenderRestart,result_name='single-sender-restart-suite-result.json')
    else:print(json.dumps(dict(validated=True,attempts=plan['attempts'],messages=4,sender_restarts=1,recipient_restarts=1,host_mutations=0)))
if __name__=='__main__':main()
