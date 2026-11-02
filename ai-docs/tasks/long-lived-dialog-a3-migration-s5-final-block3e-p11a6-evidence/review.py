from pathlib import Path
import json,hashlib,subprocess,re
b=Path('.tmp/s54-block3e-p11a6-01');baseline='8d4773ad5fe1b72cbb21d5f57fe045e6f654af6e'
def git(*a):return subprocess.check_output(['git',*a],text=True,timeout=180).strip()
def read(n):return json.loads((b/n).read_text(encoding='utf-8-sig'))
def pin(p):
    p=Path(p);raw=p.read_bytes();return dict(path=p.as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
def check(r,p=None):
    actual=pin(p or r['path']);assert actual['bytes']==r['bytes'] and actual['sha256']==r['sha256'],actual
assert git('rev-parse','HEAD')==baseline and not git('diff','--cached','--name-only')
sources=read('source-paths.json');assert len(sources)==7
navigation={'ai-docs/README.md','ai-docs/chat-coordination.md','ai-docs/next-stage-plan.md'}
assert set(git('diff','--name-only').splitlines())==navigation|{p for p in sources if git('ls-files',p)}
assert set(git('ls-files','--others','--exclude-standard','--','apps','crates').splitlines())=={p for p in sources if not git('ls-files',p)}
for r in read('navigation-pins.json'):check(r)
preimages=read('preimages.json');assert len(preimages)==14
for r in preimages:check(r,r['archive'])
c=read('compile-02.capture.json');p=read('pure-02.capture.json')
assert c['build_inputs_before']==c['build_inputs_after']==p['build_inputs_before']==p['build_inputs_after']
assert len(c['units'])==501==len(p['units']) and all(u['fresh'] for u in p['units']) and p['frozen_guard_verified'] is True
identity=lambda units:[{k:v for k,v in r.items() if k!='fresh'} for r in units]
assert identity(c['units'])==identity(p['units']) and c['stable_artifacts']==p['stable_artifacts']
for r in c['build_inputs_after']['source_inputs']:assert pin(r['path'])['sha256']==r['sha256'],r['path']
assert len(c['build_inputs_after']['source_inputs'])==586
normal=[u for u in c['units'] if u['package']=='kilogram-cli' and u['executable'] and not u['profile']['test']];assert len(normal)==1
for r in c['stable_artifacts']:check(r)
changed=[];unchanged=[]
for r in preimages:
    if r['path'].lower().endswith('.exe'):
        after=pin(r['path']);(unchanged if after['sha256']==r['sha256'] else changed).append(dict(before=r,after=after))
assert len(changed)==1 and changed[0]['after']['path'].endswith('kilogram-cli-test-migration_s5_process-tests.exe')
old=json.loads(Path('.tmp/s54-block3e-p11a5-01/review.json').read_text());fields=old['fixed_field_images_unchanged']
for r in fields:check(r)
steps=[read(n+'.json') for n in ['fmt-01','clippy-01','clippy-02','fmt-02','clippy-03','compile-01','pure-01','fmt-03','clippy-04','compile-02','pure-02']]
assert [r['exit_code'] for r in steps]==[0,1,1,0,0,0,1,0,0,0,0] and all(r['cleanup_verified'] for r in steps)
selection=read('test-selection.json');log=(b/'pure-02.stdout.log').read_text()
actual=re.findall(r'^test (\S+) \.\.\. ok$',log,re.M)
isolated=re.findall(r'^running_isolated_test=(\S+)$',log,re.M)
assert len(actual)==len(isolated)==19 and set(actual)==set(selection['expected_names'])==set(isolated)
assert not re.search(r'^test \S+ \.\.\. (FAILED|ignored)',log,re.M) and 'isolated_test_count=19' in log
failed_log=(b/'pure-01.stdout.log').read_text();initial_pass=re.findall(r'^test (\S+) \.\.\. ok$',failed_log,re.M);initial_fail=re.findall(r'^test (\S+) \.\.\. FAILED$',failed_log,re.M)
assert len(initial_pass)==5 and len(initial_fail)==1 and 's54_p11a6_cut_content_' in initial_fail[0]
failures=[]
for folder in ['failed-clippy-01','failed-clippy-02','failed-pure-01']:
    for row in read(folder+'/sources.json'):check(row);failures.append(row)
failed_exe=pin(b/'failed-pure-01/kilogram-cli-test-migration_s5_process-tests.exe')
first=read('pure-01.capture.json');prior_exe=next(x for x in first['stable_artifacts'] if 'migration_s5_process' in x['path']);check(prior_exe,failed_exe['path'])
assert first['frozen_guard_verified'] is True
contracts=['ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block3e-'+n for n in ['p11a6-native-task-01.md','p11-contract-01.md','p11-contract-root-review-01.md']]
assert pin(contracts[0])['sha256']=='457646c9e24c1458c60c100c415a79f95cd8292091c36bf42f625d132aee030a'
assert pin(contracts[1])['sha256']=='5d634a445e9998a2cf2e5eeeaaff88633a9ac5b5d4dca802001a404401d7daa0'
helpers=['check.ps1','compile.ps1','clippy.ps1','pure.ps1','fmt.ps1','test-selection.json','isolated-tests.json']
pins=[pin(b/n) for n in helpers]
premises=['apps/kilogram-cli/src/ordered_dialog/'+n for n in ['runtime7/native.rs','runtime7/data2.rs','runtime7/control.rs','runtime7/owner.rs','engine.rs','state.rs']]+['crates/kilogram-ratchet/src/ordered.rs','crates/kilogram-ratchet/src/ordered_v2.rs','apps/kilogram-cli/tests/support/migration_s54_offline_witness.rs']
rows=[pin(path)|{'git_blob':git('hash-object','--path='+path,path)} for path in sources]
result=dict(classification='P11A6 dormant same-opening Native checkpoint; actual in-memory crypto restore/gates only, no actual profile/repository or whole scenario',baseline=baseline,source=rows,contracts=[pin(x) for x in contracts],premises=[pin(x) for x in premises],steps=steps,actual_passed_tests=actual,isolated_processes=19,new_tests=8,affected_tests=11,first_failed_run=dict(passed=initial_pass,failed=initial_fail,executable=failed_exe),preimages_preserved=preimages,failed_sources_preserved=failures,compiler_source_input_count=586,units=501,pure_fresh_units=501,artifacts=[pin(normal[0]['executable'])]+c['stable_artifacts'],unchanged_images=unchanged,changed_images=changed,fixed_field_images_unchanged=fields,validation_helpers=pins,documentation_helpers_classification='Report/archive generation helpers are mutable documentation tools, excluded from historical validation-input pins; final artifact/provenance pins are recorded only after generation.',actual_profile_reader_executed=False,runtime_scenario_executed=False,commit_push_performed=False,index_empty=True,navigation_preserved=sorted(navigation))
out=b/'review.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(source_paths=7,raw_source_bytes=sum(r['bytes'] for r in rows),actual_passed=19,units=501,source_inputs=586,changed_images=len(changed),unchanged_images=len(unchanged),review=pin(out))))
