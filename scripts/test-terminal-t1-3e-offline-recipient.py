"""One graceful recipient stop/reconnect; four messages; existing A2 binaries and lab lifecycle."""
import argparse,json,runpy,time
from pathlib import Path
SCRIPTS=Path(__file__).resolve().parent
S=runpy.run_path(str(SCRIPTS/'test-terminal-t1-3e-hint-smoke.py'))
M=S['M'];N=S['N'];D=S['D'];require=S['require'];save=S['save']
PURPOSE='offline-recipient-v1'
ATTEMPT='hint-offline-02'
SOURCES=S['SOURCES']|{'scripts/test-terminal-t1-3e-offline-recipient.py','scripts/test-terminal-t1-3e-offline-recipient-check.py'}

def verify_initial_history(snapshots,contexts,messages,hosts):
    require(not messages,'initial history requires no prior sends')
    require(len(contexts)==2 and all(D['HEX'].fullmatch(c or '') for c in contexts) and len(set(contexts))==2,'two installed initial contexts required')
    require(len(hosts)==2 and len(set(hosts))==2 and isinstance(snapshots,list) and len(snapshots)==2,'two initial history hosts required')
    require(all(isinstance(s,dict) and s.get('host') in hosts for s in snapshots) and {s['host'] for s in snapshots}==set(hosts),'initial history hosts differ')
    for snapshot in snapshots:
        entries=snapshot.get('contexts')
        require(isinstance(entries,list) and len(entries)==2 and all(isinstance(c,dict) and c.get('context') in contexts for c in entries),'initial history contexts differ')
        require({c['context'] for c in entries}==set(contexts),'initial history contexts differ')
        require(all(isinstance(c.get('rows'),list) and not c['rows'] for c in entries),'initial history rows must be empty')

def materialized_pending(row,status,history):
    require(status['state'] in D['PENDING'],'offline request was not pending')
    if status['state']!='materialized':return False
    event=status.get('event_id');require(D['HEX'].fullmatch(event or ''),'offline materialized event')
    require(not row.get('pending_event') or row['pending_event']==event,'offline event changed')
    found=[r for r in history if r['event_id']==event or r['text']==row['text']]
    require(len(found)==1 and found[0]['event_id']==event and found[0]['text']==row['text'],'durable sender history mismatch')
    row['pending_event']=event
    return True

def notification_required(row,already_present):
    return row['sender']==1 or row['event_id'] not in already_present

def notification_verdict(messages,processes,already_present):
    require(len(messages)==4 and {(r['sender'],r['slot']) for r in messages}=={(s,c) for s in (0,1) for c in (0,1)},'four exact logical messages')
    expected={r['event_id']:r for r in messages};require(len(expected)==4,'duplicate message event identity')
    observed={event:[] for event in expected}
    for index,term in processes:
        for notice in term.events:
            event=notice.get('event_id');require(event in expected,'unexpected notification event')
            row=expected[event]
            require(index==1-row['sender'] and term.instance==row['receiver_instance'] and term.epoch==row['receiver_epoch'],'notification source incarnation')
            require(D['prior']['incoming_result']([notice],row)['status']=='confirmed','notification binding mismatch')
            observed[event].append(notice);require(len(observed[event])==1,'duplicate notification event')
    details=[]
    for row in messages:
        required=notification_required(row,already_present);notices=observed[row['event_id']]
        require(not required or len(notices)==1,'required live notification missing')
        details.append(dict(event_id=row['event_id'],slot=row['slot'],sender=row['sender'],required=required,
            coverage='confirmed' if notices else 'unproven_at_anchor',notification=D['prior']['incoming_result'](notices,row)))
    unproven=sum(r['coverage']=='unproven_at_anchor' for r in details)
    return dict(evaluated=True,required_passed=True,coverage_complete=unproven==0,required=sum(r['required'] for r in details),
        observed=sum(len(v) for v in observed.values()),not_covered=unproven,per_message=details)

