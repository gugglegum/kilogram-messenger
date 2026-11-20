"""One opt-in old-A persistence-runtime crash; all other scopes use normal gates."""
import argparse,base64,copy,hashlib,json,runpy,time,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
R=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-sender-restart.py'));O,M,N,D=R['O'],R['M'],R['N'],R['D'];C=N['C']
require=R['require'];save=R['save'];PURPOSE='sender-runtime-crash-v1';ATTEMPT='hint-crash-02'
SOURCES=R['SOURCES']|{'scripts/test-terminal-t1-3e-sender-crash.py','scripts/test-terminal-t1-3e-sender-crash-check.py',
    'scripts/test-terminal-t1-3e-sender-crash-native.ps1','scripts/test-terminal-t1-3e-lab-agent.ps1','scripts/test-terminal-t1-3e-lab-common.ps1'}
SOURCES|={'scripts/test-terminal-t1-3e-sender-crash-policy.ps1'}
RUNTIME_FINALS={'runtime_stopping','runtime_network_stopped','runtime_final'}

class CrashAttempt(C['Attempt']):
    def start(self,mailbox,mode,end,**args):
        require(mode=='chat' and mailbox.tag=='owner-a-1' and args==dict(observe_socket=True),'one old-A fault owner')
        self.agent.call('owner',attempt=self.attempt,generation=self.generation,tag=mailbox.tag,
            owner_generation=mailbox.generation,mode=mode,fake_mode='',expected_runtime_crash=True)
        return C['Owner'](self,mailbox.tag,mailbox.generation)

def crashed_diagnostics(reply,nonce,chat_pid,runtime_pid,instance):
    records,coverage=M['shutdown_records'](reply,nonce,chat_pid,runtime_pid,instance)
    require(not coverage['complete'] and set(coverage['missing'])==RUNTIME_FINALS and not coverage['rejected'],'only crashed runtime finals may be missing')
    expected_startup={'startup_'+k for k in ['setup_open','begin','import_begin','import_end','profile_begin','prepared','spawned','descriptor','status']}
    require(set(coverage['startup_kinds'])==expected_startup,'crashed scope startup records incomplete')
    parsed={json.loads(raw)['kind']:json.loads(raw)['data'] for raw in records.values()}
    c,w=parsed['connection_final'],parsed['worker_final']
    require(c['was_running'] is False and c['child_query_failed'] is False and c['ack_attempted'] is False and c['ack'] is False and
        c['forced'] is False and c['exit_success'] is False and c['exit']==137 and c['eof'] is True and c['drain_count']==2 and
        c['drain_eof']==[True,True] and c['drain_failed']==[False,False],'crashed connection must reap/drain exact injected child')
    require(all(w[k] is True for k in ['running_ok','saved_ok','reaped','eof']) and w['stop_graceful'] is False and
        w['stop_forced'] is False and w['exit']==137,'crashed worker must finish without forced cleanup')
    return records,coverage

