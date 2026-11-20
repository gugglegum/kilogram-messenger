"""Offline adapter scope/persistence/notification/diagnostic boundaries. Synthetic files only."""
import base64,copy,json,runpy,subprocess,sys,time,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1]
M=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-offline-recipient.py'))
E=Path(sys.argv[1]).resolve();E.mkdir(parents=True,exist_ok=False);NOTIFICATION_ONLY='--notification-only' in sys.argv[2:];INITIAL_ONLY='--initial-only' in sys.argv[2:];sys.argv=sys.argv[:1]
PS=r'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
def save(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2),encoding='utf-8')

class Checks(unittest.TestCase):
    def initial_fixture(self,label):
        field=object.__new__(M['OfflineRecipient']);field.evidence=E/label;field.evidence.mkdir();field.contexts=['a'*64,'b'*64];field.messages=[]
        field.hosts=[SimpleNamespace(name=s) for s in ['A','B']];field.calls=[];field.rows=[{c:[] for c in field.contexts} for _ in range(2)]
        def call(index,op,end,**args):
            self.assertEqual(op,'history');self.assertEqual(args['limit'],16);field.calls.append((index,args['context']))
            return dict(context=args['context'],finished=True,items=copy.deepcopy(field.rows[index][args['context']]))
        field.pair=[SimpleNamespace(call=lambda op,end,i=i,**kw:call(i,op,end,**kw)) for i in range(2)]
        return field

    def test_initial_actual_inherited_collection_accepts_two_empty_contexts(self):
        field=self.initial_fixture('initial-valid')
        self.assertIs(type(field).collect_histories,M['D']['Field'].collect_histories)
        field.initial_histories(100)
        snapshot=json.loads((field.evidence/'initial-history.json').read_bytes())
        self.assertEqual(len(field.calls),4);self.assertEqual({s['host'] for s in snapshot},{'A','B'})
        self.assertTrue(all(len(s['contexts'])==2 and all(c['rows']==[] for c in s['contexts']) for s in snapshot))
        # The old call remains invalid: its final-delivery invariant is unchanged.
        with self.assertRaisesRegex(RuntimeError,'final history contexts differ from deliveries'):
            M['D']['prior']['verify_final_history'](snapshot,[])

    def test_initial_validator_rejects_missing_extra_wrong_and_malformed_scope(self):
        field=self.initial_fixture('initial-scope');snapshot=field.collect_histories(100)
        variants=[]
        for mutation in ['missing_host','extra_host','duplicate_host','missing_context','extra_context','wrong_context','duplicate_context','missing_rows','malformed_rows','nonempty']:
            bad=copy.deepcopy(snapshot)
            if mutation=='missing_host':bad.pop()
            elif mutation=='extra_host':bad.append(copy.deepcopy(bad[0]))
            elif mutation=='duplicate_host':bad[1]['host']='A'
            elif mutation=='missing_context':bad[1]['contexts'].pop()
            elif mutation=='extra_context':bad[1]['contexts'].append(dict(context='c'*64,rows=[]))
            elif mutation=='wrong_context':bad[1]['contexts'][0]['context']='c'*64
            elif mutation=='duplicate_context':bad[1]['contexts'][1]['context']=bad[1]['contexts'][0]['context']
            elif mutation=='missing_rows':bad[1]['contexts'][0].pop('rows')
            elif mutation=='malformed_rows':bad[1]['contexts'][0]['rows']={}
            else:bad[1]['contexts'][0]['rows']=[dict(text='unexpected')]
            with self.subTest(mutation=mutation),self.assertRaises(RuntimeError):M['verify_initial_history'](bad,field.contexts,[],['A','B'])
            variants.append(mutation)
        with self.assertRaisesRegex(RuntimeError,'no prior sends'):M['verify_initial_history'](snapshot,field.contexts,[dict()],['A','B'])
        for contexts in [[],field.contexts[:1],field.contexts+['c'*64],['a'*64,'a'*64],['wrong','b'*64]]:
            with self.assertRaises(RuntimeError):M['verify_initial_history'](snapshot,contexts,[],['A','B'])
        save(E/'initial-validator-proof.json',dict(cases=variants,prior_sends_rejected=True,installed_contexts_validated=True))

    def test_initial_actual_collection_rejects_nonempty_unfinished_wrong_or_failed_io(self):
        for scenario in ['nonempty','prior_send','wrong_context','unfinished','missing_items','malformed_rows','call_failure','missing_installed','extra_installed','duplicate_installed']:
            field=self.initial_fixture('initial-io-'+scenario)
            if scenario=='nonempty':field.rows[1][field.contexts[1]]=[dict(text='unexpected')]
            elif scenario=='prior_send':field.messages=[dict()]
            elif scenario=='missing_installed':field.contexts=field.contexts[:1]
            elif scenario=='extra_installed':
                field.contexts.append('c'*64)
                for rows in field.rows:rows['c'*64]=[]
            elif scenario=='duplicate_installed':field.contexts[1]=field.contexts[0]
            else:
                original=field.pair[1].call
                def call(op,end,original=original,scenario=scenario,**args):
                    if scenario=='call_failure':raise OSError('history I/O sentinel')
                    value=original(op,end,**args)
                    if scenario=='wrong_context':value['context']='c'*64
                    elif scenario=='unfinished':value['finished']=False
                    elif scenario=='malformed_rows':value['items']={}
                    else:value.pop('items')
                    return value
                field.pair[1].call=call
            with self.subTest(scenario=scenario),self.assertRaises((RuntimeError,KeyError,OSError)):field.initial_histories(100)
            # Collection succeeded in these cases: retain the invalid snapshot before rejection.
            self.assertEqual((field.evidence/'initial-history.json').exists(),scenario in ['nonempty','prior_send','malformed_rows','missing_installed','extra_installed','duplicate_installed'])

    def test_initial_refactor_keeps_actual_final_history_strict(self):
        field=self.initial_fixture('initial-final-strict')
        field.messages=[dict(key=dict(context=c),event_id=str(slot+1)*64,text='text-'+str(slot)) for slot,c in enumerate(field.contexts)]
        valid={m['key']['context']:[dict(event_id=m['event_id'],text=m['text'])] for m in field.messages}
        field.rows=[copy.deepcopy(valid),copy.deepcopy(valid)];field.histories('valid',100)
        self.assertTrue((field.evidence/'valid-history.json').exists())
        for mutation in ['missing','extra','duplicate','changed_event','changed_text','peer_row_difference','context_scope']:
            field.rows=[copy.deepcopy(valid),copy.deepcopy(valid)];field.contexts=['a'*64,'b'*64]
            c=field.contexts[0]
            if mutation=='missing':field.rows[1][c]=[]
            elif mutation=='extra':field.rows[1][c].append(dict(event_id='f'*64,text='extra'))
            elif mutation=='duplicate':field.rows[1][c]*=2
            elif mutation=='changed_event':field.rows[1][c][0]['event_id']='f'*64
            elif mutation=='changed_text':field.rows[1][c][0]['text']='wrong'
            elif mutation=='peer_row_difference':field.rows[1][c][0]['ordinal']=2
            else:field.contexts=field.contexts[:1]
            with self.subTest(mutation=mutation),self.assertRaises(RuntimeError):field.histories(mutation,100)

    def test_pending_requires_durable_materialized_identity(self):
        event='a'*64;row=dict(text='offline',pending_event=None)
        history=[dict(text='offline',event_id=event)]
        self.assertFalse(M['materialized_pending'](row,dict(state='queued',event_id=None),[]))
        self.assertTrue(M['materialized_pending'](row,dict(state='materialized',event_id=event),history))
        self.assertEqual(row['pending_event'],event)
        for status,rows in [(dict(state='delivered',event_id=event),history),(dict(state='materialized',event_id='b'*64),history),(dict(state='materialized',event_id=event),history*2)]:
            with self.assertRaises(RuntimeError):M['materialized_pending'](row,status,rows)

    def notification_fixture(self):
        terms=[SimpleNamespace(instance='a',epoch=2,events=[]),SimpleNamespace(instance='b',epoch=2,events=[])]
        rows=[]
        for sender,slot in [(0,0),(0,1),(1,0),(1,1)]:
            event=str(len(rows)+1)*64;receiver=terms[1-sender]
            row=dict(sender=sender,slot=slot,event_id=event,pending_event=event if sender==0 else None,text=f'message-{sender}-{slot}',key=dict(context=str(slot)),receiver_instance=receiver.instance,receiver_epoch=2)
            rows.append(row);receiver.events.append(dict(event_id=event,context=str(slot),text=row['text'],runtime_instance=receiver.instance,epoch=2))
        return rows,[(0,terms[0]),(1,terms[1])]

    def test_notification_pre_anchor_delivery_has_explicit_unproven_coverage(self):
        rows,processes=self.notification_fixture();processes[1][1].events=[]
        present={r['event_id'] for r in rows[:2]}
        verdict=M['notification_verdict'](rows,processes,present)
        self.assertTrue(verdict['required_passed']);self.assertFalse(verdict['coverage_complete'])
        self.assertEqual((verdict['required'],verdict['observed'],verdict['not_covered']),(2,2,2))
        self.assertEqual([v['coverage'] for v in verdict['per_message']],['unproven_at_anchor','unproven_at_anchor','confirmed','confirmed'])

    def test_notification_post_watching_missing_and_both_online_replies_fail(self):
        rows,processes=self.notification_fixture();processes[1][1].events=[]
        with self.assertRaisesRegex(RuntimeError,'required live notification missing'):M['notification_verdict'](rows,processes,set())
        for removed in [0,1]:
            rows,processes=self.notification_fixture();processes[1][1].events=[];processes[0][1].events.pop(removed)
            with self.assertRaisesRegex(RuntimeError,'required live notification missing'):
                M['notification_verdict'](rows,processes,{r['event_id'] for r in rows}) # Reply can never become optional.

    def test_notification_optional_observed_duplicate_misbound_unexpected_fail(self):
        for wrong in ['duplicate','text','context','instance','epoch','unknown','wrong_mailbox']:
            rows,processes=self.notification_fixture();term=processes[1][1];present={r['event_id'] for r in rows[:2]}
            if wrong=='duplicate':term.events.append(copy.deepcopy(term.events[0]))
            elif wrong=='unknown':term.events.append(dict(event_id='f'*64))
            elif wrong=='wrong_mailbox':processes[0][1].events.append(term.events.pop(0))
            else:term.events[0][{'instance':'runtime_instance'}.get(wrong,wrong)]=0 if wrong=='epoch' else 'wrong'
            with self.subTest(wrong=wrong),self.assertRaises(RuntimeError):M['notification_verdict'](rows,processes,present)

    def test_notification_actual_optional_wait_does_not_mask_other_errors(self):
        rows,processes=self.notification_fixture();row=rows[0];field=object.__new__(M['OfflineRecipient'])
        field.pair=[SimpleNamespace(call=lambda *a,**k:dict(state='delivered',event_id=row['event_id'])),processes[1][1]]
        with patch.dict(M['D'],until=lambda check,*a:check(100)),patch.dict(M['D']['prior'],observe_incoming=lambda *a:dict(status='not_observed',events=[])):
            field.deliver(row,100,required_notification=False)
            self.assertEqual(row['delivery']['state'],'delivered');self.assertEqual(row['notification']['status'],'not_observed')
            with self.assertRaisesRegex(RuntimeError,'required live notification missing'):field.deliver(row,100,required_notification=True)
        for status in ['duplicate','binding_mismatch','unexpected_status']:
            with patch.dict(M['D'],until=lambda check,*a:check(100)),patch.dict(M['D']['prior'],observe_incoming=lambda *a:dict(status=status)):
                with self.assertRaises(RuntimeError):field.deliver(row,100,required_notification=False)
        field.pair[0].call=lambda *a,**k:(_ for _ in ()).throw(RuntimeError('request status failure'))
        with patch.dict(M['D'],until=lambda check,*a:check(100)):
            with self.assertRaisesRegex(RuntimeError,'request status failure'):field.deliver(row,100,required_notification=False)
        with patch.dict(M['D'],until=lambda *a:(_ for _ in ()).throw(OSError('transport sentinel'))):
            with self.assertRaisesRegex(OSError,'transport sentinel'):field.deliver(row,100,required_notification=False)
        field.pair[0].call=lambda *a,**k:dict(state='delivered',event_id='f'*64)
        with patch.dict(M['D'],until=lambda check,*a:check(100)):
            with self.assertRaisesRegex(RuntimeError,'different event'):field.deliver(row,100,required_notification=False)

    def test_notification_reconciled_after_cleanup_including_late_duplicates(self):
        for duplicate in [False,True]:
            rows,processes=self.notification_fixture();field=object.__new__(M['OfflineRecipient']);field.messages=rows;field.processes=processes;field.anchor_present={r['event_id'] for r in rows[:2]}
            field.delivery_history_passed=True;field.notification_coverage=dict(evaluated=False);field.recipient_restarts=1;field.evidence=E/('late-duplicate' if duplicate else 'late-valid');field.evidence.mkdir()
            rows[0]['notification']=dict(status='not_observed') # Bounded wait can precede a final drain event.
            def cleanup(self):
                if duplicate:processes[1][1].events.append(copy.deepcopy(processes[1][1].events[0]))
            with patch.object(M['M']['Diagnostic'],'run',cleanup):
                if duplicate:
                    with self.assertRaisesRegex(RuntimeError,'duplicate notification'):field.run()
                else:field.run()
            saved=json.loads((field.evidence/'offline-recipient-result.json').read_bytes())
            self.assertEqual(saved['passed'],not duplicate);self.assertTrue(saved['delivery_history']['passed'])
            observed=json.loads((field.evidence/'notifications-observed.json').read_bytes())
            self.assertEqual(sum(len(p['events']) for p in observed['processes']),5 if duplicate else 4)
            self.assertEqual(saved['notification_coverage']['observed'],5 if duplicate else 4)
            if not duplicate:self.assertTrue(saved['notification_coverage']['coverage_complete'])

    def test_per_incarnation_capture_does_not_use_newest_global_pid(self):
        field=object.__new__(M['OfflineRecipient']);field.evidence=E/'capture';field.end=time.monotonic()+60;field.capture_errors=[];field.generations=['g','g']
        calls=[]
        def call(op,**args):calls.append(args);return dict(scope=args)
        field.hosts=[None,SimpleNamespace(name='B',call=call)]
        field.chat_pids=[999,999];field.latest=[None,None]
        field.incarnations={(1,'old'):dict(chat_pid=10,snapshot=dict(doc=dict(runtime_pid=20,runtime_instance='a'*64)),collected=False),
                            (1,'new'):dict(chat_pid=11,snapshot=dict(doc=dict(runtime_pid=21,runtime_instance='b'*64)),collected=False)}
        def decode(reply,nonce,chat,pid,instance):
            self.assertEqual((chat,pid,instance),(reply['scope']['diagnostic_chat_pid'],reply['scope']['diagnostic_runtime_pid'],reply['scope']['diagnostic_runtime_instance']))
            return {f'worker-final-{chat}.json':b'{}'},dict(complete=True)
        with patch.dict(M['M'],shutdown_records=decode):
            for tag,instance in [('old','a'*64),('new','b'*64)]:field.collect_shutdown(1,SimpleNamespace(mode='chat',tag=tag,instance=instance))
        self.assertFalse(field.capture_errors);self.assertEqual([c['diagnostic_chat_pid'] for c in calls],[10,11])
        self.assertEqual(calls[0]['diagnostic_previous'],[]);self.assertEqual(calls[1]['diagnostic_previous'][0]['diagnostic_runtime_pid'],20)
        self.assertTrue((field.evidence/'shutdown/B/old/worker-final-10.json').exists());self.assertTrue((field.evidence/'shutdown/B/new/worker-final-11.json').exists())

    def model(self,label):
        outer=self
        class Model(M['OfflineRecipient']):
            def __init__(self):
                self.evidence=E/label;self.evidence.mkdir();self.end=time.monotonic()+60;self.contexts=['a'*64,'b'*64];self.messages=[];self.offline_rows=[];self.processes=[];self.capture_errors=[];self.log=[];self.recipient_restarts=0;self.delivery_history_passed=False
                self.hosts=[SimpleNamespace(name=s) for s in ['A','B']];self.stored=[{c:[] for c in self.contexts} for _ in range(2)]
                self.pair=[self.term('a',True),self.term('b',True)]
            def term(self,tag,live):
                index=int(tag!='a')
                def call(op,end,**args):
                    outer.assertEqual(op,'history');self.log.append(('history_io',index,args['context']))
                    return dict(context=args['context'],finished=True,items=copy.deepcopy(self.stored[index][args['context']]))
                t=SimpleNamespace(tag=tag,instance=tag,epoch=2,events=[],live=live,call=call);self.processes.append((index,t));return t
            def monitors(self,label,*a,**k):self.log.append(('watching',label))
            def sample(self,*a):return dict(slots=[dict(own_locator_revision=1)]*2),dict(port=50001)
            def stop(self,i,t):
                t.live=False;self.log.append(('stop',i));save(self.evidence/f'B-{t.tag}-job.json',dict(child_exit=0,mailbox_quit=True,mailbox_eof=True,forced=False,after=[],descriptor=False))
            def port_gone(self,*a):self.log.append(('socket_gone',))
            def start(self,i,tag,mode):
                self.log.append(('restart',i))
                if getattr(self,'pre_anchor',False):self.stored[1]=copy.deepcopy(self.stored[0])
                return self.term(tag,True)
            def connect(self,i,warm):self.log.append(('connect',i,warm))
            def send(self,sender,slot,label,end):
                if sender==0:outer.assertFalse(self.pair[1].live)
                row=dict(sender=sender,slot=slot,text=label,key=dict(context=self.contexts[slot]),pending_event=None);self.messages.append(row);self.log.append(('send',sender,slot));return row
            def pending_persisted(self,row):
                row['pending_event']=str(row['slot']+1)*64;self.log.append(('persisted',row['slot']))
                rows=self.stored[0][row['key']['context']]
                if not rows:rows.append(dict(event_id=row['pending_event'],text=row['text']))
                return dict(materialized=True)
            def probe(self,*a):return []
            def deliver(self,row,*a,required_notification=True):
                receiver=self.pair[1-row['sender']];row['event_id']=row['pending_event'] or str(row['slot']+3)*64
                row.update(receiver_instance=receiver.instance,receiver_epoch=receiver.epoch,notification=dict(status='confirmed'))
                for side in [0,1]:
                    rows=self.stored[side][row['key']['context']]
                    if not any(r['event_id']==row['event_id'] for r in rows):rows.append(dict(event_id=row['event_id'],text=row['text']))
                if not required_notification and getattr(self,'omit_optional',False):
                    row['notification']=dict(status='not_observed');self.log.append(('delivered',row['sender'],row['slot']));return
                receiver.events.append(dict(event_id=row['event_id'],context=row['key']['context'],text=row['text'],runtime_instance=receiver.instance,epoch=receiver.epoch));self.log.append(('delivered',row['sender'],row['slot']))
        return Model()

    def test_notification_full_scope_delivery_can_pass_with_unproven_queued_coverage(self):
        field=self.model('pre-anchor-scope');field.omit_optional=True;field.pre_anchor=True
        gate=dict(admitted=True,last_complete_runtime=dict(B=dict(doc=dict(slots=[dict(own_locator_revision=2)]*2))))
        with patch.object(M['M']['Diagnostic'],'functional',lambda self:None),patch.dict(M['M'],admission_gate=lambda *a,**k:gate):field.functional()
        with patch.object(M['M']['Diagnostic'],'run',lambda self:None):field.run()
        saved=json.loads((field.evidence/'offline-recipient-result.json').read_bytes())
        self.assertTrue(saved['passed']);self.assertTrue(saved['delivery_history']['passed']);self.assertEqual(len(saved['messages']),4)
        self.assertEqual((saved['notification_coverage']['required'],saved['notification_coverage']['observed'],saved['notification_coverage']['not_covered']),(2,2,2))
        self.assertFalse(saved['notification_coverage']['coverage_complete'])

    def test_actual_functional_order_recipient_only_and_two_pending(self):
        field=self.model('order')
        gate=dict(admitted=True,last_complete_runtime=dict(B=dict(doc=dict(slots=[dict(own_locator_revision=2)]*2))))
        with patch.object(M['M']['Diagnostic'],'functional',lambda self:self.log.append(('initial_admitted',))),patch.dict(M['M'],admission_gate=lambda *a,**k:gate):field.functional()
        initial=json.loads((field.evidence/'initial-history.json').read_bytes())
        self.assertEqual([len(c['rows']) for h in initial for c in h['contexts']],[0,0,0,0])
        self.assertIs(type(field).histories,M['D']['Field'].histories)
        self.assertEqual([x for x in field.log if x[0]=='send'],[('send',0,0),('send',0,1),('send',1,0),('send',1,1)])
        self.assertEqual([x for x in field.log if x[0]=='restart'],[('restart',1)])
        self.assertLess(field.log.index(('socket_gone',)),field.log.index(('send',0,0)))
        self.assertLess(max(i for i,x in enumerate(field.log) if x[0]=='persisted'),field.log.index(('restart',1)))
        self.assertEqual(field.pair[0].instance,'a');self.assertEqual(len(field.messages),4)
        self.assertEqual(field.recipient_restarts,1)
        self.assertTrue(field.delivery_history_passed)
        verdict=M['notification_verdict'](field.messages,field.processes,field.anchor_present)
        self.assertEqual((verdict['required'],verdict['observed'],verdict['not_covered']),(4,4,0))
        save(E/'order-proof.json',dict(synthetic=True,sequence=field.log))

    def test_initial_failure_never_stops_or_sends(self):
        field=self.model('initial-failure')
        with patch.object(M['M']['Diagnostic'],'functional',side_effect=RuntimeError('initial gate')):
            with self.assertRaisesRegex(RuntimeError,'initial gate'):field.functional()
        self.assertEqual(field.log,[])
        self.assertEqual(field.recipient_restarts,0)

    def test_initial_full_functional_rejects_invalid_history_before_offline_actions(self):
        for scenario in ['nonempty','unfinished','call_failure']:
            field=self.model('initial-order-'+scenario)
            if scenario=='nonempty':field.stored[1][field.contexts[0]]=[dict(event_id='f'*64,text='prior')]
            elif scenario=='unfinished':field.pair[1].call=lambda op,end,**args:dict(context=args['context'],finished=False,items=[])
            else:field.pair[1].call=lambda *a,**k:(_ for _ in ()).throw(OSError('history I/O sentinel'))
            with patch.object(M['M']['Diagnostic'],'functional',lambda self:self.log.append(('initial_admitted',))):
                with self.assertRaises((RuntimeError,OSError)):field.functional()
            self.assertFalse(any(entry[0] in ['send','stop','restart','persisted'] for entry in field.log))
            self.assertEqual(field.messages,[]);self.assertEqual(field.recipient_restarts,0)

    def test_actual_ps_reader_two_incarnations_and_fail_closed(self):
        kinds=['runtime_stopping','runtime_network_stopped','runtime_final','connection_final','worker_final',
               *('startup_'+x for x in ['setup_open','begin','import_begin','import_end','profile_begin','prepared','spawned','descriptor','status'])]
        outcomes=[]
        for scenario in ['valid','prior_nonce','prior_instance','unknown_pid','without_previous','too_many_previous','pid_collision','count_cap']:
            folder=E/'reader'/scenario;run=folder/'runs/one';control=folder/'ordinary-agent/control';diag=run/'setup/lab-diagnostics-v1';diag.mkdir(parents=True);control.mkdir(parents=True)
            nonce='a'*32;session='b'*32
            save(run/'attempt.json',dict(attempt='one',generation=nonce,session=session,pins={}))
            save(run/'started.json',dict(generation=nonce,pid=123,started=134395772085441302));save(control/'ready.json',dict(session=session));save(control/'state.json',dict(session=session,active=None,blocked=None));save(diag/'enabled.json',dict(schema=1,run_nonce=nonce))
            for chat,runtime,instance in [(44,42,'c'*64),(54,52,'d'*64)]:
                for kind in kinds:
                    pid=runtime if kind.startswith('runtime_') else chat
                    row=dict(schema=1,kind=kind,run_nonce=nonce,pid=pid,runtime_pid=runtime,runtime_instance=instance,data=dict(ok=True))
                    if kind.startswith('startup_') and kind!='startup_status':row.update(runtime_pid=None,runtime_instance=None)
                    if chat==44 and kind=='runtime_final':
                        if scenario=='prior_nonce':row['run_nonce']='f'*32
                        if scenario=='prior_instance':row['runtime_instance']='f'*64
                    save(diag/(kind.replace('_','-')+f'-{pid}.json'),row)
            prior=dict(diagnostic_chat_pid=44,diagnostic_runtime_pid=42,diagnostic_runtime_instance='c'*64)
            request=dict(op='lab-diagnostic-read',base=str(folder),attempt='one',agent_session=session,agent_generation=nonce,worker_pid=123,worker_started=134395772085441302,pins={},diagnostic_chat_pid=54,diagnostic_runtime_pid=52,diagnostic_runtime_instance='d'*64,diagnostic_previous=[prior])
            if scenario=='without_previous':request.pop('diagnostic_previous')
            if scenario=='too_many_previous':request['diagnostic_previous']=[prior,prior]
            if scenario=='pid_collision':prior['diagnostic_runtime_pid']=52
            if scenario in ['unknown_pid','count_cap']:
                for number in range(1 if scenario=='unknown_pid' else 5):save(diag/f'worker-final-{90+number}.json',dict(schema=1,kind='worker_final',run_nonce=nonce,pid=90+number,runtime_pid=52,runtime_instance='d'*64,data=dict(ok=True)))
            save(folder/'request.json',request)
            host=str(ROOT/'scripts/test-terminal-t1-1f-host.ps1').replace("'","''");req=str(folder/'request.json').replace("'","''")
            script=folder/'read.ps1';script.write_text('Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"; '+f"& ([scriptblock]::Create([IO.File]::ReadAllText('{host}'))) -Request ([IO.File]::ReadAllText('{req}'))",encoding='utf-8')
            out=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(script)],capture_output=True,timeout=25,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
            (folder/'stdout.bin').write_bytes(out.stdout);(folder/'stderr.bin').write_bytes(out.stderr)
            if scenario in ['without_previous','too_many_previous','pid_collision','count_cap']:self.assertNotEqual(out.returncode,0)
            else:
                self.assertEqual(out.returncode,0,out.stderr);reply=json.loads(out.stdout)
                self.assertEqual(len(reply['records']),14);self.assertEqual(reply['prior_validated'],14 if scenario in ['valid','unknown_pid'] else 13)
                self.assertEqual(len(reply['rejected']),0 if scenario=='valid' else 1)
                self.assertTrue(all(json.loads(base64.b64decode(r['data']))['runtime_instance'] in (None,'d'*64) for r in reply['records']))
            outcomes.append(dict(scenario=scenario,exit=out.returncode))
        save(E/'reader-proof.json',dict(actual_reader=True,synthetic_files=True,cases=outcomes))

if __name__=='__main__':
    if NOTIFICATION_ONLY or INITIAL_ONLY:
        names=[n for n in unittest.defaultTestLoader.getTestCaseNames(Checks) if
            (INITIAL_ONLY and n.startswith('test_initial_')) or
            (NOTIFICATION_ONLY and n.startswith('test_notification_')) or
            n in ['test_actual_functional_order_recipient_only_and_two_pending','test_notification_full_scope_delivery_can_pass_with_unproven_queued_coverage']]
        result=unittest.TextTestRunner(verbosity=2).run(unittest.TestSuite(Checks(n) for n in names));sys.exit(not result.wasSuccessful())
    else:unittest.main()
