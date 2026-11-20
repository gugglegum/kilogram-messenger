"""Actual observer_command gzip/stdin and external Restricted PS5, shared isolated fixture."""
import base64,copy,hashlib,json,os,runpy,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=Path(sys.argv[1]).resolve();PHASE=sys.argv[2]
D=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-driver.py'));fixture=json.loads((OUT/'fixture.json').read_bytes());BASE=Path(fixture['base']);RUN=BASE/'runs/one'
PS='C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe';ROWS=[]
def sha(raw):return hashlib.sha256(raw).hexdigest()
def save(p,v):p.write_text(json.dumps(v,indent=2)+'\n',encoding='utf-8')
def invoke(label,source,request,expected_error=None):
    # Only the existing fixture redirect changes the observer's fixed A base/IP.
    text=source.decode().replace(r'E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f',str(BASE)).replace("$ip = '192.168.0.140'","$ip = '127.0.0.1'")
    args,data=D['observer_command'](text.encode(),request);assert '-ExecutionPolicy' not in args
    start=time.monotonic();r=subprocess.run([PS,*args],input=data,capture_output=True,timeout=15,env=dict(os.environ,PSExecutionPolicyPreference='Restricted'),creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    (OUT/(PHASE+'-'+label+'.stdout.bin')).write_bytes(r.stdout);(OUT/(PHASE+'-'+label+'.stderr.bin')).write_bytes(r.stderr);(OUT/(PHASE+'-'+label+'.stdin.bin')).write_bytes(data)
    save(OUT/(PHASE+'-'+label+'.invocation.json'),dict(argv=[PS,*args],source_sha256=sha(text.encode()),process_Restricted=True,checked_Bypass=False,exit=r.returncode,elapsed=time.monotonic()-start))
    if expected_error:assert r.returncode!=0 and expected_error.encode() in r.stderr,(label,r.stderr)
    else:assert r.returncode==0 and not r.stderr,(label,r.stderr)
    ROWS.append(dict(case=label,refused=bool(expected_error),actual_external_transport=True));return json.loads(r.stdout) if not expected_error else None
source=(ROOT/'scripts/test-terminal-t1-3e-observe.ps1').read_bytes()
request=dict(side='A',host=fixture['host'],attempt='one')
if PHASE=='preconnect':
    request.update(op='preconnect',tag='owner-b-1',crash_restart=True,crash_contexts=fixture['intent']['contexts'])
    old=subprocess.check_output(['git','show','2441dc11d24883ab4a441065144729cc5197d763:scripts/test-terminal-t1-3e-observe.ps1'],timeout=15);(OUT/'observer.original.ps1').write_bytes(old)
    invoke('original-common-file-refusal',old,request,'PSSecurityException')
    proof=invoke('corrected-stale-descriptor',source,request)
    assert proof['disconnected'] and proof['descriptor'] and proof['chat_pid']==fixture['chat_pid'] and not proof['runtime_pids'] and not proof['sockets'] and proof['crash_descriptor_sha256']==fixture['intent']['descriptor_sha256']
    invoke('wrong-contexts',source,request|dict(crash_contexts=['0'*64,'1'*64]),'Crash preconnect contexts')
    state=BASE/'ordinary-agent/control/state.json';raw=state.read_bytes();data=json.loads(raw);data['session']='0'*32;state.write_text(json.dumps(data))
    invoke('wrong-session',source,request,'Crash preconnect session');state.write_bytes(raw)
    descriptor=RUN/'setup/connection/ipc.json';raw=descriptor.read_bytes();descriptor.write_bytes(b'changed')
    invoke('wrong-descriptor',source,request,'LAB_CRASH_OUTCOME');descriptor.write_bytes(raw)
    done=RUN/'owners/2.done.json';raw=done.read_bytes();data=json.loads(raw);data['state']='running';done.write_text(json.dumps(data))
    invoke('unproven-fault',source,request,'LAB_CRASH_NOT_RECONCILED');done.write_bytes(raw)
    common=BASE/'ordinary-agent/test-terminal-t1-3e-lab-common.ps1';raw=common.read_bytes();marker=OUT/'unverified-code-ran'
    common.write_bytes(raw+b'\n'+("[IO.File]::WriteAllText('"+str(marker)+"','bad')").encode())
    invoke('wrong-common',source,request,'CRASH_OBSERVER_COMMON_PIN');assert not marker.exists();common.write_bytes(raw)
    # Same checked bytes after a test-only on-disk replacement hook.
    line=b'        . ([scriptblock]::Create([Text.Encoding]::UTF8.GetString($commonBytes)))';assert source.count(line)==1
    hooked=source.replace(line,b"        [IO.File]::WriteAllText($common,\"throw 'UNVERIFIED_REREAD'\")\n"+line)
    proof=invoke('verified-array-after-file-change',hooked,request);assert proof['chat_pid']==fixture['chat_pid'];common.write_bytes(raw)
elif PHASE=='status':
    runtime=json.loads((OUT/'runtime.json').read_bytes());request.update(op='status',sha256=fixture['exe_sha256'],runtime_pid=runtime['pid'],runtime_started=runtime['started'],runtime_image=runtime['image'],full_status=True)
    value=invoke('full-status-current-runtime',source,request);assert value['ready'] and value['runtime_pid']==runtime['pid'] and value['runtime_instance']==fixture['intent']['runtime_instance']
    raw=base64.b64decode(value['full_status']['data']);assert sha(raw)==value['full_status']['sha256'] and len(raw)==value['full_status']['bytes']
    invoke('wrong-image-hash',source,request|dict(sha256='0'*64),'CLI diagnostic executable SHA-256 mismatch')
    invoke('wrong-runtime-start',source,request|dict(runtime_started=runtime['started']+1),'T13E_RUNTIME_START')
    invoke('wrong-runtime-pid',source,request|dict(runtime_pid=runtime['pid']+1),'T13E_RUNTIME_JOB_PID')
    descriptor=RUN/'setup/connection/ipc.json';raw=descriptor.read_bytes();descriptor.unlink()
    assert invoke('status-before-descriptor',source,request)==dict(ready=False);descriptor.write_bytes(raw)
    identity=invoke('identity',source,dict(side='A',host=fixture['host'],attempt='one',op='identity'));assert identity['administrator'] is False
    ports=invoke('ports',source,dict(side='A',host=fixture['host'],attempt='one',op='ports'));assert isinstance(ports['sockets'],list)
else:raise AssertionError(PHASE)
save(OUT/(PHASE+'-checks.json'),dict(ok=True,cases=ROWS,count=len(ROWS),actual_observer_command=True,full_gzip_stdin=True,process_Restricted=True,no_checked_Bypass=True,
 fixture_path_redirect_only=True,product_protocol_reply_synthetic=True,installed_actors=False))
print(json.dumps(dict(ok=True,phase=PHASE,cases=len(ROWS))),flush=True)
