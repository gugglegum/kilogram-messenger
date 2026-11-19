"""One fresh Auto LAN admission observation; no message/offline/restart scenario."""
import argparse
import base64
import hashlib
import json
import re
from pathlib import Path
import runpy
import subprocess
import time
import uuid

SCRIPTS=Path(__file__).resolve().parent
N=runpy.run_path(str(SCRIPTS/'test-terminal-t1-3e-normal.py'))
D=N['D'];require=N['require'];save=N['save'];parallel=N['parallel']
SOURCES=N['SOURCES']|{'scripts/test-terminal-t1-3e-admission-diagnostic.py','scripts/test-terminal-t1-3e-admission-diagnostic-check.py'}
RUST_SOURCES={
    'crates/kilogram-runtime-ipc/src/lib.rs','crates/kilogram-runtime-ipc/src/lab_diagnostics.rs',
    'crates/kilogram-first-dialog/src/connection.rs',
    *('apps/kilogram-cli/src/ordered_dialog/runtime2/'+name for name in ('mod.rs','owner.rs','diagnostics.rs','discovery.rs','runtime.rs','lan_runtime.rs'))}
MIN_CALL=2.0
PROBE_SECONDS=24.0
ADMISSION_SECONDS=90.0

class ObservationBudgetExhausted(RuntimeError):pass
class AdmissionFailure(RuntimeError):pass

def error_record(error):
    value=dict(type=type(error).__name__,message=str(error))
    if isinstance(error,subprocess.TimeoutExpired):
        def content(value):return value.encode('utf-8') if isinstance(value,str) else value or b''
        value.update(timeout=error.timeout,stdout_sha256=hashlib.sha256(content(error.stdout)).hexdigest(),stderr_sha256=hashlib.sha256(content(error.stderr)).hexdigest())
    return value

def budget(end,maximum=12,*,now=time.monotonic):
    seconds=min(maximum,end-now())
    if seconds<MIN_CALL:raise ObservationBudgetExhausted(f'no new subprocess: remaining={seconds:.6f}, minimum={MIN_CALL}')
    return seconds

def admission_gate(probe,predicate,*,started,end,now=time.monotonic,sleep=time.sleep):
    """Observation timestamps decide the90s gate, never a later diagnostic snapshot."""
    cutoff=started+ADMISSION_SECONDS
    report=dict(schema=1,stage='lan_admission',started=started,deadline=cutoff,admitted=False,
        last_complete_runtime={},rounds=[],raw_cause=None,after_deadline=None)
    stop=min(cutoff,end)
    while now()<stop and len(report['rounds'])<48:
        sent=now()
        if min(cutoff,end)-sent<MIN_CALL:break
        # A whole bounded observation may retire after cutoff; it cannot create a late pass.
        limit=min(end,sent+PROBE_SECONDS)
        try:rows=probe(limit)
        except Exception as error:
            report['raw_cause']=error_record(error);break
        received=now();round_doc=dict(sent=sent,received=received,probe_deadline=limit,hosts=rows)
        report['rounds'].append(round_doc)
        complete=[row for row in rows if row.get('runtime_complete')]
        for row in complete:report['last_complete_runtime'][row['side']]=row
        if received>limit:
            report['raw_cause']=dict(type='ObservationDeadline',message='observer exceeded its independent observation budget',deadline=limit,received=received)
            break
        if len(complete)==2:
            observed=max(row['runtime_received'] for row in complete)
            if observed<cutoff and observed<=received and predicate(complete):
                report.update(admitted=True,admitted_observed=observed,decision_at=received)
                return report
        failures=[row['error'] for row in rows if row.get('error')]
        if failures:
            report['raw_cause']=dict(type='IncompleteObservation',hosts=failures)
            break
        if received>=cutoff:
            report['after_deadline']=dict(label='late_observation_not_admission',**round_doc)
            break
        sleep(min(.15,max(0,cutoff-now())))
    report['decision_at']=now()
    if report['raw_cause'] is None:
        if end<cutoff:report['raw_cause']=dict(type='CleanupReserve',message='no further observation may consume cleanup reserve',deadline=end)
        elif len(report['rounds'])>=48:report['raw_cause']=dict(type='ObservationRoundCap',message='bounded observation count exhausted')
        else:report['raw_cause']=dict(type='AdmissionDeadline',message='four authenticated live leases not observed before deadline')
    # A separate bounded diagnostic observation; never fed into predicate/admitted.
    if report['after_deadline'] is None:
        if report['raw_cause']['type']=='AdmissionDeadline' and now()<stop:sleep(max(0,stop-now()))
        if end-now()>=MIN_CALL:
            sent=now();limit=min(end,sent+PROBE_SECONDS)
            label='after_deadline_diagnostic_only' if sent>=cutoff else 'after_failure_diagnostic_only'
            try:report['after_deadline']=dict(label=label,sent=sent,received=None,probe_deadline=limit,hosts=probe(limit));report['after_deadline']['received']=now();report['after_deadline']['budget_exceeded']=now()>limit
            except Exception as error:report['after_deadline']=dict(label=label,sent=sent,received=now(),probe_deadline=limit,error=error_record(error))
        else:report['after_deadline']=dict(label='unavailable_cleanup_reserve',at=now())
    return report