class OfflineRecipient(M['Diagnostic']):
    def __init__(self,*args):
        super().__init__(*args);self.incarnations={};self.offline_rows=[];self.recipient_restarts=0
        self.anchor_present=set();self.delivery_history_passed=False;self.notification_coverage=dict(evaluated=False)

    def initial_histories(self,end):
        snapshots=self.collect_histories(end)
        save(self.evidence/'initial-history.json',snapshots)
        verify_initial_history(snapshots,self.contexts,self.messages,[h.name for h in self.hosts])

    def start(self,index,tag,mode,end=None):
        term=super().start(index,tag,mode,end)
        if mode=='chat':
            self.latest[index]=None
            self.incarnations[index,tag]=dict(chat_pid=self.chat_pids[index],snapshot=None,collected=False)
        return term

    def sample(self,index,term,end):
        result=super().sample(index,term,end)
        if self.latest[index] and self.latest[index]['doc']['runtime_instance']==term.instance:
            self.incarnations[index,term.tag]['snapshot']=self.latest[index]
        return result

    def collect_shutdown(self,index,term):
        if term.mode!='chat':return
        incarnation=self.incarnations[index,term.tag]
        if incarnation['collected']:return
        incarnation['collected']=True
        try:
            cached=incarnation['snapshot'];runtime_pid=cached['doc']['runtime_pid'] if cached else 0
            previous=[dict(diagnostic_chat_pid=v['chat_pid'],diagnostic_runtime_pid=v['snapshot']['doc']['runtime_pid'],diagnostic_runtime_instance=v['snapshot']['doc']['runtime_instance'])
                for (i,tag),v in self.incarnations.items() if i==index and tag!=term.tag and v['collected'] and v['snapshot']]
            reply=self.hosts[index].call('lab-diagnostic-read',timeout=M['budget'](self.end),diagnostic_chat_pid=incarnation['chat_pid'],
                diagnostic_runtime_pid=runtime_pid,diagnostic_runtime_instance=term.instance if cached else None,diagnostic_previous=previous)
            folder=self.evidence/'shutdown'/self.hosts[index].name/term.tag;folder.mkdir(parents=True)
            records,coverage=M['shutdown_records'](reply,self.generations[index],incarnation['chat_pid'],runtime_pid,term.instance if cached else None)
            for name,raw in records.items():(folder/name).write_bytes(raw)
            save(folder/'collection.json',reply);save(folder/'coverage.json',coverage)
            if not coverage['complete']:self.capture_errors.append(dict(side=self.hosts[index].name,tag=term.tag,error=dict(type='IncompleteShutdownDiagnostics',coverage=coverage)))
        except Exception as error:self.capture_errors.append(dict(side=self.hosts[index].name,tag=term.tag,error=M['error_record'](error)))

    def receipt(self,index,term,cleanup=False):
        try:return N['Normal'].receipt(self,index,term,cleanup)
        finally:
            if term.mode=='chat':self.collect_shutdown(index,term)

    def pending_persisted(self,row):
        sender=self.pair[0]
        def sample(end):
            status=sender.call('request_status',end,**row['key'])
            history=D['history'](sender,row['key']['context'],end)
            return dict(request=status,sender_history=history) if materialized_pending(row,status,history) else None
        return D['until'](sample,35,self.end,'offline materialized request not persisted')

    def deliver(self,row,end,*,required_notification=True):
        sender=self.pair[row['sender']];receiver=self.pair[1-row['sender']]
        def check(limit):
            value=sender.call('request_status',limit,**row['key'])
            return value if value['state']=='delivered' else None
        result=D['until'](check,35,end,'message did not reach delivered')
        event=result['event_id']
        require(D['HEX'].fullmatch(event) and (not row.get('pending_event') or event==row['pending_event']),'request mapped to a different event')
        row.update(event_id=event,delivery=result,receiver_instance=receiver.instance,receiver_epoch=receiver.epoch,notification_required=required_notification)
        row['notification']=D['prior']['observe_incoming'](receiver,row,end)
        require(row['notification']['status'] in ('confirmed','not_observed'),'duplicate or misbound receiver notification')
        require(not required_notification or row['notification']['status']=='confirmed','required live notification missing')

    def functional(self):
        # Inherited fresh Setup/Auto, initial original90s gate, typed diagnostics.
        super().functional();self.monitors('A',self.end,watching=True);self.initial_histories(self.end)
        sender_instance=self.pair[0].instance;old_receiver=self.pair[1]
        previous=self.sample(1,old_receiver,self.end);require(previous is not None,'recipient baseline unavailable')
        old_doc,old_point=previous
        self.stop(1,old_receiver);self.port_gone(1,old_point['port'],'recipient-offline',self.end)
        require(not self.capture_errors,'recipient shutdown diagnostics incomplete')
        receipt=json.loads((self.evidence/f'B-{old_receiver.tag}-job.json').read_bytes())
        require(receipt['child_exit']==0 and receipt['mailbox_quit'] and receipt['mailbox_eof'] and not receipt['forced'] and not receipt['after'] and not receipt['descriptor'],'recipient graceful stop')
        save(self.evidence/'recipient-stopped.json',dict(instance=old_receiver.instance,runtime=old_doc,endpoint=old_point,job=receipt))
        pending=[]
        for slot in (0,1):
            row=self.send(0,slot,f'offline-{slot}',self.end)
            first=self.pending_persisted(row) # Retires the protected intent before the next send.
            self.offline_rows.append(row);pending.append(dict(slot=slot,key=row['key'],first=first))
        for row,proof in zip(self.offline_rows,pending):proof['second']=self.pending_persisted(row)
        require(not old_receiver.events,'recipient notification before restart')
        save(self.evidence/'offline-pending.json',dict(recipient_stopped=True,sender_instance=sender_instance,requests=pending))
        self.pair[1]=self.start(1,'owner-b-1','chat');self.recipient_restarts+=1
        started=time.monotonic();self.connect(1,True)
        require(self.pair[1].instance!=old_receiver.instance and self.pair[0].instance==sender_instance,'recipient-only fresh instance')
        # A fresh Chat anchors at the current tail; sample it before waiting for admission.
        self.monitors('B',self.end,watching=True)
        boundary=[dict(context=c,rows=D['history'](self.pair[1],c,self.end)) for c in self.contexts]
        self.anchor_present={r['event_id'] for item in boundary for r in item['rows']}
        save(self.evidence/'recipient-notification-boundary.json',dict(instance=self.pair[1].instance,epoch=self.pair[1].epoch,history=boundary,
            note='Already-present queued events may have unproven notification coverage; delivery/history still mandatory. All observed events are reconciled after cleanup.'))
        def admitted(rows):return all(r['point'] for r in rows) and D['live_pair']([r['doc'] for r in rows],[r['point'] for r in rows])
        gate=M['admission_gate'](self.probe,admitted,started=started,end=self.end)
        save(self.evidence/'reconnect-admission.json',gate)
        if not gate['admitted']:raise M['AdmissionFailure']('recipient reconnect original90s gate failed')
        new_doc=gate['last_complete_runtime']['B']['doc']
        require(all(new['own_locator_revision']>old['own_locator_revision'] for new,old in zip(new_doc['slots'],old_doc['slots'])),'recipient locator revision did not advance')
        for row in self.offline_rows:
            self.deliver(row,self.end,required_notification=row['pending_event'] not in self.anchor_present)
        self.histories('queued-delivered',self.end)
        for slot in (0,1):self.deliver(self.send(1,slot,f'reply-{slot}',self.end),self.end)
        self.histories('offline-final',self.end)
        require(len(self.messages)==4 and {(r['sender'],r['slot']) for r in self.messages}=={(s,c) for s in (0,1) for c in (0,1)},'offline/reply exact messages')
        self.delivery_history_passed=True

    def run(self):
        passed=False
        try:
            super().run()
            require(self.delivery_history_passed,'delivery/history verdict incomplete')
            self.notification_coverage=dict(evaluated=True,required_passed=False,
                required=sum(notification_required(r,self.anchor_present) for r in self.messages),
                observed=sum(len(t.events) for _,t in self.processes),not_covered=None)
            self.notification_coverage=notification_verdict(self.messages,self.processes,self.anchor_present)
            passed=True
        finally:
            save(self.evidence/'notifications-observed.json',dict(schema=1,processes=[dict(host_index=i,tag=getattr(t,'tag',None),
                instance=getattr(t,'instance',None),epoch=getattr(t,'epoch',None),events=t.events) for i,t in self.processes]))
            save(self.evidence/'offline-recipient-result.json',dict(schema=2,purpose=PURPOSE,passed=passed,messages=self.messages,expected_messages=4,
                delivery_history=dict(passed=self.delivery_history_passed),notification_coverage=self.notification_coverage,
                recipient_restarts=self.recipient_restarts,expected_recipient_restarts=1,sender_restart=False,cleanup_in_application_and_diagnostic_receipts=True))

