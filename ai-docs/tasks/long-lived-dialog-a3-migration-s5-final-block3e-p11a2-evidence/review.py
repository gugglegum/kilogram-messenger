from pathlib import Path
import hashlib,json,re,subprocess
base=Path('.tmp/s54-block3e-p11a2-01')
baseline='5fef1926d532d2a5b926739d71af7680818550e8'
git=lambda *args:subprocess.check_output(['git',*args],text=True).strip()
def pin(path):
 p=Path(path);data=p.read_bytes()
 return dict(path=p.as_posix(),bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
def check_pin(row,path=None):
 actual=pin(path or row['path']);assert actual['bytes']==row['bytes'] and actual['sha256']==row['sha256'],actual
def read(name):return json.loads((base/name).read_text(encoding='utf-8-sig'))
assert git('rev-parse','HEAD')==baseline and not git('diff','--cached','--name-only')
sources=sorted('apps/kilogram-cli/tests/support/migration_s54_'+n+'.rs' for n in ['relay_locators','relay_locator_tests'])
tracked=set(git('diff','--name-only').splitlines());navigation={'ai-docs/README.md','ai-docs/chat-coordination.md','ai-docs/next-stage-plan.md'}
assert tracked==navigation|set(sources[:0])|{p for p in sources if git('ls-files',p)}
assert set(git('ls-files','--others','--exclude-standard','--','apps','crates').splitlines())=={p for p in sources if not git('ls-files',p)}
preimages=read('preimages.json')
assert len(preimages)==9
for r in preimages:check_pin(r,r['archive'])
for r in read('navigation-pins.json'):check_pin(r)
c=read('compile-01.capture.json');p=read('pure-01.capture.json')
assert c['build_inputs_before']==c['build_inputs_after']==p['build_inputs_before']==p['build_inputs_after']
assert p['frozen_guard_verified'] is True and len(c['units'])==501==len(p['units']) and all(u['fresh'] for u in p['units'])
for row in c['build_inputs_after']['source_inputs']:
 assert pin(row['path'])['sha256']==row['sha256'],row['path']
identity=lambda units:[{k:v for k,v in r.items() if k!='fresh'} for r in units]
assert identity(c['units'])==identity(p['units']) and c['stable_artifacts']==p['stable_artifacts']
normal=[u for u in c['units'] if u['package']=='kilogram-cli' and u['executable'] and not u['profile']['test']]
assert len(normal)==1 and normal[0]['profile']=={'opt_level':'3','debuginfo':'line-tables-only','debug_assertions':True,'overflow_checks':True,'test':False}
for r in c['stable_artifacts']:check_pin(r)
normal_pin=pin(normal[0]['executable']);assert normal_pin['sha256']=='f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9'
old=json.loads(Path('.tmp/s54-block3c-p9a-02/review.json').read_text())
fields=old['fixed_field_images_unchanged']
for r in fields:check_pin(r)
steps=[read(n+'.json') for n in ['clippy-01','compile-01','pure-01','fmt-01']]
assert [s['exit_code'] for s in steps]==[0,0,0,0] and all(s['cleanup_verified'] for s in steps)
selection=read('test-selection.json');log=(base/'pure-01.stdout.log').read_text()
actual=re.findall(r'^test (\S+) \.\.\. ok$',log,re.M)
assert len(actual)==10 and {n.split('::')[-1] for n in actual}==set(selection['expected_names'])
assert not re.search(r'^test \S+ \.\.\. (FAILED|ignored)',log,re.M) and '10 passed; 0 failed; 0 ignored' in log
contracts=['ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block3e-'+n for n in ['p11a2-locators-task-01.md','p11-contract-01.md','p11-contract-root-review-01.md']]
assert pin(contracts[0])['sha256']=='89b649d4ef316c8e2344f6215c5141fed768fb9bb52ab45a6d759a7bdea08306'
assert pin(contracts[1])['sha256']=='5d634a445e9998a2cf2e5eeeaaff88633a9ac5b5d4dca802001a404401d7daa0'
rows=[]
for path in sources:
 r=pin(path);r['git_blob']=git('hash-object','--path='+path,path);rows.append(r)
assert len(c['build_inputs_after']['source_inputs'])==571
for r in preimages[:7]:
 if not r['path'].endswith('kilogram-cli-test-migration_s5_process-tests.exe'):check_pin(r)
evidence=[pin(x) for x in sorted(base.iterdir()) if x.is_file() and x.name not in {'review.json','review.py','review.stdout.log'}]
result=dict(classification='P11A2 dormant authenticated same-route locator support; actual restart/publication/admission unexecuted',baseline=baseline,source=rows,contracts=[pin(x) for x in contracts],steps=steps,actual_passed_tests=actual,compiler_source_input_count=len(c['build_inputs_after']['source_inputs']),units=len(c['units']),pure_fresh_units=sum(u['fresh'] for u in p['units']),normal_cli_unit=normal[0],artifacts=[normal_pin]+c['stable_artifacts'],preimages_preserved=preimages,fixed_field_images_unchanged=fields,index_empty=True,navigation_preserved=sorted(navigation),runtime_executed=False,old_private_fixtures_read=False,commit_push_performed=False,evidence=evidence)
output=base/'review.json';assert not output.exists()
output.write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'source_paths':len(rows),'raw_source_bytes':sum(r['bytes'] for r in rows),'passed':len(actual),'units':len(c['units']),'source_inputs':len(c['build_inputs_after']['source_inputs']),'review':pin(output)},ensure_ascii=False))