class SenderCrash(R['SenderRestart']):
    def __init__(self,*args):
        super().__init__(*args);self.crash=None;self.crash_classified=False

    def start(self,index,tag,mode,end=None):
        fault=index==0 and tag=='owner-a-1' and mode=='chat'
        stale=index==0 and tag=='owner-b-1' and mode=='chat'
        if not fault and not stale:return super().start(index,tag,mode,end)
        if stale:require(self.crash_classified,'reopen requires conclusive old-A crash cleanup')
        term=D['MailboxTerminal'](self.hosts[index],tag,mode,end or self.phase_end,
            interactive=(CrashAttempt if fault else C['Attempt'])(self.agents[index],self.attempt,self.generations[index]),observe_socket=True)
        self.processes.append((index,term));ready=term.line(term.deadline)
        require(ready==dict(v=1,type='ready') and term.call('status')==dict(connected=False),'disconnected Chat readiness')
        proof=self.observe(index,'preconnect',term.deadline,tag=tag,**(dict(crash_restart=True,crash_contexts=self.contexts) if stale else {}))
        require(proof['disconnected'] and not proof['sockets'] and not proof['runtime_pids'],'Chat preconnect ownership')
        require(proof['descriptor'] is stale and (not stale or proof['crash_descriptor_sha256']==self.crash['intent']['descriptor_sha256']),'exact expected stale descriptor')
        save(self.evidence/f'A-{tag}-prestart.json',proof)
        self.chat_pids[index]=proof['chat_pid'];self.latest[index]=None
        self.incarnations[index,tag]=dict(chat_pid=proof['chat_pid'],snapshot=None,collected=False)
        return term

    def connect(self,index,warm):
        super().connect(index,warm)
        if index==0 and self.crash_classified:
            metadata=self.hosts[0].call('lab-crash-descriptor',timeout=M['budget'](self.end),**self.hosts[0].lab_identity)
            old=self.crash['intent']
            require(metadata['sha256']!=old['descriptor_sha256'] and metadata['profile_name']==old['profile_name'] and
                metadata['profile_sha256']==old['profile_sha256'],'product must replace only stale descriptor, preserve exact profile')
            save(self.evidence/'reopened-descriptor.json',metadata)

    def clean_stop(self,index,label):
        if index!=0:return super().clean_stop(index,label)
        require(label=='sender-offline' and self.crash is None,'one injected old-A scope')
        self.checkpoint('before-runtime-crash');term=self.pair[0];host=self.hosts[0]
        doc,point=D['until'](lambda end:self.sample(0,term,end),60,self.end,'precrash runtime snapshot')
        socket=json.loads((self.evidence/f'A-{term.tag}-socket.json').read_bytes())
        descriptor=host.call('lab-crash-descriptor',timeout=M['budget'](self.end),**host.lab_identity)
        raw=(self.evidence/'offline-pending.json').read_bytes();proof_sha=hashlib.sha256(raw).hexdigest()
        put=host.call('put',name='crash-pending.json',sha256=proof_sha,data=base64.b64encode(raw)+b'\n',timeout=M['budget'](self.end))
        require(put['sha256']==proof_sha,'pending proof transfer')
        intent=dict(v=1,request=uuid.uuid4().hex,session=self.agents[0].session,attempt=self.attempt,attempt_generation=self.generations[0],
            owner_tag=term.tag,owner_generation=term.mailbox.generation,worker_pid=host.lab_identity['worker_pid'],worker_started=host.lab_identity['worker_started'],
            runtime_pid=socket['pid'],runtime_started=socket['started'],runtime_image=socket['image'],runtime_sha256=self.plan['pins']['kilogram-cli.exe'],
            runtime_instance=term.instance,descriptor_sha256=descriptor['sha256'],proof_sha256=proof_sha,contexts=self.contexts,
            profile_name=descriptor['profile_name'],profile_sha256=descriptor['profile_sha256'])
        save(self.evidence/'runtime-fault-intent.json',intent)
        # Retry the SAME immutable request only for transport uncertainty; host publication/handle execution are one-use.
        for attempt in range(2):
            try:outcome=host.call('lab-runtime-crash',fault=intent,timeout=M['budget'](self.end,maximum=15),**host.lab_identity);break
            except (OSError,TimeoutError,N['subprocess'].TimeoutExpired):
                if attempt:raise
        require(outcome['request']==intent['request'] and outcome['runtime_pid']==intent['runtime_pid'] and
            outcome['runtime_started']==intent['runtime_started'] and outcome['runtime_instance']==intent['runtime_instance'] and outcome['exit']==137,'injected death proof')
        self.crash=dict(intent=intent,result=outcome);save(self.evidence/'runtime-fault-result.json',outcome)
        self.port_gone(0,point['port'],'runtime-crashed',self.end)
        limit=min(self.end,time.monotonic()+30);quit_reply=term.result('quit',{},limit)
        require(quit_reply['ok'] is False and quit_reply['code']=='cleanup_unconfirmed','only expected postcrash frontend error')
        def reconciled(end):
            term.collect(end)
            state,owner=term.local_owner.sample(end)
            require(not state['blocked'] and state['result'] is None,'attempt must remain available for same-profile reopen')
            if owner and owner['state']=='expected_crash_reconciled':
                require(owner['exit']==1 and term.closed=='failed' and not term.queued and all(r['reply'] for r in term.requests.values()),'raw failed owner and complete mailbox drain')
                return owner
            require(not owner or owner['state']=='running','unexpected fault owner outcome')
        owner=D['until'](reconciled,30,self.end,'expected crash reconciliation unavailable')
        receipt=host.call('receipt',tag=term.tag,timeout=M['budget'](self.end))
        save(self.evidence/f'A-{term.tag}-job.json',receipt)
        require(receipt['child_exit']==3 and receipt['mailbox_eof'] and not receipt['mailbox_quit'] and receipt['membership'] and
            not receipt['forced'] and not receipt['before'] and not receipt['after'] and receipt['descriptor'] and not receipt['work_expired'] and
            receipt['error']=='Mailbox launch, stop or exchange failed','expected crash receipt must retain conclusive cleanup')
        self.collect_shutdown(0,term);require(not self.capture_errors,'crashed scope evidence incomplete beyond permitted finals')
        self.receipts.add((0,term.tag));self.crash_classified=True
        save(self.evidence/'sender-crash-stopped.json',dict(runtime=doc,endpoint=point,owner=owner,job=receipt,expected_fault=True))
        return doc

    def collect_shutdown(self,index,term):
        if not (self.crash and index==0 and term.tag=='owner-a-1'):return super().collect_shutdown(index,term)
        inc=self.incarnations[index,term.tag]
        if inc['collected']:return
        inc['collected']=True
        try:
            runtime=inc['snapshot']['doc']['runtime_pid']
            reply=self.hosts[index].call('lab-diagnostic-read',timeout=M['budget'](self.end),diagnostic_chat_pid=inc['chat_pid'],
                diagnostic_runtime_pid=runtime,diagnostic_runtime_instance=term.instance,diagnostic_previous=[])
            folder=self.evidence/'shutdown/A'/term.tag;folder.mkdir(parents=True)
            save(folder/'collection.json',reply)
            records,coverage=crashed_diagnostics(reply,self.generations[index],inc['chat_pid'],runtime,term.instance)
            for name,raw in records.items():(folder/name).write_bytes(raw)
            save(folder/'coverage.json',coverage)
            save(folder/'expected-crash.json',dict(expected_runtime_crash=True,raw_complete=coverage['complete'],missing=coverage['missing']))
        except Exception as error:self.capture_errors.append(dict(side='A',tag=term.tag,error=M['error_record'](error)))

    def run(self):
        passed=False
        try:
            ready=self.agents[0].ready()
            require(ready.get('crash_policy_version')=='owned-runtime-crash-v1' and ready.get('crash_policy_sha256')==self.plan['nodes'][0]['crash_policy_sha256'],
                'A must have the reviewed crash policy loaded, not only changed files on disk')
            M['Diagnostic'].run(self)
            require(self.crash_classified and self.delivery_history_passed and self.sender_restarts==self.recipient_restarts==1,'crash recovery incomplete')
            self.command_coverage=self.command_verdict()
            self.notification_coverage=dict(evaluated=True,required_passed=False,required=sum(O['notification_required'](r,self.anchor_present) for r in self.messages),
                observed=sum(len(t.events) for _,t in self.processes),not_covered=None)
            self.notification_coverage=O['notification_verdict'](self.messages,self.processes,self.anchor_present)
            agents=json.loads((self.evidence/'result.json').read_bytes())['agents']
            require(agents['A']['owners_clean']==2 and agents['A']['owners_expected_crash']==1 and agents['B']['owners_clean']==3 and
                agents['B'].get('owners_expected_crash',0)==0,'one expected old-A fault, all other owners normal')
            passed=True
        finally:
            save(self.evidence/'notifications-observed.json',dict(schema=1,processes=[dict(host_index=i,tag=t.tag,instance=getattr(t,'instance',None),epoch=getattr(t,'epoch',None),events=t.events) for i,t in self.processes]))
            save(self.evidence/'sender-crash-result.json',dict(schema=1,purpose=PURPOSE,passed=passed,expected_crash=self.crash,
                crash_classified=self.crash_classified,messages=self.messages,post_sender_restart_proofs=self.recovery_proofs,
                delivery_history=dict(passed=self.delivery_history_passed),notification_coverage=self.notification_coverage,command_coverage=self.command_coverage))

