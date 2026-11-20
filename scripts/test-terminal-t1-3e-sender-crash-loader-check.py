"""External Host.command under process Restricted; shared policy fixtures, no runtime kill."""
import copy,hashlib,json,os,runpy,subprocess,sys,threading,time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1];PS='C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
OUT=Path(sys.argv[1]).resolve();assert ROOT in OUT.parents and not OUT.exists();OUT.mkdir(parents=True)
D=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-1f-driver.py'))
COMMON=ROOT/'scripts/test-terminal-t1-3e-lab-common.ps1';COMMON_PIN='02b2a53e5b9d13f52312059ab1fc8a859de43eb694be72ce0eca00b06fe6346a'
def digest(data):return hashlib.sha256(data).hexdigest()
def save(path,value):
    with path.open('x',encoding='utf-8') as f:json.dump(value,f,indent=2);f.write('\n')
assert digest(COMMON.read_bytes())==COMMON_PIN
original=subprocess.check_output(['git','show','f81eb45513a8005b2baae4f3319c007b4c6bb1c7:scripts/test-terminal-t1-1f-host.ps1'],timeout=15)
(OUT/'original-host.ps1').write_bytes(original)
def ps(code,timeout=20):
    # Bootstrap only. The CHECKED Host.command invocation below gets no Bypass.
    import base64
    return subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',base64.b64encode(code.encode('utf-16-le')).decode()],capture_output=True,timeout=timeout,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
def fixture(label,host_source):
    folder=OUT/label;folder.mkdir();evidence=ROOT/'.tmp'/('cl-'+hashlib.md5((str(OUT)+label).encode()).hexdigest()[:10])
    assert not evidence.exists();save(folder/'fixture-path.json',dict(disposable_path=str(evidence)))
    code="$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';. '"+str(ROOT/'scripts/test-terminal-t1-3e-sender-crash-policy.ps1')+"' -Evidence '"+str(evidence)+"' -FixtureLibrary;\n"+r'''
$hostChild=Get-Process -Id $PID;$hostStart=$hostChild.StartTime.ToFileTimeUtc();$f=Fixture 'transport'
$f|ConvertTo-Json -Depth 16 -Compress
'''
    r=ps(code);(folder/'bootstrap.stdout.bin').write_bytes(r.stdout);(folder/'bootstrap.stderr.bin').write_bytes(r.stderr)
    assert r.returncode==0 and not r.stderr,r.stderr
    data=json.loads(r.stdout);base=Path(data['cfg']['base']);assert evidence in base.parents
    (base/'bin').mkdir();(base/'bin/host.ps1').write_bytes(host_source)
    agent=base/'ordinary-agent';(agent/'control').mkdir(parents=True);(agent/'test-terminal-t1-3e-lab-common.ps1').write_bytes(COMMON.read_bytes())
    save(agent/'lab.json',data['cfg']);save(agent/'control/ready.json',dict(session=data['entry']['session']))
    save(agent/'control/state.json',dict(session=data['entry']['session'],active='one',blocked=None))
    data['entry']['pins']['host.ps1']=digest(host_source)
    for name in ('deadline.ps1','mailbox.cs','setup-pipe.cs','kilogram-bootstrap.exe'):
        data['entry']['pins'][name]=digest(name.encode())
    (Path(data['run'])/'attempt.json').write_text(json.dumps(data['entry']),encoding='utf-8')
    # Preserve the existing shared synthetic result; remove only this fixture's
    # prepublished request/result so the actual first-request branch must publish.
    data['synthetic_reply']=json.loads((Path(data['run'])/'receipts/runtime-fault-result.json').read_bytes())
    for name in ('runtime-fault-request.json','runtime-fault-result.json','runtime-fault-claim.json'):
        path=Path(data['run'])/'receipts'/name;assert evidence in path.resolve().parents;path.unlink()
    host=D['Host']('A',dict(base=str(base),ipv4='127.0.0.1',hostname=data['cfg']['host']),'127.0.0.1','one',data['entry']['pins'],[],folder,time.monotonic()+90)
    binding=dict(agent_session=data['entry']['session'],agent_generation=data['entry']['generation'],worker_pid=data['started']['pid'],worker_started=data['started']['started'])
    return folder,data,host,binding
