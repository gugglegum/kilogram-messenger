from pathlib import Path
import hashlib,json,re,subprocess
b=Path('.tmp/s54-block3e-p11a5-01')
baseline='1f2e950ad991ef353a4b0ecdfa2976fe1a97e872'
def git(*args):return subprocess.check_output(['git',*args],text=True,timeout=180).strip()
def pin(path):
    p=Path(path);data=p.read_bytes();return dict(path=p.as_posix(),bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
def check(r,path=None):
    actual=pin(path or r['path']);assert actual['bytes']==r['bytes'] and actual['sha256']==r['sha256'],actual
def read(n):return json.loads((b/n).read_text(encoding='utf-8-sig'))
assert git('rev-parse','HEAD')==baseline and not git('diff','--cached','--name-only')
sources=read('source-paths.json');assert len(sources)==15
navigation={'ai-docs/README.md','ai-docs/chat-coordination.md','ai-docs/next-stage-plan.md'}
assert set(git('diff','--name-only').splitlines())==navigation|{p for p in sources if git('ls-files',p)}
assert set(git('ls-files','--others','--exclude-standard','--','apps','crates').splitlines())=={p for p in sources if not git('ls-files',p)}
for p in read('navigation-pins.json'):check(p)
preimages=read('preimages.json')+[read('state-preimage.json')]
for p in preimages:check(p,p['archive'])
c=read('compile-02.capture.json');p=read('pure-01.capture.json')
assert c['build_inputs_before']==c['build_inputs_after']==p['build_inputs_before']==p['build_inputs_after']
assert p['frozen_guard_verified'] is True and len(c['units'])==len(p['units']) and all(u['fresh'] for u in p['units'])
for row in c['build_inputs_after']['source_inputs']:assert pin(row['path'])['sha256']==row['sha256'],row['path']
identity=lambda units:[{k:v for k,v in r.items() if k!='fresh'} for r in units]
assert identity(c['units'])==identity(p['units']) and c['stable_artifacts']==p['stable_artifacts']
normal=[u for u in c['units'] if u['package']=='kilogram-cli' and u['executable'] and not u['profile']['test']];assert len(normal)==1
artifacts=[pin(normal[0]['executable'])]+c['stable_artifacts']
for r in artifacts:check(r)
prior=json.loads(Path('.tmp/s54-block3e-p11a4-01/review.json').read_text())
fields=prior['fixed_field_images_unchanged']
for r in fields:check(r)
steps=[read(n+'.json') for n in ['fmt-01','clippy-01','clippy-02','clippy-03','fmt-02','clippy-04','fmt-03','clippy-05','fmt-04','clippy-06','compile-01','fmt-05','clippy-07','compile-02','pure-01']]
assert [s['exit_code'] for s in steps]==[0,1,1,0,0,0,0,0,0,0,0,0,0,0,0] and all(s['cleanup_verified'] for s in steps)
selection=read('test-selection.json');log=(b/'pure-01.stdout.log').read_text()
actual=re.findall(r'^test (\S+) \.\.\. ok$',log,re.M)
assert len(actual)==30 and {n.split('::')[-1] for n in actual}==set(selection['expected_names'])
assert not re.search(r'^test \S+ \.\.\. (FAILED|ignored)',log,re.M)
assert '29 passed; 0 failed; 0 ignored' in log and '1 passed; 0 failed; 0 ignored' in log
contracts=['ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block3e-'+n for n in ['p11a5-reader-task-01.md','p11-contract-01.md','p11-contract-root-review-01.md']]
assert pin(contracts[0])['sha256']=='d5fbfd60162e37ed7fdb9e0fa57a8d130681bc62893c814ca5caf24d5f6ae293'
assert pin(contracts[1])['sha256']=='5d634a445e9998a2cf2e5eeeaaff88633a9ac5b5d4dca802001a404401d7daa0'
rows=[pin(path)|{'git_blob':git('hash-object','--path='+path,path)} for path in sources]
unchanged=[];changed=[]
for old in preimages:
    if old['path'].lower().endswith('.exe'):
        target=unchanged if pin(old['path'])['sha256']==old['sha256'] else changed;target.append(dict(before=old,after=pin(old['path'])))
failures=[]
for folder in ['failed-clippy-01','failed-clippy-02']:
    for row in read(folder+'/sources.json'):check(row);failures.append(row)
evidence=[pin(path) for path in sorted(b.iterdir()) if path.is_file() and path.name not in {'review.json','review.py','review.stdout.log'}]
result=dict(classification='P11A5 dormant stopped source/receipt repository adapter, source-reviewed integration; only pure synthetic gates/signature seams executed, no actual repository/profile/Native/runtime',baseline=baseline,source=rows,contracts=[pin(x) for x in contracts],steps=steps,actual_passed_tests=actual,compiler_source_input_count=len(c['build_inputs_after']['source_inputs']),units=len(c['units']),pure_fresh_units=sum(u['fresh'] for u in p['units']),normal_cli_unit=normal[0],artifacts=artifacts,preimages_preserved=preimages,failed_sources_preserved=failures,unchanged_compiler_images=unchanged,changed_compiler_images=changed,fixed_field_images_unchanged=fields,index_empty=True,navigation_preserved=sorted(navigation),actual_repository_reader_executed=False,old_private_fixtures_read=False,native_checkpoint_executed=False,runtime_executed=False,commit_push_performed=False,evidence=evidence)
output=b/'review.json';assert not output.exists();output.write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
print(json.dumps(dict(source_paths=len(rows),raw_source_bytes=sum(r['bytes'] for r in rows),passed=len(actual),units=len(c['units']),source_inputs=len(c['build_inputs_after']['source_inputs']),unchanged_compiler_images=len(unchanged),changed_compiler_images=len(changed),review=pin(output))))
