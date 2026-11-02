from pathlib import Path
import json,hashlib,subprocess,re
b=Path('.tmp/s54-block3e-p11a7-01');baseline='4d40bb3f5e21fb5e9108c3a702ccf8a10077153b'
def read(p):return json.loads(Path(p).read_text(encoding='utf-8-sig'))
def git(*args):return subprocess.check_output(['git',*args],text=True,timeout=180).strip()
def pin(p):
 p=Path(p);raw=p.read_bytes();return dict(path=p.as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
def check(row,path=None):
 now=pin(path or row['path']);assert now['sha256']==row['sha256'] and ('bytes' not in row or now['bytes']==row['bytes']),now
assert git('rev-parse','HEAD')==baseline and not git('diff','--cached','--name-only')
sources=read(b/'source-paths.json');assert len(sources)==10
assert not git('diff','HEAD','--name-only','--','Cargo.toml','Cargo.lock','apps/kilogram-cli/Cargo.toml','scripts/run-cargo-tests-stable.ps1','scripts/cargo-frozen-artifacts.ps1','scripts/migration-s5-l1-deadline.ps1')
assert set(git('diff','--name-only').splitlines())=={'ai-docs/README.md','ai-docs/chat-coordination.md','ai-docs/next-stage-plan.md'}|{p for p in sources if git('ls-files',p)}
assert set(git('ls-files','--others','--exclude-standard','--','apps','crates').splitlines())=={p for p in sources if not git('ls-files',p)}
for row in read(b/'navigation-pins.json'):check(row)
preimages=read(b/'preimages.json');assert len(preimages)==15
for row in preimages:check(row,row['archive'])
for row in read(b/'validation-inputs.json'):check(row)
failed=read(b/'failed-clippy-01/sources.json')
for row in failed:check(row,row['archive'])
assert len(failed)==17
compiler=read(b/'compile-01.capture.json');pure=read(b/'pure-01.capture.json')
assert compiler['build_inputs_before']==compiler['build_inputs_after']==pure['build_inputs_before']==pure['build_inputs_after']
for row in compiler['build_inputs_after']['source_inputs']:check(row)
assert len(compiler['units'])==len(pure['units'])==501 and all(u['fresh'] for u in pure['units']) and pure['frozen_guard_verified']
identity=lambda units:[{k:v for k,v in u.items() if k!='fresh'} for u in units]
assert identity(compiler['units'])==identity(pure['units']) and compiler['stable_artifacts']==pure['stable_artifacts']
for row in compiler['stable_artifacts']:check(row)
images=[]
for row in preimages:
 if row['path'].endswith('.exe'):
  now=pin(row['path']);images.append(dict(before=row,after=now,unchanged=now['sha256']==row['sha256']))
assert len(images)==10 and sum(not row['unchanged'] for row in images)==1
assert next(row for row in images if not row['unchanged'])['after']['path'].endswith('kilogram-cli-test-migration_s5_process-tests.exe')
fields=read('.tmp/s54-block3e-p11a6-01/review.json')['fixed_field_images_unchanged']
for row in fields:check(row)
steps=[read(b/(name+'.json')) for name in ['fmt-01','clippy-01','fmt-02','clippy-02','compile-01','pure-01']]
assert [row['exit_code'] for row in steps]==[0,1,0,0,0,0]
assert all(row['cleanup_verified'] and row['actual_launcher_membership_verified'] and not row['work_expired'] and row['members_after_cleanup']==[] for row in steps)
log=(b/'pure-01.stdout.log').read_text()
expected=read(b/'test-selection.json')['expected_names']
segments=re.split(r'^running_isolated_test=(\S+)\s*$',log,flags=re.M)
names=[]
for i in range(1,len(segments),2):
 name=segments[i];body=segments[i+1];names.append(name)
 assert re.search(r'^test result: ok\. 1 passed; 0 failed; 0 ignored;',body,re.M),name
assert names==expected and len(names)==13 and 'isolated_test_count=13' in log
assert not re.search(r'^test result: FAILED',log,re.M)
assert len(re.findall(r'^test result: ok\. 0 passed;',log,re.M))==6
receipt=steps[-1];members=receipt['capture_harness_membership'];assert len(members)==8
harness=next(row for row in compiler['stable_artifacts'] if row['target']=='migration_s5_process')
admitdir=b/'pure-01.admission';intent=read(admitdir/'admission.json')
assert intent['owner_pid']==receipt['outer_job_owner_pid']
assert receipt['members_before_cleanup']==[]
for member in members:
 assert member['actual_handle_membership_verified'] and member['sha256']==harness['sha256'] and member['owner_pid']==intent['owner_pid'] and member['launcher_pid']==intent['launcher_pid']
 request=read(member['request']);permit=read(admitdir/f"permit-{member['harness_pid']}.json")
 assert request['membership_verified'] is False and permit['membership_verified'] is True
 assert {k:v for k,v in request.items() if k!='membership_verified'}=={k:v for k,v in permit.items() if k!='membership_verified'}
 assert request['nonce']==member['nonce'] and request['harness_pid']==member['harness_pid']
case_rows=[]
for mode,pid,directory in re.findall(r'synthetic_capture_case=(\S+) pid=(\d+) evidence=(.+)$',log,re.M):
 directory=Path(directory.strip());case=read(directory/'case-result.json');facts=case['actual']['capture']
 assert facts['pid']==int(pid) and facts['waited'] and facts['exit_observed'] and not facts['cleanup_pending'] and facts['stdout_eof'] and facts['stderr_eof']
 assert facts['classification']==case['expected'] and case['mode']==mode and case['elapsed_seconds']<35
 assert case['actual']['harness_pid'] in {m['harness_pid'] for m in members}
 assert case['actual']['containment_owner_pid']==intent['owner_pid'] and case['actual']['attempt']==intent['attempt']
 rawpins=[]
 for p in directory.iterdir():
  if p.name.startswith('command-'):
   raw=p.read_bytes();assert b'S54_STATUS_SENTINEL' not in raw and b'S54_STDERR_SENTINEL' not in raw and b'"keys":' not in raw
  rawpins.append(pin(p))
 case_rows.append(dict(mode=mode,pid=int(pid),directory=directory.as_posix(),result=case,raw_files=rawpins))
assert len(case_rows)==8 and len({row['pid'] for row in case_rows})==8
assert {r['mode']:r['result']['expected'] for r in case_rows}=={'valid':'success','pressure':'success','malformed':'invalid_json','invalid-utf8':'invalid_utf8','nonzero':'nonzero_exit','stdout-overflow':'stdout_cap','stderr-overflow':'stderr_cap','stall':'deadline'}
contracts=['ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block3e-'+name for name in ['p11a7-capture-task-01.md','p11a7-capture-api-gap-01.md','p11a7-capture-api-gap-01.json','p11a7-capture-gap-root-decision-01.md']]
pins=[pin(path) for path in contracts]
assert pins[0]['sha256']=='2d1a0e4b19112be3a499948182fac4d7fbc97972e8ab3d411b9f1eca4b7fa845'
assert pins[-1]['sha256']=='8568b5931988a9c21102731248c9e2e81bf277931fc7b881e568c774250d02de'
assert subprocess.run(['git','diff','--check','--',*sources],timeout=180).returncode==0
for path in sources:assert all(line.rstrip()==line for line in Path(path).read_text().splitlines()),path
rows=[pin(path)|{'git_blob':git('hash-object','--path='+path,path)} for path in sources]
review=dict(classification='P11A7 opt-in status capture; actual Windows named-pipe synthetic checks, no messenger/profile/store/network/whole S54',baseline=baseline,tree=git('rev-parse','HEAD^{tree}'),source=rows,contracts=pins,steps=steps,actual_isolated_names=names,aggregate_tests=0,actual_cases=case_rows,actual_synthetic_children=8,outer_job_membership=members,actual_outer_job_empty_after_return=True,artifacts=[pin(row['path']) for row in compiler['stable_artifacts']],compiler_units=501,pure_fresh_units=501,compiler_source_inputs=len(compiler['build_inputs_after']['source_inputs']),compiler_inputs_equal=True,frozen_guard_verified=True,validation_inputs=read(b/'validation-inputs.json'),preimages_preserved=preimages,failed_sources_preserved=failed,compiler_images=images,fixed_field_images_unchanged=fields,raw_capture_pins=[pin(b/name) for name in ['compile-01.capture.json','pure-01.capture.json','pure-01.expected.json','pure-01.inventory.json']],helper_revisions=dict(first_fmt_clippy=pin(b/'helper-revision-01/check.ps1'),compile_and_actual_checks=pin(b/'check.ps1')),limitations=['Pending kill branch uses deterministic pure state injection; no deliberately unkillable process','Actual messenger old/current status routing and early S54 Controller remain source-reviewed/compiled only','Future whole runner needs equivalent verified outer containment binding; no whole activation','Zeroizing raw byte buffers does not promise wiping serde values or OS pipe buffers'],documentation_helpers_classification='Review/report/archive generators are mutable and excluded from historical validation helper pins',commit_push=False)
(b/'review.json').write_text(json.dumps(review,indent=2)+'\n')
print(json.dumps(dict(review=pin(b/'review.json'),sources=len(rows),units=501,source_inputs=review['compiler_source_inputs'],isolated=len(names),actual_children=8,outer_cleanup=True),indent=2))