class Host(N['Host']):
    @property
    def minimum_subprocess_seconds(self):return MIN_CALL if getattr(self,'observation_active',False) else 0

    def call(self,op,*args,**kwargs):
        if op in ('lab-diagnostic-enable','lab-diagnostic-read'):kwargs.update(self.lab_identity)
        if getattr(self,'observation_active',False):
            seconds=kwargs.get('timeout')
            if seconds is not None and seconds<MIN_CALL:raise ObservationBudgetExhausted('no tiny-budget host subprocess')
        return super().call(op,*args,**kwargs)

    def socket(self,tag,end):
        row=self.call('socket',tag=tag,timeout=budget(end))
        if not row['observed']:return False
        require(row['ip']==self.config['ip'] and self.udp[0]<=row['port']<=self.udp[1],'diagnostic LAN UDP socket')
        path=self.evidence/f'{self.name}-{tag}-socket.json'
        if not path.exists():save(path,row)
        return True

def full_status(result):
    frame=result['full_status'];require(frame['schema']==1 and type(frame['bytes']) is int and 0<frame['bytes']<=49152,'full status frame cap')
    raw=base64.b64decode(frame['data'],validate=True)
    require(len(raw)==frame['bytes'] and hashlib.sha256(raw).hexdigest()==frame['sha256'],'full status content pin')
    value=json.loads(raw)
    require(value['version']==2 and value['runtime_pid']==result['runtime_pid'] and value['runtime_instance']==result['runtime_instance'],'full status native identity')
    return raw

def shutdown_records(reply,nonce,chat_pid,runtime_pid,instance):
    require(reply['schema']==1 and reply['run_nonce']==nonce and len(reply['records'])<=16,'shutdown receipt scope')
    records={}
    for record in reply['records']:
        name=record['name']
        require(re.fullmatch(r'(runtime-stopping|runtime-network-stopped|runtime-final|connection-final|worker-final)-[1-9][0-9]*\.json',name) and name not in records,'shutdown receipt name')
        raw=base64.b64decode(record['data'],validate=True)
        require(0<len(raw)<=16384 and len(raw)==record['bytes'] and hashlib.sha256(raw).hexdigest()==record['sha256'],'shutdown receipt content pin')
        value=json.loads(raw)
        require(set(value)=={'schema','kind','run_nonce','pid','runtime_pid','runtime_instance','data'} and value['schema']==1 and value['run_nonce']==nonce,'shutdown envelope binding')
        require(name==value['kind'].replace('_','-')+'-'+str(value['pid'])+'.json','shutdown filename binding')
        if value['kind'].startswith('runtime_'):
            require(runtime_pid and value['pid']==runtime_pid and value['runtime_pid']==runtime_pid and value['runtime_instance']==instance,'shutdown runtime binding')
        else:
            require(value['pid']==chat_pid and (not runtime_pid or value['runtime_pid']==runtime_pid and value['runtime_instance']==instance),'shutdown Chat binding')
        records[name]=raw
    kinds={json.loads(raw)['kind'] for raw in records.values()}
    expected={'runtime_stopping','runtime_network_stopped','runtime_final','connection_final','worker_final'}
    return records,dict(complete=kinds==expected and not reply.get('rejected'),missing=sorted(expected-kinds),rejected=reply.get('rejected',[]))

