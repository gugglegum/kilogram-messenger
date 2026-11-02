"""Construct public synthetic metadata from constants, never read/copy an old fixture."""
from pathlib import Path
import json, hashlib, sys, time, copy
w=Path.cwd(); b=w/'.tmp/s54-block4f-path-implementation-01'
step=sys.argv[1]; spec=json.loads((b/(step+'.spec.json')).read_text(encoding='utf-8-sig'))
allowed=w/'.tmp/s54-block4b-r1-01/rust-public-fixtures'
def guard(p,boundary,fresh=False):
    assert p.is_absolute() and p.is_relative_to(boundary) and p.resolve()==p
    assert not fresh or not p.exists()
    for a in p.parents:
        if a.exists(): assert not getattr(a.lstat(),'st_file_attributes',0)&0x400
        if a==boundary:break
    return p
def mkdir(p,boundary):
    if p.exists():return
    guard(p,boundary,True)
    if not p.parent.exists():mkdir(p.parent,boundary)
    p.mkdir()
def raw(p,data,boundary):
    guard(p,boundary,True);mkdir(p.parent,boundary);guard(p,boundary,True);p.write_bytes(data);return pin(p)
def save(p,v,boundary):return raw(p,json.dumps(v,ensure_ascii=False,separators=(',',':')).encode(),boundary)
def pin(p):
    data=p.read_bytes();return dict(path=str(p),bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
required=['kilogram-cli','kilogram-state','kilogram-identity','kilogram-protocol','kilogram-transport-iroh','kilogram-ratchet','redb','ed25519-dalek','curve25519-dalek','chacha20poly1305','iroh','vodozemac']
cases=['positive-P','positive-F','case-separator','duplicate','foreign','queried-content-index','queried-missing-query','queried-reparse-query','unqueried-content-index','unqueried-missing-index','unqueried-reparse-index','after-last-query-content','after-last-query-missing','after-last-query-reparse','after-last-query-embedded','malformed','profile','graph','raw','unit','tool','origin','config','environment','capture','source-set','F01','foreign-selector','image']
rows=[];created=[]
for name in cases:
    root=allowed/('index-pathimpl-'+step+'-'+name);guard(root,allowed,True);root.mkdir();created.append(str(root))
    purpose=spec['F' if name in ('positive-F','F01','foreign-selector') else 'P']
    helpers=spec['helpers']; embedded=spec['embedded']; images=spec['images']; target='x86_64-pc-windows-msvc';profile='verification-scale'
    mkdir(root/'crates',root)
    source_names=set(helpers+embedded+['Cargo.toml','Cargo.lock','rust-toolchain.toml','apps/kilogram-cli/tests/support/migration_s54_p5.rs'])
    for n in sorted(source_names):raw(root/n,b'public fixture source only\n',root)
    head='97a5bfb9360a9c3a95dbb4b9f3d84e9b5540f54c'
    raw(root/'.git/HEAD',(head+'\n').encode(),root)
    image_rows=[dict(role=r['role'],pin=raw(root/r['path'],('public fixture image '+r['role']).encode(),root)) for r in images]
    mkdir(root/'.cargo',root);configs=[dict(path=str(root/'.cargo/config'),exists=False,sha256=None)]
    resolutions=[]
    for n in ['pwsh.exe','powershell.exe','git.exe','rustup.exe','cargo.exe','rustc.exe']:
        raw(root/'tools'/n,('public fixture '+n).encode(),root);resolutions.append(dict(name=n,path=str(root/'tools'/n)))
    sources=[pin(root/n) for n in sorted(source_names,key=str.casefold)]
    source_inputs=[dict(path=Path(r['path']).relative_to(root).as_posix(),sha256=r['sha256']) for r in sources if Path(r['path']).relative_to(root).as_posix() not in embedded and (Path(r['path']).relative_to(root).as_posix() not in helpers or Path(r['path']).relative_to(root).as_posix() in helpers[:3])]
    tools=[]
    for n in ['cargo','rustc']:
        p=pin(root/'tools'/(n+'.exe'));tools.append(dict(name=n,path=p['path'],sha256=p['sha256'],shim_path=p['path'],shim_sha256=p['sha256']))
    build=dict(format_version=1,profile=profile,target=target,packages=['kilogram-cli','kilogram-transport-iroh'],cargo_features=[],tools=tools,rustc=['rustc 1.98.0','release: 1.98.0','host: '+target],cargo=['cargo 1.98.0'],environment=spec['environment'],configs=configs,source_inputs=source_inputs,package_names=[dict(id='fixture:'+n,name=n) for n in required],resolved_graph=[dict(id='fixture:'+n,features=[],deps=[]) for n in required])
    def compiler(normal):
        roles=[images[1]] if normal else images[3:]
        entries=[(r['package'],r['kind'],r['target'],r) for r in roles]
        for n in required:
            if not any(e[0]==n for e in entries):entries.append((n,'lib',n,None))
        units=[];lines=[];artifacts=[];deps=[]
        for ix,(package,kind,name,role) in enumerate(entries):
            executable=None
            if role:
                executable=root/'target'/target/profile/(('normal' if normal else 'tests')+'-unit-'+str(ix)+'.exe')
                raw(executable,(root/role['path']).read_bytes(),root)
                dep=executable.with_suffix('.d')
                inputs=' '.join(embedded+['apps/kilogram-cli/tests/support/migration_s54_p5.rs'])
                deps.append(raw(dep,(str(dep)+': '+inputs+'\n').encode(),root))
                p=pin(root/role['path']);artifacts.append(dict(package=package,kind=kind,target=name,path=p['path'],bytes=p['bytes'],sha256=p['sha256'],profile=profile))
            filename=executable or root/'target'/target/profile/('unit-'+str(ix)+'.rlib')
            prof=dict(opt_level='3',debuginfo='line-tables-only',debug_assertions=True,overflow_checks=True,test=not normal)
            u=dict(package_id='fixture:'+package,package=package,kind=[kind],target=name,features=[],profile=prof,filenames=[str(filename)],executable=str(executable) if executable else None,**{'class':'runtime-target'},fresh=True)
            msg=dict(reason='compiler-artifact',package_id=u['package_id'],manifest_path=str(root/'Cargo.toml'),target=dict(kind=[kind],crate_types=[kind],name=name,src_path=str(root/'apps/kilogram-cli/tests/support/migration_s54_p5.rs'),edition='2024',doc=True,doctest=False,test=not normal),profile=prof,features=[],filenames=u['filenames'],executable=u['executable'],fresh=True)
            units.append(u);lines.append(json.dumps(msg,separators=(',',':')))
        cap=dict(classification='nonexecuting-normal-cli-compiler-provenance' if normal else 'nonexecuting-compiler-artifact-provenance',profile=profile,target=target,expected_manifest_sha256=None,build_inputs_before=build,build_inputs_after=build,units=units,raw_compiler_artifact_json=lines,stable_artifacts=artifacts,frozen_guard_verified=False)
        return cap,deps
    normal,_=compiler(True);tests,deps=compiler(False)
    evidence=root/'.tmp'/purpose['PublicNamespace']/purpose['Attempt'];mkdir(evidence,root)
    ident=[{k:v for k,v in u.items() if k!='fresh'} for u in tests['units']]
    frozen=dict(format_version=1,classification='frozen-scale-test-artifacts',profile=profile,target=target,stable_directory=str(root/'target/verification-scale/deps'),build_inputs=build,units_identity=ident,artifacts=tests['stable_artifacts'])
    epins=[pin(root/n) for n in embedded]
    comp=dict(normal=save(evidence/'normal.json',normal,root),tests=save(evidence/'tests.json',tests,root),frozen_manifest=save(evidence/'frozen.json',frozen,root),embedded=dict(before=epins,after=epins,dep_info=deps),normal_argv=['build','--package','kilogram-cli','--bin','kilogram-cli','--profile',profile,'--target',target,'--jobs','2','--offline','--locked','--message-format=json-render-diagnostics'],tests_argv=['test','--package','kilogram-cli','--package','kilogram-transport-iroh','--profile',profile,'--target',target,'--jobs','2','--offline','--locked','--no-run','--message-format=json-render-diagnostics'])
    run=root/'.tmp'/purpose['Namespace']/purpose['Attempt'];mkdir(run,root)
    isolated=save(evidence/'isolated.json',dict(format_version=1,tests=[dict(package='kilogram-cli',kind='test',target='migration_s5_process',name=purpose['Selector'])]),root)
    listing=raw(evidence/'list.txt',(purpose['Selector']+': test\n').encode(),root)
    identity=dict(case=purpose['Case'],attempt=purpose['Attempt'],selector=purpose['Selector'],run_id='0b'*32,workspace=str(root),run=str(run),output=str(run/'assertions.json'),activation=str(run/'activation.json'),launch=str(run/'launch.json'),capture=str(run/'capture.json'))
    inventory=dict(schema=1,classification=purpose['InventoryClass'],identity=identity,deadline=purpose['Deadline'],source=dict(s53_product='1f404b54ebce47ae3b52eaeb6855e30f3b8f3309',closure_publication='20418e41e4c486158fdc30b0c9f857750432e25e',implementation=head,observed_head=head,files=sources),compiler=comp,images=image_rows,runtime=dict(metadata_shell=pin(root/'tools/pwsh.exe'),memory_shell=pin(root/'tools/powershell.exe'),resolutions=resolutions,environment=spec['environment'],configs=configs,workers=spec['workers'],cargo_jobs=2,priority='BelowNormal',offline=True,locked=True,skip_cache=True,architecture='x64'),selection=dict(isolated=isolated,list=listing,harness_ref='stable-migration',named=1,native_markers=1,wrapper_markers=1,aggregates=0))
    inp=save(run/'inputs.json',inventory,root);now=int(time.time()*1000)-1000
    activation=dict(schema=1,classification=purpose['ActivationClass'],inventory=inp,case=purpose['Case'],attempt=purpose['Attempt'],run_id=identity['run_id'],run=str(run),selector=purpose['Selector'],implementation=head,execution_mode='NativeToken',deadline=purpose['Deadline'],not_before_utc_ms=now-1000,not_after_utc_ms=now+3600000,retry='root_reallocation_only')
    act=save(run/'activation.json',activation,root)
    capture=dict(version=1,attempt=purpose['Attempt'],owner_pid=100,launcher_pid=101,harness_path=str(root/images[5]['path']),whole_utc_ms=now+purpose['Deadline']['outer']*1000)
    cap=save(run/'capture.json',capture,root)
    launch=dict(schema=1,classification='s54_original_launch_binding',inventory=inp,activation=act,capture=cap,case=purpose['Case'],attempt=purpose['Attempt'],run_id=identity['run_id'],run=str(run),owner_pid=100,launcher_pid=101,start_tick_ms=5000,start_utc_ms=now,controller_tick_ms=5000+purpose['Deadline']['controller']*1000,work_tick_ms=5000+purpose['Deadline']['work']*1000,outer_tick_ms=5000+purpose['Deadline']['outer']*1000)
    ln=save(run/'launch.json',launch,root)
    index=b/(step+'.'+name+'.fixture-index.json')
    save(index,dict(classification='public_schema_fixture_only',root=str(root),inputs=dict(inventory=inp,activation=act,launch=ln,capture=cap)),b)
    rows.append(dict(name=name,index=str(index),root=str(root)))
save(b/(step+'.fixture-set.json'),dict(classification='public_compiler_index_fixture_set',cases=rows),b)
save(b/(step+'.construction.json'),dict(classification='fresh synthetic public constants only; no historical tree reads/copies',roots=created,historical_fixtures_read=False,historical_fixtures_written=False),b)
print('fresh_from_scratch_fixture_cases='+str(len(rows)))
