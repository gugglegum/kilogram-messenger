"""One explicit disposable product probe. A causal RTT upper-bounds each leg;
no clock subtraction between hosts, IP entry or external control server.
Use local receiver only to verify the fixture, never as a physical-network claim.
"""
import argparse,hashlib,json,math,runpy,subprocess,threading,time,uuid
from pathlib import Path
Actor=runpy.run_path(str(Path(__file__).with_name('test-usable-peer-chat.py')))['Actor']

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--exe',required=True,type=Path);p.add_argument('--evidence',required=True,type=Path)
    p.add_argument('--network',choices=['local-fixture','LAN','different-networks'],required=True);p.add_argument('--local-receiver',action='store_true');p.add_argument('--samples',type=int,default=20)
    a=p.parse_args();assert 1<=a.samples<=30 and (not a.local_receiver or a.network=='local-fixture')
    out=a.evidence.resolve();assert '.tmp' in out.parts;out.mkdir(parents=True,exist_ok=False)
    exe=a.exe.resolve();end=time.perf_counter()+840;actor=None;receiver=None;failure=None;samples=[];nonce=uuid.uuid4().hex;cleanup=[];receiver_case='probe-'+nonce
    (out/'run.json').write_text(json.dumps(dict(local_receiver=a.local_receiver,receiver_case=receiver_case,network=a.network))+'\n',encoding='utf8')
    try:
        actor=Actor(exe,out/'profile',out,'probe-A',end)
        _,invite=actor.until(lambda e:e['type']=='invitation',180)
        link=invite['value']['invitation']
        if a.local_receiver:
            receiver=subprocess.Popen(['C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(Path(__file__).with_name('test-usable-peer-receiver.ps1')),'-Exe',str(exe),'-Invitation',link,'-Case',receiver_case],stdout=(out/'receiver.stdout.log').open('xb'),stderr=(out/'receiver.stderr.log').open('xb'))
            def receiver_exit():
                if receiver.wait()!=0 and actor is not None and actor.process.poll() is None:
                    try:actor.write('/quit')
                    except OSError:pass
            threading.Thread(target=receiver_exit,daemon=True).start()
        else:
            print('На ноутбуке: test-peer-receiver.cmd; вставить эту ОДНУ ссылку:',flush=True);print(link,flush=True)
        actor.ready();actor.connected();connected=time.perf_counter()
        for n in range(a.samples):
            mark=actor.mark();start,ack,_=actor.send(f'KGPROBE/{nonce}/{n}')
            finish,_=actor.until(lambda e:e['type']=='message' and e['value'].get('text')==f'KGPROBE-REPLY/{nonce}/{n}',20,mark)
            samples.append(dict(n=n,round_trip_ms=(finish-start)*1000,send_ack_ms=(ack-start)*1000))
        mark=actor.mark();actor.send('KGPROBE-STOP/'+nonce)
        actor.until(lambda e:e['type']=='message' and e['value'].get('text')=='KGPROBE-STOPPED/'+nonce,20,mark)
        actor.close();actor=None
        if receiver is not None:assert receiver.wait(timeout=60)==0
        values=sorted(s['round_trip_ms'] for s in samples)
        result=dict(passed=True,environment=a.network,physical_environment_requires_external_confirmation=not a.local_receiver,n=len(samples),p95_round_trip_ms=values[math.ceil(.95*len(values))-1],max_ms=max(values),samples=samples,method='same PC perf_counter: input flush -> B authenticated display/echo -> A authenticated display; causal RTT is an upper bound, never divided by two',exe_sha256=hashlib.sha256(exe.read_bytes()).hexdigest())
        (out/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf8');print(json.dumps({k:v for k,v in result.items() if k!='samples'}),flush=True)
    except Exception as error:
        failure=str(error);(out/'failure.json').write_text(json.dumps(dict(error=failure,samples=samples))+'\n',encoding='utf8');raise
    finally:
        if actor is not None and actor.process.poll() is None:
            try:actor.close()
            except Exception as error:cleanup.append(dict(scope='A',error=str(error)))
        if receiver is not None and receiver.poll() is None:
            # Only this local disposable fixture's immutable case. No PID/name
            # kill or remote system control; receiver performs ordinary quit.
            try:
                stop=exe.parent/'test-data'/receiver_case/'stop'
                with stop.open('xb') as stream:stream.write(b'1')
                if receiver.wait(timeout=40)!=0:cleanup.append(dict(scope='local-B',error='receiver nonzero'))
            except Exception as error:cleanup.append(dict(scope='local-B',error=str(error)))
        (out/'cleanup.json').write_text(json.dumps(dict(errors=cleanup))+'\n',encoding='utf8')
        if cleanup and failure is None:raise RuntimeError('probe cleanup not confirmed')
if __name__=='__main__':main()