class Diagnostic(N['Normal']):
    def __init__(self,*args):
        super().__init__(*args,host_class=Host)
        self.latest=[None,None];self.full=[None,None];self.chat_pids=[None,None];self.capture_errors=[];self.collected=set()

    def observation_timeout(self,end):return budget(end)

    def start(self,index,tag,mode,end=None):
        term=super().start(index,tag,mode,end)
        if mode=='chat':self.chat_pids[index]=json.loads((self.evidence/f'{self.hosts[index].name}-{tag}-prestart.json').read_bytes())['chat_pid']
        return term

    def observe(self,index,op,end,**params):
        budget(end)
        if op=='status':params['full_status']=True
        result=super().observe(index,op,end,**params)
        if op=='status' and result.get('ready'):
            frame=result['full_status'];raw=full_status(result)
            folder=self.evidence/'full-status';folder.mkdir(exist_ok=True);path=folder/f'{self.hosts[index].name}-{uuid.uuid4().hex}.json';path.write_bytes(raw)
            result['full_status']=dict(schema=1,bytes=len(raw),sha256=frame['sha256'],file=str(path.relative_to(self.evidence)))
            self.full[index]=result['full_status']
        return result

    def connect(self,index,warm):
        term=self.pair[index];value=term.call('connect',end=min(self.end,time.monotonic()+25),mode='start')
        require(value['mode']=='owner' and value['contexts']==self.contexts and D['HEX'].fullmatch(value['runtime_instance']) and value['runtime_instance'] not in self.instances[index],'fresh diagnostic runtime/preserved contexts')
        term.contexts=self.contexts;term.instance=value['runtime_instance'];term.epoch=None
        self.instances[index].add(term.instance)

    def sample(self,index,term,end):
        host=self.hosts[index]
        if not host.socket(term.tag,end):return None
        job=json.loads((self.evidence/f'{host.name}-{term.tag}-socket.json').read_bytes())
        args=dict(sha256=self.plan['pins']['kilogram-cli.exe'],runtime_pid=job['pid'],runtime_started=job['started'],runtime_image=job['image'])
        if index:args.update(owner_token=self.owner_tokens[index],owner_session=self.plan['nodes'][index]['windows_session'])
        doc=self.observe(index,'status',end,**args)
        if not doc['ready']:return None
        require(doc['runtime_instance']==term.instance,'diagnostic expected runtime instance')
        self.latest[index]=dict(doc=doc,point=None,received=time.monotonic())
        point=D['sample_endpoint'](index,term,doc,job,self.owner_tokens[index])
        require([s['role'] for s in doc['slots']]==[self.roles[index]]*2,'diagnostic consent roles')
        self.latest[index]['point']=point
        return doc,point

    def probe(self,end):
        def one(i):
            host=self.hosts[i];host.observation_active=True
            row=dict(side=host.name,runtime_complete=False,monitor_complete=False,error=None)
            try:
                sample=self.sample(i,self.pair[i],end)
                if sample:
                    row.update(runtime_complete=True,runtime_received=self.latest[i]['received'],doc=sample[0],point=sample[1])
                # Serial with this host's runtime observation and every other mailbox call.
                budget(end)
                status,monitor=self.pair[i].status(end)
                require(status['mode']=='owner' and status['runtime_instance']==self.pair[i].instance and status['contexts']==self.contexts,'Chat monitor binding')
                row.update(monitor_complete=True,monitor_received=time.monotonic(),chat_status=status,monitor=monitor)
            except Exception as error:
                row['error']=error_record(error)
                self.capture_errors.append(dict(side=host.name,stage='probe',error=row['error']))
            finally:host.observation_active=False
            return row
        return parallel(one,(0,1))

    def functional(self):
        self.contexts=D['prior']['onboard'](self.hosts,lambda h:self.start(int(h.name=='B'),'setup','setup'),self.evidence,self.transfers,self.select_route)
        for i,term in self.processes:self.receipt(i,term)
        for i,host in enumerate(self.hosts):
            enabled=host.call('lab-diagnostic-enable')
            require(enabled==dict(schema=1,run_nonce=self.generations[i],enabled=True),'fresh post-Setup diagnostic marker')
            save(self.evidence/f'{host.name}-diagnostic-enabled.json',enabled)
        self.pair=parallel(lambda i:self.start(i,'owner-a-1','chat'),(0,1))
        started=time.monotonic()
        try:parallel(lambda i:self.connect(i,False),(0,1))
        except Exception as error:
            report=dict(schema=1,stage='runtime_connect',started=started,deadline=started+ADMISSION_SECONDS,admitted=False,raw_cause=error_record(error),last_complete_runtime=self.latest)
            try:
                budget(self.end);sent=time.monotonic();limit=min(self.end,sent+PROBE_SECONDS)
                report['after_failure']=dict(label='after_connect_failure_diagnostic_only',sent=sent,probe_deadline=limit,hosts=self.probe(limit),received=time.monotonic())
            except Exception as capture:report['after_failure']=dict(label='connect_failure_snapshot_unavailable',error=error_record(capture))
            save(self.evidence/'admission-result.json',report)
            raise
        def admitted(rows):return all(row['point'] for row in rows) and D['live_pair']([row['doc'] for row in rows],[row['point'] for row in rows])
        result=admission_gate(self.probe,admitted,started=started,end=self.end)
        save(self.evidence/'admission-result.json',result)
        if not result['admitted']:raise AdmissionFailure('lan_admission: four leases not observed before90s; see admission-result.json raw_cause/last_complete_runtime')
        # Evidence goal reached; no message send, manual locator import, offline or restart.

    def collect_shutdown(self,i,term):
        if term.mode!='chat' or not self.chat_pids[i] or i in self.collected:return
        self.collected.add(i)
        try:
            cached=self.latest[i];runtime_pid=cached['doc']['runtime_pid'] if cached else 0
            reply=self.hosts[i].call('lab-diagnostic-read',timeout=budget(self.end),diagnostic_chat_pid=self.chat_pids[i],
                diagnostic_runtime_pid=runtime_pid,diagnostic_runtime_instance=term.instance if cached else None)
            folder=self.evidence/'shutdown'/self.hosts[i].name;folder.mkdir(parents=True)
            records,coverage=shutdown_records(reply,self.generations[i],self.chat_pids[i],runtime_pid,term.instance if cached else None)
            for name,raw in records.items():(folder/name).write_bytes(raw)
            save(folder/'collection.json',reply);save(folder/'coverage.json',coverage)
            if not coverage['complete']:self.capture_errors.append(dict(side=self.hosts[i].name,error=dict(type='IncompleteShutdownDiagnostics',coverage=coverage)))
        except Exception as error:self.capture_errors.append(dict(side=self.hosts[i].name,error=error_record(error)))

    def receipt(self,index,term,cleanup=False):
        try:return super().receipt(index,term,cleanup)
        finally:
            if cleanup:self.collect_shutdown(index,term)

    def run(self):
        try:super().run()
        finally:
            for i,term in self.processes:self.collect_shutdown(i,term)
            result=json.loads((self.evidence/'result.json').read_bytes()) if (self.evidence/'result.json').exists() else None
            save(self.evidence/'diagnostic-result.json',dict(schema=1,purpose='lan-admission-diagnostic-v1',application_result=result,
                capture_errors=self.capture_errors,capture_complete=not self.capture_errors,messages_sent=len(self.messages),no_second_attempt=True))
        require(not self.capture_errors,'diagnostic capture incomplete; application outcome retained separately')

