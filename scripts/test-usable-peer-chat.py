"""Bounded disposable public-product exercise. Same-driver causal timings only.

Run inside the existing native Windows Job guard, never against personal profiles.
The local lane is explicitly NOT a physical LAN/cross-network result.
"""
import argparse, base64, hashlib, json, math, os, subprocess, threading, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

class Actor:
    def __init__(self, exe, home, evidence, label, deadline, join=None, hold_accept=False):
        self.hold_accept=hold_accept
        self.label, self.deadline, self.events = label, deadline, []
        self.condition, self.input_lock = threading.Condition(), threading.Lock()
        argv = [str(exe), '--profile', str(home), '--jsonl']
        if join: argv += ['--join', join]
        self.log = (evidence / (label + '.jsonl')).open('x', encoding='utf-8')
        self.err = (evidence / (label + '.stderr.bin')).open('xb')
        self.process = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.err,
            creationflags=subprocess.CREATE_NO_WINDOW | subprocess.BELOW_NORMAL_PRIORITY_CLASS)
        self.reader = threading.Thread(target=self.read, daemon=True); self.reader.start()
    def write(self, line):
        with self.input_lock:
            self.process.stdin.write(line.encode('utf-8') + b'\n'); self.process.stdin.flush()
    def read(self):
        try:
            for raw in self.process.stdout:
                when = time.perf_counter()
                if len(raw) > 1024 * 1024: raise RuntimeError('product frame bound')
                event = json.loads(raw)
                saved = event
                if event['type'] in ('recovery', 'invitation'):
                    saved = dict(type=event['type'], redacted=True)
                self.log.write(json.dumps(dict(tick=when, event=saved), ensure_ascii=False) + '\n'); self.log.flush()
                with self.condition: self.events.append((when,event)); self.condition.notify_all()
                if event['type'] == 'welcome': self.write('')
                if event['type'] == 'confirm':
                    command = event['value']['command']
                    if command not in ('/accept','/saved'): raise RuntimeError('unexpected product consent')
                    if command!='/accept' or not self.hold_accept:
                        self.write(command)  # Explicit input on disposable accounts, not a production bypass.
        except Exception as error:
            with self.condition: self.events.append((time.perf_counter(),dict(type='driver_error',text=str(error))));self.condition.notify_all()
        finally:
            with self.condition: self.events.append((time.perf_counter(),dict(type='eof')));self.condition.notify_all()
    def until(self, predicate, seconds=120, since=0):
        end = min(self.deadline, time.perf_counter()+seconds)
        with self.condition:
            index=since
            while True:
                for when,event in self.events[index:]:
                    if predicate(event): return when,event
                    if event['type']=='operation_error': raise RuntimeError(self.label+' rejected operation: '+event['text'])
                    if event['type'] in ('eof','driver_error'): raise RuntimeError(self.label+' product stopped before expected observation: '+str(event))
                index=len(self.events)
                left=end-time.perf_counter()
                if left<=0:
                    tail=[x[1] for x in self.events[-8:] if x[1]['type'] not in ('recovery','invitation')]
                    raise TimeoutError(self.label+' observation deadline; tail='+json.dumps(tail,ensure_ascii=False))
                self.condition.wait(min(left,.2))
    def mark(self):
        with self.condition:return len(self.events)
    def ready(self):return self.until(lambda e:e['type']=='ready',240)
    def connected(self):return self.until(lambda e:e['type']=='connection' and e['value'].get('status',{}).get('live_admitted') is True,120)
    def send(self, text):
        mark=self.mark();start=time.perf_counter();self.write(text)
        when,event=self.until(lambda e:e['type']=='result',40,mark)
        if event['value'].get('state') not in ('queued','materialized','delivered'):
            raise RuntimeError('send not acknowledged durable: '+str(event))
        return start,when,event
    def history(self):
        rows=[];more=False
        for _ in range(100):
            mark=self.mark();self.write('/history more' if more else '/history')
            self.until(lambda e:e['type']=='history_page',30,mark)
            events=[e for _,e in self.events[mark:]]
            rows.extend(e['value'] for e in events if e['type']=='history')
            page=next(e for e in events if e['type']=='history_page')
            if not page['value']['has_more']:return rows
            more=True
        raise RuntimeError('history exceeded bounded fixture pages')
    def close(self):
        if self.process.poll() is None and not any(e['type']=='eof' for _,e in self.events):self.write('/quit')
        code=self.process.wait(timeout=min(60,max(1,self.deadline-time.perf_counter())))
        self.reader.join(timeout=2);self.log.close();self.err.close()
        if self.reader.is_alive():raise RuntimeError(self.label+' product stdout EOF not confirmed')
        if code:raise RuntimeError(self.label+' nonzero ordinary close '+str(code))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--exe',required=True,type=Path);p.add_argument('--evidence',required=True,type=Path)
    p.add_argument('--samples',type=int,default=10);p.add_argument('--seconds',type=int,default=840);p.add_argument('--recovery',action='store_true');p.add_argument('--checkpoint-refusal',action='store_true');p.add_argument('--require-latency',action='store_true');p.add_argument('--legacy-migration',action='store_true');p.add_argument('--confirmation-regression',action='store_true')
    a=p.parse_args();assert 1<=a.samples<=40 and 60<=a.seconds<=840
    out=a.evidence.resolve();out.mkdir(parents=True,exist_ok=False)
    assert ROOT in out.parents and '.tmp' in out.parts
    exe=a.exe.resolve();assert exe.name=='kilogram-peer.exe'
    start=time.perf_counter();end=start+a.seconds;actors=[];samples=[];expected=[];failure=None;forced=[];closed=[]
    def save(name,value):(out/name).write_text(json.dumps(value,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    try:
        A=Actor(exe,out/'A/profile',out,'A',end);actors.append(A)
        _,invitation=A.until(lambda e:e['type']=='invitation',180)
        B=Actor(exe,out/'B/profile',out,'B',end,invitation['value']['invitation']);actors.append(B)
        A.ready();B.ready();A.connected();B.connected();connected=time.perf_counter()
        save('connection.json',dict(seconds=connected-start,local_two_processes=True,physical_LAN=False,physical_cross_network=False))
        print('public pairing/consent/connect complete',flush=True)
        duplicate=subprocess.run([str(exe),'--profile',str(out/'A/profile'),'--jsonl'],input=b'/quit\n',capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
        assert duplicate.returncode!=0 and not duplicate.stdout and b'Close the existing account window first' in duplicate.stderr
        save('locked-profile.json',dict(passed=True,exit=duplicate.returncode,stdout_empty=True,refusal_before_network_key_or_account_mutation=True))
        nonce=uuid.uuid4().hex
        for direction,sender,receiver in [('A-B',A,B),('B-A',B,A)]:
            for n in range(a.samples):
                text=f'usable-{nonce}-{direction}-{n}';mark=receiver.mark()
                sent,acked,_=sender.send(text)
                visible,event=receiver.until(lambda e:e['type']=='message' and e['value'].get('text')==text,15,mark)
                samples.append(dict(direction=direction,n=n,send_ack_ms=1000*(acked-sent),send_to_visible_ms=1000*(visible-sent),event_id=event['value']['event_id']))
                expected.append(text)
            print(direction,'messages visible',a.samples,flush=True)
        if a.recovery:
            B.close();closed.append('B');print('recipient offline',flush=True)
            queued=[f'usable-{nonce}-offline-{n}' for n in range(2)]
            for text in queued:A.send(text)
            before=A.history();pending={r['text']:r['event_id'] for r in before if r['text'] in queued};assert len(pending)==2
            A.close();closed.append('A');print('sender saved pending and stopped',flush=True)
            A=Actor(exe,out/'A/profile',out,'A-restarted',end);actors.append(A);A.ready()
            after=A.history();assert {r['text']:r['event_id'] for r in after if r['text'] in queued}==pending
            print('same-profile sender history recovered before recipient return',flush=True)
            B=Actor(exe,out/'B/profile',out,'B-restarted',end);actors.append(B);B.ready();A.connected();B.connected()
            # Delivery is independently checked by exact history. Visibility is a
            # separate assertion, including messages that raced the initial anchor.
            for text in queued:B.until(lambda e:e['type']=='message' and e['value'].get('text')==text,45)
            expected+=queued;save('offline-recovery.json',dict(passed=True,original_event_ids=pending,recipient_offline=True,sender_restarted_before_B_return=True))
        left=A.history();right=B.history()
        assert len(left)==len(right)==len(expected) and left==right
        assert set(r['text'] for r in left)==set(expected) and len(set(r['event_id'] for r in left))==len(expected)
        save('history.json',dict(passed=True,A=left,B=right))
        visible=[e['value']['text'] for actor in actors for _,e in actor.events if e['type']=='message']
        assert sorted(visible)==sorted(expected), 'ordinary UI skipped or duplicated an incoming row across restart'
        save('notifications.json',dict(passed=True,exact_once_during_ordinary_run=True,texts=visible))
        if a.checkpoint_refusal:
            B.close();closed.append('B-before-checkpoint-refusal')
            context=next(e['value']['contexts'][0] for _,e in B.events if e['type']=='ready')
            checkpoint=out/'B/profile'/('peer-history-'+context+'.cursor')
            original=checkpoint.read_bytes();raw=bytearray(base64.urlsafe_b64decode(original+b'='));raw[-1]^=1
            checkpoint.write_bytes(base64.urlsafe_b64encode(raw).rstrip(b'='))
            B=Actor(exe,out/'B/profile',out,'B-checkpoint-refusal',end);actors.append(B);B.ready()
            B.until(lambda e:e['type']=='history_gap' and e['value'].get('code')=='history_cursor_invalid',20)
            mark=B.mark();text='usable-'+nonce+'-native-MAC-refusal';A.send(text)
            try:B.until(lambda e:e['type']=='message' and e['value'].get('text')==text,1,mark)
            except TimeoutError:pass
            else:raise RuntimeError('silently replaced invalid checkpoint with an anchor')
            B.write('/resync');B.until(lambda e:e['type']=='message' and e['value'].get('text')==text,20,mark)
            expected.append(text);left=A.history();right=B.history()
            assert left==right and len(left)==len(expected) and set(r['text'] for r in left)==set(expected)
            save('checkpoint-refusal.json',dict(passed=True,actual_native_MAC_refusal=True,no_silent_fallback=True,explicit_authenticated_replay=True,history_preserved=True))
            save('history-after-checkpoint.json',dict(passed=True,A=left,B=right))
        A.close();closed.append('A-final');B.close();closed.append('B-final')
        if a.legacy_migration:
            original_rows={r['event_id']:r for r in left};public_before={}
            for side in ('A','B'):
                home=out/side/'profile';assert out in home.parents
                public_before[side]=json.loads((home/'setup.json').read_bytes())['public']
                # Only the separate technical rendezvous key is archived. The
                # ORIGINAL account/device keys and history remain in place.
                (home/'peer-network.sealed').rename(out/(side+'-technical-network-before-migration.sealed'))
            B=Actor(exe,out/'B/profile',out,'legacy-B-inviter',end);actors.append(B)
            _,invitation=B.until(lambda e:e['type']=='invitation',60)
            if a.confirmation_regression:
                home=out/'A/profile';before=hashlib.sha256((home/'setup.json').read_bytes()).hexdigest()
                cancelled=Actor(exe,home,out,'confirmation-EOF',end,invitation['value']['invitation'],hold_accept=True);actors.append(cancelled)
                cancelled.until(lambda e:e['type']=='confirm' and e['value']['command']=='/accept',60)
                cancelled.process.stdin.close();assert cancelled.process.wait(timeout=15)==0
                assert hashlib.sha256((home/'setup.json').read_bytes()).hexdigest()==before and not (home/'peer-network.sealed').exists()
            A=Actor(exe,out/'A/profile',out,'legacy-A-joiner',end,invitation['value']['invitation'],hold_accept=a.confirmation_regression);actors.append(A)
            if a.confirmation_regression:
                import queue
                A.until(lambda e:e['type']=='confirm' and e['value']['command']=='/accept',60);held=time.perf_counter()
                duplicate=subprocess.run([str(exe),'--profile',str(home),'--jsonl'],input=b'/quit\n',capture_output=True,timeout=10,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
                assert duplicate.returncode!=0 and not duplicate.stdout and b'Close the existing account window first' in duplicate.stderr
                legacy=subprocess.Popen([str(exe),'chat','--setup',str(home),'--jsonl'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(out/'legacy-lock-refusal.stderr.bin').open('xb'),creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
                replies=queue.Queue()
                def read_legacy():
                    for line in legacy.stdout:replies.put(json.loads(line))
                threading.Thread(target=read_legacy,daemon=True).start()
                assert replies.get(timeout=10)['type']=='ready'
                legacy.stdin.write(b'{"v":1,"id":"locked","op":"connect","args":{"mode":"start"}}\n');legacy.stdin.flush()
                while True:
                    reply=replies.get(timeout=20)
                    if reply.get('id')=='locked':break
                assert reply['ok'] is False and reply.get('code')=='connect_failed',reply
                legacy.stdin.write(b'{"v":1,"id":"quit","op":"quit","args":{}}\n');legacy.stdin.flush();legacy.stdin.close()
                assert legacy.wait(timeout=35)==0
                print('holding real existing-account confirmation >120s; both competing openers refused',flush=True)
                while time.perf_counter()-held<125:
                    assert A.process.poll() is None
                    time.sleep(.1)
                assert hashlib.sha256((home/'setup.json').read_bytes()).hexdigest()==before and not (home/'peer-network.sealed').exists()
                assert sum(e['type']=='confirm' and e['value']['command']=='/accept' for _,e in A.events)==1
                A.hold_accept=False;A.write('/accept')
                save('confirmation-ownership.json',dict(passed=True,held_seconds=time.perf_counter()-held,frontend_duplicate_refused=True,legacy_owner_refused=reply,eof_cancel_no_mutation=True,no_repeated_prompt=True,no_preapproval_mutation=True))
            A.ready();B.ready();A.connected();B.connected()
            for side,sender,receiver in [('A',A,B),('B',B,A)]:
                text='usable-'+nonce+'-legacy-'+side;mark=receiver.mark();sender.send(text)
                receiver.until(lambda e:e['type']=='message' and e['value'].get('text')==text,20,mark);expected.append(text)
            left=A.history();right=B.history()
            assert left==right and len(left)==len(expected) and set(r['text'] for r in left)==set(expected)
            assert all(next(r for r in left if r['event_id']==event)==row for event,row in original_rows.items())
            for side in ('A','B'):assert json.loads((out/side/'profile/setup.json').read_bytes())['public']==public_before[side]
            assert not any(e['type']=='recovery' for actor in (A,B) for _,e in actor.events),'migration created an account'
            save('legacy-migration.json',dict(passed=True,original_accounts_reused=True,bootstrap_not_repeated=True,joined_profile_can_invite_original_peer=True,preserved_events=len(original_rows),A=left,B=right))
            A.close();closed.append('legacy-A-final');B.close();closed.append('legacy-B-final')
        locator_counts={}
        for side in ('A','B'):
            folder=out/side/'profile/connection'
            locator_counts[side]=dict(staging=len(list(folder.glob('.peer-incoming-*'))),network=len(list(folder.glob('network-*.json'))),retained=len(list(folder.glob('peer-*.json'))),exports=len(list(folder.glob('public-*.json'))))
            assert locator_counts[side]['staging']==locator_counts[side]['network']==locator_counts[side]['exports']==0 and locator_counts[side]['retained']<=1,locator_counts
        save('bounded-locator-files.json',dict(passed=True,counts=locator_counts))
        values=sorted(x['send_to_visible_ms'] for x in samples)
        save('latency.json',dict(samples=samples,n=len(samples),p50_ms=values[(len(values)-1)//2],p95_ms=values[math.ceil(.95*len(values))-1],max_ms=max(values),
            method='same driver perf_counter before input flush through peer stdout receipt; no host wall-clock subtraction',
            physical_LAN=False,physical_cross_network=False,connection_measured_separately=True,target_500ms_met=values[math.ceil(.95*len(values))-1]<500))
        if a.require_latency:assert values[math.ceil(.95*len(values))-1]<500,'local p95 target missed; this is not a physical LAN claim'
        print('product exercise passed',json.dumps(dict(n=len(values),p95_ms=values[math.ceil(.95*len(values))-1],max_ms=max(values))),flush=True)
    except Exception as error:
        failure=dict(type=type(error).__name__,message=str(error));raise
    finally:
        for actor in actors:
            if actor.process.poll() is None:
                try:actor.close()
                except Exception as error:
                    forced.append(dict(actor=actor.label,error=str(error)))
                    actor.process.kill() # Only this retained own disposable frontend handle; its product Job contains children.
                    actor.process.wait(timeout=5)
        save('samples.json',samples)
        save('result.json',dict(passed=failure is None,failure=failure,forced_frontends=forced,ordinary_closed=closed,
            exe_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),elapsed=time.perf_counter()-start,public_path=True,
            personal_profiles_used=False,local_fixture_NOT_cross_network=True))
if __name__=='__main__':main()