def validate(plan,root):
    require(plan['purpose']==PURPOSE and plan['attempts']==[ATTEMPT],'single recipient-only offline attempt')
    require(plan['connect_seconds']==85 and plan['admission_seconds']==90 and plan['reconnect_seconds']==90 and plan['probe_seconds']==60 and plan['minimum_call_seconds']==2 and
        plan['additional_network_seconds']==90 and plan['additional_network_mode']=='diagnostic_only_after_original_window' and plan['original_window_origin']=='before_first_connect_submit','measured startup/reconnect budgets')
    path=root/plan['build_manifest'];require(root in path.resolve().parents and N['digest'](path)==plan['build_manifest_sha256'],'reused A2 build pin')
    build=json.loads(path.read_bytes());require(build['commit']=='a61385a00e12fbe0b78805fe7655afb5b3d6bcbe' and build['clean_published_worktree'],'published unchanged A2 images')
    for row in build['inputs']:
        source=root/row['path'];require(root in source.resolve().parents and N['digest'](source)==row['sha256'],'A2 build input pin')
    N['validate'](plan,root,attempt_count=1,source_paths=SOURCES,product_pins=build['images'])

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('manifest',type=Path);parser.add_argument('--sha256',required=True)
    parser.add_argument('--run',action='store_true');parser.add_argument('--evidence',type=Path);args=parser.parse_args()
    require(N['digest'](args.manifest)==args.sha256,'reviewed offline plan');plan=json.loads(args.manifest.read_bytes());root=SCRIPTS.parent;validate(plan,root)
    if args.run:
        require(args.evidence is not None and root in args.evidence.resolve().parents,'fresh offline evidence')
        N['execute'](plan,root,args.evidence.resolve(),normal_factory=OfflineRecipient,result_name='single-offline-recipient-suite-result.json')
    else:print(json.dumps(dict(validated=True,attempts=plan['attempts'],messages=4,recipient_restarts=1,host_mutations=0)))

if __name__=='__main__':main()