def validate(plan,root):
    require(plan['purpose']=='lan-admission-diagnostic-v1' and plan['admission_seconds']==90 and plan['probe_seconds']==24 and plan['minimum_call_seconds']==2,'diagnostic purpose/budgets')
    path=root/plan['build_manifest'];require(root in path.resolve().parents and N['digest'](path)==plan['build_manifest_sha256'],'diagnostic build manifest')
    manifest=json.loads(path.read_bytes())
    require(manifest['base']=='32d54c6b114e9047992de31c943f6794bf3e917e' and manifest['unrelated_tests_excluded'] is True,'isolated diagnostic source base')
    require(set(manifest['diagnostic_sources'])==RUST_SOURCES and set(manifest['images'])=={'kilogram-cli.exe','kilogram-bootstrap.exe'},'complete diagnostic build inputs/images')
    for rel,pin in manifest['diagnostic_sources'].items():require(N['digest'](root/rel)==pin,'diagnostic product source pin')
    N['validate'](plan,root,attempt_count=1,source_paths=SOURCES,product_pins=manifest['images'])

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('manifest',type=Path);parser.add_argument('--sha256',required=True)
    parser.add_argument('--run',action='store_true');parser.add_argument('--evidence',type=Path);args=parser.parse_args()
    require(N['digest'](args.manifest)==args.sha256,'reviewed diagnostic plan pin');plan=json.loads(args.manifest.read_bytes());root=SCRIPTS.parent;validate(plan,root)
    if args.run:
        require(args.evidence is not None and root in args.evidence.resolve().parents,'fresh diagnostic evidence')
        N['execute'](plan,root,args.evidence.resolve(),normal_factory=Diagnostic,result_name='single-diagnostic-suite-result.json')
    else:print(json.dumps(dict(validated=True,attempts=plan['attempts'],diagnostic_only=True,host_mutations=0)))

if __name__=='__main__':main()
