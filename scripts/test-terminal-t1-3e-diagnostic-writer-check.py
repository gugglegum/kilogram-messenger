"""Read real Rust Sink output with the production two-scope PS reader/coverage validator."""
import copy,hashlib,json,runpy,shutil,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
if sys.argv[1]=='--candidate-build':
    manifest=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=False)
    module=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-offline-recipient.py'));build=json.loads(manifest.read_bytes())
    module['validate_candidate_build'](build,ROOT);cases=[]
    for scenario in ['old_schema','published_label','wrong_base','missing_input','duplicate_input','unrelated_delta','overlay_hash','raw_hash','forged_source','candidate_id','receipt_hash','resource_policy']:
        bad=copy.deepcopy(build)
        if scenario=='old_schema':bad['schema']=1
        elif scenario=='published_label':bad['source_kind']='clean_published_worktree'
        elif scenario=='wrong_base':bad['base_commit']='a61385a00e12fbe0b78805fe7655afb5b3d6bcbe'
        elif scenario=='missing_input':bad['inputs'].pop()
        elif scenario=='duplicate_input':bad['inputs'].append(copy.deepcopy(bad['inputs'][0]))
        elif scenario=='unrelated_delta':next(r for r in bad['inputs'] if r['git_path'] not in module['CANDIDATE_PATCHES'])['git_blob']='0'*40
        elif scenario=='overlay_hash':next(iter(bad['patches'].values()))['sha256']='0'*64
        elif scenario=='raw_hash':bad['inputs'][0]['sha256']='0'*64
        elif scenario=='forged_source':
            row=bad['inputs'][0];spoof=out/'forged-input';spoof.write_bytes((ROOT/row['path']).read_bytes()+b'\nforged\n')
            row['path']=str(spoof.relative_to(ROOT)).replace('\\','/');row['sha256']=hashlib.sha256(spoof.read_bytes()).hexdigest()
        elif scenario=='candidate_id':bad['candidate_id']='0'*64
        elif scenario=='receipt_hash':bad['receipt']['sha256']='0'*64
        else:bad['jobs']=2
        try:module['validate_candidate_build'](bad,ROOT)
        except RuntimeError:cases.append(scenario)
        else:raise AssertionError('Invalid candidate build accepted: '+scenario)
    result=dict(passed=True,valid_candidate=True,negative_cases=cases,manifest_sha256=hashlib.sha256(manifest.read_bytes()).hexdigest(),source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in ['scripts/test-terminal-t1-3e-offline-recipient.py','scripts/test-terminal-t1-3e-diagnostic-writer-check.py']})
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8');print(json.dumps(dict(passed=True,negative_cases=len(cases))),flush=True);sys.exit(0)
fixture=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=False)
meta=json.loads((fixture/'scopes.json').read_bytes());scopes=meta['scopes'];nonce=meta['nonce'];session='b'*32
assert len(scopes)==2 and len({s[k] for s in scopes for k in ['chat_pid','runtime_pid']})==4
assert meta['old_files_unchanged'] and meta['marker_unchanged'] and sum(meta['cap_successes'])==4
M=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-admission-diagnostic.py'))
PS=r'C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n',encoding='utf-8')
results=[]
for label,home,index,previous in [('first',fixture/'first-home',0,[]),('second',fixture/'runs/one/setup',1,[scopes[0]]),('first-after-second',fixture/'runs/one/setup',0,[scopes[1]]),('full-without-prior',fixture/'runs/one/setup',1,[])]:
    base=out/label;diag=base/'runs/one/setup/lab-diagnostics-v1';diag.mkdir(parents=True)
    source=home/'lab-diagnostics-v1';pins={p.name:sha(p) for p in source.iterdir()}
    assert len(pins)==(15 if label=='first' else 29)
    for p in source.iterdir():shutil.copyfile(p,diag/p.name)
    assert {p.name:sha(p) for p in diag.iterdir()}==pins
    save(base/'runs/one/attempt.json',dict(attempt='one',generation=nonce,session=session,pins={}))
    save(base/'runs/one/started.json',dict(generation=nonce,pid=123,started=134395772085441302))
    save(base/'ordinary-agent/control/ready.json',dict(session=session));save(base/'ordinary-agent/control/state.json',dict(session=session,active=None,blocked=None))
    scope=scopes[index];request=dict(op='lab-diagnostic-read',base=str(base),attempt='one',agent_session=session,agent_generation=nonce,worker_pid=123,worker_started=134395772085441302,pins={},diagnostic_chat_pid=scope['chat_pid'],diagnostic_runtime_pid=scope['runtime_pid'],diagnostic_runtime_instance=scope['instance'])
    if previous:request['diagnostic_previous']=[dict(diagnostic_chat_pid=s['chat_pid'],diagnostic_runtime_pid=s['runtime_pid'],diagnostic_runtime_instance=s['instance']) for s in previous]
    save(base/'request.json',request)
    host=str(ROOT/'scripts/test-terminal-t1-1f-host.ps1').replace("'","''");req=str(base/'request.json').replace("'","''")
    code='Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"; '+f"& ([scriptblock]::Create([IO.File]::ReadAllText('{host}'))) -Request ([IO.File]::ReadAllText('{req}'))"
    (base/'read.ps1').write_text(code,encoding='utf-8');started=time.monotonic()
    p=subprocess.run([PS,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(base/'read.ps1')],capture_output=True,timeout=25,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    (base/'stdout.bin').write_bytes(p.stdout);(base/'stderr.bin').write_bytes(p.stderr)
    if label=='full-without-prior':
        assert p.returncode!=0 and b'Diagnostic file count cap' in p.stderr
        results.append(dict(case=label,rejected=True,exit=p.returncode));continue
    assert p.returncode==0 and not p.stderr,p.stderr
    reply=json.loads(p.stdout);records,coverage=M['shutdown_records'](reply,nonce,scope['chat_pid'],scope['runtime_pid'],scope['instance'])
    assert coverage['complete'] and len(records)==14 and len(coverage['startup_kinds'])==9 and not reply['rejected']
    assert reply.get('prior_validated',0)==14*bool(previous)
    for name,raw in records.items():assert raw==(source/name).read_bytes()
    save(base/'coverage.json',coverage);save(base/'receipt-pins.json',pins)
    results.append(dict(case=label,exit=0,records=len(records),prior_validated=reply.get('prior_validated',0),coverage_complete=True,seconds=time.monotonic()-started))
save(out/'result.json',dict(passed=True,real_rust_writer=True,real_ps_reader=True,production_coverage_validator=True,actual_child_pids=scopes,synthetic_payloads=True,handwritten_receipts=False,results=results,source_sha256={p:sha(ROOT/p) for p in ['crates/kilogram-runtime-ipc/src/lab_diagnostics.rs','crates/kilogram-runtime-ipc/tests/lab_diagnostics_writer.rs','scripts/test-terminal-t1-1f-host.ps1','scripts/test-terminal-t1-3e-admission-diagnostic.py']}))
print(json.dumps(dict(passed=True,real_writer_reader=True,scopes=2,records_per_scope=14,cases=len(results))),flush=True)