def invoke(folder,label,host,op,binding,**kwargs):
    argv=host.command(op,**binding,**kwargs);assert '-ExecutionPolicy' not in argv and all('bypass' not in x.lower() for x in argv)
    env=dict(os.environ,PSExecutionPolicyPreference='Restricted');start=time.monotonic()
    r=subprocess.run(argv,capture_output=True,stdin=subprocess.DEVNULL,env=env,timeout=20,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    (folder/(label+'.stdout.bin')).write_bytes(r.stdout);(folder/(label+'.stderr.bin')).write_bytes(r.stderr)
    save(folder/(label+'.invocation.json'),dict(argv=argv,PSExecutionPolicyPreference='Restricted',process_only=True,exit=r.returncode,elapsed=time.monotonic()-start))
    return r
rows=[]
for label,source,success in [('original',original,False),('corrected',(ROOT/'scripts/test-terminal-t1-1f-host.ps1').read_bytes(),True)]:
    folder,data,host,binding=fixture(label,source)
    r=invoke(folder,'descriptor',host,'lab-crash-descriptor',binding)
    if success:
        assert r.returncode==0 and not r.stderr,r.stderr
        reply=json.loads(r.stdout);assert reply==dict(sha256=data['intent']['descriptor_sha256'],profile_name=data['intent']['profile_name'],profile_sha256=data['intent']['profile_sha256'])
    else:assert r.returncode!=0 and b'PSSecurityException' in r.stderr and b'lab-common.ps1' in r.stderr,r.stderr
    rows.append(dict(case=label+'-descriptor',passed=True,actual_transport=True))
    if not success:continue
    request=Path(data['run'])/'receipts/runtime-fault-request.json';result=Path(data['run'])/'receipts/runtime-fault-result.json';assert not request.exists() and not result.exists()
    writes=[];publisher_errors=[];cancel=threading.Event()
    def publish_synthetic_reply():
        try:
            until=time.monotonic()+15
            while not request.exists():
                if cancel.wait(.02) or time.monotonic()>until:return
            raw=request.read_bytes();assert json.loads(raw)==data['intent']
            response=data['synthetic_reply']|dict(digest=digest(raw));temp=result.with_suffix('.pending');temp.write_text(json.dumps(response),encoding='utf-8');temp.rename(result);writes.append(response)
        except Exception as e:publisher_errors.append(repr(e))
    worker=threading.Thread(target=publish_synthetic_reply);worker.start()
    try:r=invoke(folder,'runtime-first',host,'lab-runtime-crash',binding,fault=data['intent'])
    finally:cancel.set();worker.join(timeout=2)
    assert not worker.is_alive() and not publisher_errors,publisher_errors
    assert r.returncode==0 and not r.stderr and len(writes)==1,r.stderr
    reply=json.loads(r.stdout);assert reply==writes[0];raw=request.read_bytes();assert digest(raw)==reply['digest']
    state=Path(host.base)/'ordinary-agent/control/state.json';state.write_text(json.dumps(dict(session=data['entry']['session'],active=None,blocked=None)))
    r=invoke(folder,'runtime-cached',host,'lab-runtime-crash',binding,fault=data['intent']);assert r.returncode==0 and not r.stderr and json.loads(r.stdout)==reply,r.stderr
    changed=copy.deepcopy(data['intent']);changed['request']='0'*32
    r=invoke(folder,'runtime-changed',host,'lab-runtime-crash',binding,fault=changed);assert r.returncode!=0 and b'LAB_CRASH_REPLAY' in r.stderr,r.stderr
    assert request.read_bytes()==raw and len(writes)==1
    rows.extend(dict(case=x,passed=True,actual_transport=True,synthetic_reply_no_kill=True) for x in ('runtime-first','runtime-cached-after-inactive','runtime-changed-refused'))
    r=invoke(folder,'wrong-session',host,'lab-crash-descriptor',binding|dict(agent_session='0'*32));assert r.returncode!=0 and b'Lab attempt binding' in r.stderr
    rows.append(dict(case='identity-refused',passed=True))
for op in ('lab-crash-descriptor','lab-runtime-crash'):
    folder,data,host,binding=fixture('wrong-common-'+op,(ROOT/'scripts/test-terminal-t1-1f-host.ps1').read_bytes())
    common=Path(host.base)/'ordinary-agent/test-terminal-t1-3e-lab-common.ps1';marker=folder/'unverified-executed'
    common.write_bytes(COMMON.read_bytes()+b'\n'+("[IO.File]::WriteAllText('"+str(marker)+"','unverified')").encode())
    r=invoke(folder,'refused',host,op,binding,**(dict(fault=data['intent']) if op=='lab-runtime-crash' else {}))
    assert r.returncode!=0 and b'LAB_CRASH_COMMON_PIN' in r.stderr,r.stderr
    assert not (Path(data['run'])/'receipts/runtime-fault-request.json').exists() and not marker.exists()
    rows.append(dict(case='wrong-common-pin-'+op,passed=True,no_request_published=True))
source=(ROOT/'scripts/test-terminal-t1-1f-host.ps1').read_text()
line="    . ([scriptblock]::Create([Text.Encoding]::UTF8.GetString($commonBytes)))"
assert source.count(line)==1
# Test-only race hook AFTER the real hash check: change the on-disk file, then
# execute the unchanged production load expression through the real transport.
instrumented=source.replace(line,"    [IO.File]::WriteAllText($common,\"throw 'UNVERIFIED_DISK_REREAD'\")\n"+line).encode()
folder,data,host,binding=fixture('same-verified-bytes',instrumented)
r=invoke(folder,'descriptor',host,'lab-crash-descriptor',binding)
assert r.returncode==0 and not r.stderr and json.loads(r.stdout)['sha256']==data['intent']['descriptor_sha256'],r.stderr
assert digest((Path(host.base)/'ordinary-agent/test-terminal-t1-3e-lab-common.ps1').read_bytes())!=COMMON_PIN
rows.append(dict(case='executes_verified_bytes_after_disk_changed',passed=True,actual_transport=True,test_only_after_hash_hook=True))
save(OUT/'checks.json',dict(ok=True,cases=rows,count=len(rows),actual_Host_command=True,process_Restricted=True,checked_invocation_Bypass=False,
    shared_policy_Fixture=True,synthetic_reply=True,runtime_termination=False,installed_actors=False,system_policy_changed=False,
    host_source_sha256=digest((ROOT/'scripts/test-terminal-t1-1f-host.ps1').read_bytes()),common_source_sha256=COMMON_PIN))
print(json.dumps(dict(ok=True,cases=len(rows),actual_transport=True,no_kill=True)),flush=True)