def validate(plan,root):
    require(plan['purpose']==PURPOSE and plan['attempts']==[ATTEMPT],'single reviewed crash attempt')
    e=plan['execution']
    require(e['evidence']=='.tmp/terminal-t1-3e/sender-crash-live-02' and e['controller_seconds']==1800 and e['outer_seconds']==1860 and
        e['priority']=='BelowNormal' and e['max_parallel_hosts']==2 and e['runtime_review_required'] is True,'crash execution bounds')
    require(plan['work_seconds']==1000 and plan['connect_seconds']==85 and plan['admission_seconds']==plan['reconnect_seconds']==90 and
        plan['probe_seconds']==60 and plan['minimum_call_seconds']==2 and plan['additional_network_seconds']==90 and
        plan['additional_network_mode']=='diagnostic_only_after_original_window' and plan['original_window_origin']=='before_first_connect_submit','unchanged timing')
    manifest=root/plan['build_manifest'];require(root in manifest.resolve().parents and N['digest'](manifest)==plan['build_manifest_sha256'],'accepted image manifest')
    build=json.loads(manifest.read_bytes());O['validate_candidate_build'](build,root)
    node=plan['nodes'][0]
    require(node['script_sha']==N['digest'](root/'scripts/test-terminal-t1-3e-lab-agent.ps1') and
        node['payload_pins']['test-terminal-t1-3e-lab-common.ps1']==N['digest'](root/'scripts/test-terminal-t1-3e-lab-common.ps1') and
        D['HEX'].fullmatch(node['crash_policy_sha256']),'reviewed A payload and loaded policy binding')
    N['validate'](plan,root,attempt_count=1,source_paths=SOURCES,product_pins=build['images'])

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('manifest',type=Path);p.add_argument('--sha256',required=True);p.add_argument('--run',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
    require(N['digest'](a.manifest)==a.sha256,'reviewed crash plan');plan=json.loads(a.manifest.read_bytes());validate(plan,ROOT)
    if a.run:
        require(a.evidence and a.evidence.resolve()==(ROOT/plan['execution']['evidence']).resolve(),'bound fresh crash evidence')
        N['execute'](plan,ROOT,a.evidence.resolve(),normal_factory=SenderCrash,result_name='single-sender-crash-suite-result.json')
    else:print(json.dumps(dict(validated=True,attempts=plan['attempts'],host_mutations=0)))
if __name__=='__main__':main()
