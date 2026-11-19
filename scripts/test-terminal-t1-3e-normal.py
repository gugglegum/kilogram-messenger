"""Ordinary two-attempt messenger adapter. No task/firewall installation or fault gates."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path, PureWindowsPath
import re
import runpy
import subprocess
import time
import uuid

SCRIPTS=Path(__file__).resolve().parent
D=runpy.run_path(str(SCRIPTS/'test-terminal-t1-3e-driver.py'))
C=runpy.run_path(str(SCRIPTS/'test-terminal-t1-3e-lab-control.py'))
require,save,parallel,until=D['require'],D['save'],D['parallel'],D['until']
NAMES={'host.ps1','deadline.ps1','mailbox.cs','setup-pipe.cs','kilogram-cli.exe','kilogram-bootstrap.exe'}
SOURCES={'scripts/'+name for name in ('test-terminal-t1-3e-normal.py','test-terminal-t1-3e-normal-check.py',
    'test-terminal-t1-3e-driver.py','test-terminal-t1-3e-interactive-field.py','test-terminal-t1-3e-lab-control.py',
    'test-terminal-t1-2e-driver.py','test-terminal-t1-2e-mailbox.py','test-terminal-t1-1f-driver.py',
    'test-terminal-t1-1f-host.ps1','test-terminal-t1-3e-observe.ps1','migration-s5-l1-deadline.ps1',
    'test-terminal-t1-2e-setup-pipe.cs','test-terminal-t1-2e-mailbox.cs')}

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def encoded_script(text):
    compressed=base64.b64encode(gzip.compress(text.encode('utf-8'),mtime=0)).decode()
    loader=("$ErrorActionPreference='Stop';try{$m=[IO.MemoryStream]::new([Convert]::FromBase64String('"+compressed+"'));"+
        "$z=[IO.Compression.GZipStream]::new($m,[IO.Compression.CompressionMode]::Decompress);"+
        "$r=[IO.StreamReader]::new($z,[Text.Encoding]::UTF8);& ([scriptblock]::Create($r.ReadToEnd()))}"+
        "catch{[Console]::Error.WriteLine($_.Exception.Message);[Environment]::Exit(1)};[Environment]::Exit(0)")
    encoded=base64.b64encode(loader.encode('utf-16-le')).decode()
    require(len(encoded)<7400,'bounded encoded script')
    return encoded

def upload_capture(command,logs,label,timeout,**kwargs):
    try:return subprocess.run(command,capture_output=True,timeout=timeout,**kwargs)
    except subprocess.TimeoutExpired as error:
        (logs/(label+'.timeout.stdout.bin')).write_bytes(error.stdout or b'')
        (logs/(label+'.timeout.stderr.bin')).write_bytes(error.stderr or b'')
        save(logs/(label+'.timeout.json'),dict(timed_out=True,seconds=timeout,reconciliation_required=True))
        raise

def deadline(clock,identity,work_seconds,sent,received,reserve=60):
    require(all(clock[k]==identity[k] for k in ('attempt','generation','session','worker_pid','worker_started')), 'local deadline identity')
    require(all(type(clock[k]) is int for k in ('start_tick','tick','work_tick','whole_tick')) and
        0<clock['start_tick']<=clock['tick']<clock['work_tick']<clock['whole_tick'] and
        clock['work_tick']-clock['start_tick']==1000*work_seconds and
        clock['whole_tick']-clock['start_tick']==1000*(work_seconds+15), 'local attempt clock')
    left=(clock['work_tick']-clock['tick'])/1000-(received-sent)-reserve
    require(left>0,'local cleanup reserve exhausted')
    return received+left

class Agent(C['Agent']):
    def __init__(self,*args,ledger,**kwargs):
        super().__init__(*args,**kwargs);self.ledger=ledger;ledger.mkdir()

    def exchange(self,request,*,deadline=None):
        path=self.ledger/(request['id']+'.json')
        if not path.exists():save(path,request)
        else:require(json.loads(path.read_bytes())==request,'immutable request ledger')
        for retry in range(2):
            try:return super().exchange(request) if deadline is None else super().exchange(request,deadline=deadline)
            except (OSError,subprocess.TimeoutExpired):
                if retry:raise
            except RuntimeError as error:
                if not str(error).startswith('lab transport outcome unconfirmed') or retry:raise
        raise RuntimeError('unconfirmed agent exchange')

class Host(D['Host']):
    def call(self,op,*args,**kwargs):
        if op=='init':op='lab-init';kwargs.update(self.lab_identity)
        return super().call(op,*args,**kwargs)

    def clock(self):return self.call('lab-clock',**self.lab_identity)

class Normal(D['Field']):
    def __init__(self,plan,attempt,evidence,agents,*,host_class=None):
        # Deliberately do not call the fault-suite constructor/run/phase/rollback.
        self.attempt=attempt;self.evidence=evidence;self.started=time.monotonic();self.end=self.started+1000
        self.phase_end=self.end
        header,newline,body=(SCRIPTS/'test-terminal-t1-3e-observe.ps1').read_bytes().partition(b'\n')
        require(header.startswith(b'param(') and newline,'observer parameter header')
        self.source=header+newline+b"[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';\n"+body
        self.agents=agents;self.plan=plan;self.processes=[];self.receipts=set();self.messages=[];self.transfers=[]
        self.instances=[set(),set()];self.pair=[None,None];self.roles=[None,None];self.rollback_started=False
        ssh=D['pinned_ssh'](plan['ssh'],D['HOSTS'][0][2],D['HOSTS'][1][2])
        make_host=host_class or Host
        self.hosts=[make_host(n['side'],dict(hostname=n['host'],ipv4=n['ip'],base=n['base'],udp_ephemeral=[49152,65535]),
            n['peer'],attempt,plan['pins'],ssh if i else [],evidence,self.end) for i,n in enumerate(plan['nodes'])]
        self.owner_tokens=[n['agent_token'] for n in plan['nodes']];self.generations=[None,None]

    def rollback(self):raise RuntimeError('fault-suite operation forbidden in ordinary adapter')
    def gate(self,*args,**kwargs):raise RuntimeError('fault gate forbidden in ordinary adapter')

    def observe(self,index,op,end,**params):
        require(op in ('identity','network','ports','preconnect','status'),'fault observer operation forbidden')
        host=self.hosts[index];request=dict(op=op,side=host.name,host=host.config['hostname'],attempt=self.attempt,**params)
        args,data=D['observer_command'](self.source,request)
        logs=self.evidence/'observer';logs.mkdir(parents=True,exist_ok=True);label=f'{host.name}-{op}-{uuid.uuid4().hex}'
        command=host.ps+args;seconds=self.observation_timeout(end)
        save(logs/(label+'.intent.json'),dict(request=request,command=command,timeout_seconds=seconds,source_sha256=hashlib.sha256(self.source).hexdigest()))
        (logs/(label+'.stdin.bin')).write_bytes(data)
        try:result=subprocess.run(command,input=data,capture_output=True,timeout=seconds)
        except subprocess.TimeoutExpired as error:
            (logs/(label+'.stdout.bin')).write_bytes(error.stdout or b'');(logs/(label+'.stderr.bin')).write_bytes(error.stderr or b'')
            save(logs/(label+'.result.json'),dict(timed_out=True,reconciliation_required=True));raise
        (logs/(label+'.stdout.bin')).write_bytes(result.stdout);(logs/(label+'.stderr.bin')).write_bytes(result.stderr)
        save(logs/(label+'.result.json'),dict(exit=result.returncode,timed_out=False))
        stage=re.search(rb'\bT13E_(?:STATUS2|RUNTIME)_[A-Z_]+\b',result.stderr)
        suffix='/'+stage.group().decode() if stage else ''
        require(result.returncode==0 and not result.stderr and 0<len(result.stdout)<=128000,
            f'{host.name}/{op}{suffix}: observer failed ({result.returncode}); inspect observer receipts')
        return json.loads(result.stdout.decode('utf-8-sig'))

    def observation_timeout(self,end):
        return D['prior']['prior']['remaining'](end,12)

    def start(self,index,tag,mode,end=None):
        term=D['MailboxTerminal'](self.hosts[index],tag,mode,end or self.phase_end,
            interactive=C['Attempt'](self.agents[index],self.attempt,self.generations[index]),
            **({'observe_socket':True} if mode=='chat' else {}))
        self.processes.append((index,term))
        ready=term.line(term.deadline)
        require(ready and ready.get('v')==1 and ready.get('type')=='ready','terminal readiness')
        if mode=='chat':
            require(ready=={'v':1,'type':'ready'},'Chat readiness shape')
            require(term.call('status')=={'connected':False},'Chat starts disconnected')
            proof=self.observe(index,'preconnect',term.deadline,tag=tag)
            require(proof['disconnected'] and not proof['descriptor'] and not proof['sockets'],'Chat preconnect proof')
            save(self.evidence/f'{self.hosts[index].name}-{tag}-prestart.json',proof)
        return term

    def sample(self,index,term,end):
        host=self.hosts[index];host.socket(term.tag,end)
        job=json.loads((self.evidence/f'{host.name}-{term.tag}-socket.json').read_bytes())
        args=dict(sha256=self.plan['pins']['kilogram-cli.exe'],runtime_pid=job['pid'],runtime_started=job['started'],runtime_image=job['image'])
        if index:args.update(owner_token=self.owner_tokens[index],owner_session=self.plan['nodes'][index]['windows_session'])
        doc=self.observe(index,'status',end,**args)
        if not doc['ready']:return None
        point=D['sample_endpoint'](index,term,doc,job,self.owner_tokens[index])
        require([r['role'] for r in doc['slots']]==[self.roles[index]]*2,'consent roles')
        return (doc,point) if point else None

    def connect(self,index,warm):
        term=self.pair[index];value=term.call('connect',mode='start')
        require(value['mode']=='owner' and value['contexts']==self.contexts and
            D['HEX'].fullmatch(value['runtime_instance']) and value['runtime_instance'] not in self.instances[index],'fresh runtime/preserved contexts')
        term.contexts=self.contexts;term.instance=value['runtime_instance'];term.epoch=None
        self.instances[index].add(term.instance)
        return until(lambda end:self.sample(index,term,end),25,self.end,'owned publication/socket readiness')

    def admit(self,label,started):
        def current(end):
            rows=parallel(lambda i:self.sample(i,self.pair[i],end),(0,1))
            if not all(rows):return None
            docs,points=map(list,zip(*rows))
            return points if D['live_pair'](docs,points) else None
        points=until(current,90,min(self.end,started+90),'four current LAN leases')
        self.monitors(label,self.end,watching=True)
        save(self.evidence/f'{label}-admitted.json',dict(seconds=time.monotonic()-started,endpoints=points))
        for i,point in enumerate(points):self.export(i,label,point,self.end)
        return points

    def monitors(self,label,end,*,watching,held=False):
        require(not held,'ordinary suite has no network hold')
        def observe(limit):
            rows=parallel(lambda i:self.monitor(i,limit,watching=watching,warm=label in ('B','C')),(0,1))
            return rows if all(rows) else None
        return until(observe,30,end,'both ordinary receivers watching')

    def restart(self,indices,label):
        previous=[self.sample(i,self.pair[i],self.end)[1] for i in indices]
        parallel(lambda i:self.stop(i,self.pair[i]),indices)
        for i,point in zip(indices,previous):self.port_gone(i,point['port'],label+'-closed',self.end)
        for i,term in zip(indices,parallel(lambda i:self.start(i,f'owner-{label.lower()}-1','chat'),indices)):self.pair[i]=term

    def functional(self):
        self.contexts=D['prior']['onboard'](self.hosts,lambda h:self.start(int(h.name=='B'),'setup','setup'),
            self.evidence,self.transfers,self.select_route)
        for i,term in self.processes:self.receipt(i,term)
        self.pair=parallel(lambda i:self.start(i,'owner-a-1','chat'),(0,1))
        started=time.monotonic();parallel(lambda i:self.connect(i,False),(0,1));self.admit('A',started)
        for slot in (0,1):self.deliver(self.send(slot,slot,f'a-{slot}',self.end),self.end,retry=True)
        self.histories('A',self.end)
        # Real application offline, no UDP Hold/Release or temporary blocks.
        old=self.sample(1,self.pair[1],self.end)[1];self.stop(1,self.pair[1]);self.port_gone(1,old['port'],'B-offline',self.end)
        offline=self.send(0,1,'offline',self.end);self.check_pending(offline,self.end,'offline',receiver_present=False)
        self.pair[1]=self.start(1,'owner-b-1','chat')
        started=time.monotonic();self.connect(1,True);self.admit('B',started)
        self.deliver(offline,self.end,retry=True)
        for slot in (0,1):self.deliver(self.send(1-slot,slot,f'b-{slot}',self.end),self.end,retry=True)
        self.histories('B',self.end)
        self.restart((0,1),'C');started=time.monotonic()
        parallel(lambda i:self.connect(i,True),(0,1));self.admit('C',started)
        for row in self.messages:
            repeated=self.pair[row['sender']].call('retry',self.end,**row['key'])
            require(repeated['state']=='delivered' and repeated['event_id']==row['event_id'],'restart replay stable event ID')
        self.histories('C-replay',self.end)
        require(not any(term.events for term in self.pair),'history replay produced duplicate notifications')
        for slot in (0,1):self.deliver(self.send(slot,slot,f'c-{slot}',self.end),self.end,retry=True)
        self.histories('C',self.end)
        parallel(lambda i:self.stop(i,self.pair[i]),(0,1))
        events=[e for _,term in self.processes for e in term.events]
        require(len(events)==len(self.messages) and all(D['prior']['incoming_result'](events,row)['status']=='confirmed' for row in self.messages),
            'missing/duplicate notifications across generations')

    def run(self):
        begun=[];primary=None;cleanup=[];results={}
        try:
            # Read-only binding/port admission; no administrative Audit or gate.
            self.tokens=parallel(lambda i:self.preflight(i),(0,1))
            require(all(self.tokens[i]['user']==self.plan['nodes'][i]['sid'] for i in (0,1)),'ordinary controller SID')
            for i,agent in enumerate(self.agents):
                ready=agent.ready();n=self.plan['nodes'][i]
                require((ready['host'],ready['sid'],ready['session'],ready['pid'],ready['started'])==
                    (n['host'],n['sid'],n['session'],n['pid'],n['started']),'reviewed agent incarnation/context')
                self.generations[i]=uuid.uuid4().hex
                # Preserve intent before exchange: an unknown begin still needs reconciliation.
                begun.append(i)
                worker=agent.call('begin',attempt=self.attempt,generation=self.generations[i],work_seconds=self.plan['work_seconds'],
                    pins=self.plan['pins'],baseline_sha256=n['baseline_sha256'])
                self.hosts[i].lab_identity=dict(agent_session=agent.session,agent_generation=self.generations[i],
                    worker_pid=worker['worker_pid'],worker_started=worker['worker_start'])
            bounds=[]
            for i,host in enumerate(self.hosts):
                until(lambda _:self.agents[i].call('status',attempt=self.attempt)['started'],12,self.end,'worker start')
                sent=time.monotonic();clock=host.clock();received=time.monotonic()
                identity=dict(attempt=self.attempt,generation=self.generations[i],session=self.agents[i].session,
                    worker_pid=host.lab_identity['worker_pid'],worker_started=host.lab_identity['worker_started'])
                bounds.append(deadline(clock,identity,self.plan['work_seconds'],sent,received))
                save(self.evidence/f'{host.name}-deadline.json',dict(clock=clock,sent=sent,received=received,controller_end=bounds[-1]))
            self.end=self.phase_end=min(bounds)
            for host in self.hosts:host.deadline=self.end
            self.functional()
        except BaseException as error:primary=dict(type=type(error).__name__,message=str(error))
        finally:
            for i,term in self.processes:
                if (i,term.tag) in self.receipts:continue
                try:term.close();self.receipt(i,term,cleanup=True)
                except Exception as error:cleanup.append(dict(side=i,tag=term.tag,error=str(error)))
            for i in begun:
                try:
                    status=self.agents[i].call('status',attempt=self.attempt)
                    if status['active']==self.attempt:
                        self.agents[i].call('cancel' if primary or cleanup else 'end',attempt=self.attempt,generation=self.generations[i])
                    result=self.agents[i].wait(self.attempt,45);results[self.hosts[i].name]=result
                    require(result['cleanup']['empty_job'] and not result['cleanup']['error'],'native attempt cleanup')
                    require(bool(primary) or result['state']=='closed' and not result['cleanup']['forced'],'normal attempt closed')
                except Exception as error:cleanup.append(dict(side=i,error=str(error)))
            save(self.evidence/'result.json',dict(attempt=self.attempt,failure=primary,cleanup=cleanup,agents=results,
                messages=self.messages,transfers=self.transfers,elapsed=time.monotonic()-self.started))
        require(not primary and not cleanup,'normal attempt failed; inspect primary and cleanup receipts')

def validate(plan,root,*,attempt_count=2,source_paths=None,product_pins=None):
    require(plan['transfer']['exe']==str(D['prior']['prior']['SYSTEM']/'OpenSSH/sftp.exe') and
        digest(Path(plan['transfer']['exe']))==plan['transfer']['sha256'],'pinned local SFTP client')
    require(plan['v']==1 and len(plan['attempts'])==len(set(plan['attempts']))==attempt_count and
        all(re.fullmatch(r'[a-z0-9][a-z0-9-]{0,30}',x) for x in plan['attempts']),'two fresh attempts')
    require(type(plan['work_seconds']) is int and 120<=plan['work_seconds']<=1000,'work budget')
    require(set(plan['pins'])==NAMES and set(plan['originals'])==NAMES,'six stable inputs')
    require(set(plan['sources'])==(source_paths if source_paths is not None else SOURCES),'complete reviewed source set')
    for section in ('sources','pins','originals'):
        require(all(re.fullmatch('[0-9a-f]{64}',x) for x in plan[section].values()),'exact content pins')
    for rel,pin in plan['sources'].items():
        path=root/rel;require('..' not in Path(rel).parts and not Path(rel).is_absolute() and digest(path)==pin,'source pin')
    for kind,pins in (('assets',plan['pins']),('original_assets',plan['originals'])):
        folder=root/plan[kind]
        require(root in folder.resolve().parents,'dedicated local assets')
        require({p.name for p in folder.iterdir()}==NAMES,'exact asset directory')
        for name,pin in pins.items():require(digest(folder/name)==pin,'asset pin')
    images=product_pins if product_pins is not None else D['PINS']
    require(plan['pins']['kilogram-cli.exe']==images['kilogram-cli.exe'],'reviewed product image')
    require(plan['pins']['kilogram-bootstrap.exe']==images['kilogram-bootstrap.exe'],'reviewed bootstrap image')
    for name,source in {'host.ps1':'test-terminal-t1-1f-host.ps1','deadline.ps1':'migration-s5-l1-deadline.ps1',
            'mailbox.cs':'test-terminal-t1-2e-mailbox.cs','setup-pipe.cs':'test-terminal-t1-2e-setup-pipe.cs'}.items():
        require(plan['pins'][name]==plan['sources']['scripts/'+source],'packaged helper/source binding')
    require(digest(Path(plan['ssh']['known_hosts']))==plan['known_hosts_sha256'] and plan['ssh']['fingerprint']==D['PIN'],'SSH host pin')
    require(len(plan['nodes'])==2,'two nodes')
    for i,n in enumerate(plan['nodes']):
        require((n['side'],n['host'],n['ip'],n['base'],n['peer'])==(D['HOSTS'][i][0],D['HOSTS'][i][1],D['HOSTS'][i][2],D['BASES'][i],D['HOSTS'][1-i][2]),'fixed host binding')
        require(len(n['payload_pins'])==15 and all(re.fullmatch('[a-z0-9][a-z0-9.-]+',key) and re.fullmatch('[0-9a-f]{64}',value)
            for key,value in n['payload_pins'].items()) and n['payload_pins']['lab.json']==n['config_sha'] and
            n['payload_pins']['test-terminal-t1-3e-lab-agent.ps1']==n['script_sha'],'installed version pins')
        require(n['agent_token']['user']==n['sid'] and n['agent_token']['administrator'] is False and
            0<len(n['agent_token']['groups'])<=128 and len(set(n['agent_token']['groups']))==len(n['agent_token']['groups']), 'observed ordinary agent token')
        observation=Path(n['token_observation'])
        require(not observation.is_absolute() and '..' not in observation.parts and
            root in (root/observation).resolve().parents and digest(root/observation)==n['token_observation_sha256'],'token observation pin/path')
        proof=json.loads((root/observation).read_bytes());native=proof['native'];raw=proof['raw']
        require(n['agent_token_view']=='WindowsIdentity.Groups' and
            all(row['pid']==n['pid'] and row['started']==n['started'] and row['sid']==n['sid'] and row['administrator'] is False for row in (native,raw)) and
            native['windows_session']==n['windows_session'] and raw['raw_token_groups_match'] is True and
            n['agent_token']['groups']==native['actual_groups'] and
            n['agent_raw_token']['user']==n['sid'] and n['agent_raw_token']['administrator'] is False and
            set(n['agent_raw_token']['groups'])=={row['sid'] for row in raw['raw_groups']},'same-incarnation token views')

def upload_script(node,stage,name,pin,length):
    require(re.fullmatch('[0-9a-f]{32}',stage) and name in NAMES and re.fullmatch('[0-9a-f]{64}',pin) and
        type(length) is int and 0<length<=128*1024*1024,'fixed bounded stage input')
    text=r'''$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";
if([Environment]::MachineName -cne '@HOST@' -or [Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne '@SID@' -or
 ([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Stage identity'}
$parent='@BASE@\staging';$check=Get-Item -LiteralPath '@BASE@';while($check){if($check.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Stage base reparse'};$check=$check.Parent}
if(Test-Path -LiteralPath $parent){if((Get-Item -LiteralPath $parent).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Stage parent reparse'}}
$dir=Join-Path $parent '@STAGE@';if(Test-Path -LiteralPath $dir){if((Get-Item -LiteralPath $dir).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Stage reparse'}};
[void][IO.Directory]::CreateDirectory($dir);
$check=Get-Item -LiteralPath $dir;while($check){if($check.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Stage reparse'};$check=$check.Parent}
$deadline=Join-Path '@BASE@' 'bin/deadline.ps1';
if((Get-FileHash -LiteralPath $deadline).Hash -ine '4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5'){throw 'Stage watchdog pin'}
. ([scriptblock]::Create([IO.File]::ReadAllText($deadline)));$guard=[KilogramS5Deadline]::new(80,90);
try{
 $path=Join-Path $dir '@NAME@';$started=(Get-Process -Id $PID).StartTime.ToFileTimeUtc();
 $record=@{pid=$PID;started=$started;name='@NAME@';bytes=@LENGTH@;sha256='@PIN@';work_seconds=80;whole_seconds=90}
 $meta=[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $record -Compress));
 $proof=[IO.File]::Open(($path+'.upload-started.json'),'CreateNew','Write','None');try{$proof.Write($meta,0,$meta.Length);$proof.Flush($true)}finally{$proof.Dispose()}
 $out=[IO.File]::Open($path,'CreateNew','Write','None');$inputStream=[Console]::OpenStandardInput();
 try{
  $buffer=New-Object byte[] 65536;$remaining=[long]@LENGTH@;
  while($remaining -gt 0){$guard.CheckWork();$n=$inputStream.Read($buffer,0,[int][Math]::Min($buffer.Length,$remaining));if($n -le 0){throw 'Short stage input'};$out.Write($buffer,0,$n);$remaining-=$n}
  $out.Flush($true)
 }finally{$out.Dispose()}
 if((Get-FileHash -LiteralPath $path).Hash -ine '@PIN@'){throw 'Stage pin'}
}finally{$guard.Finish()}
[Console]::Out.WriteLine((ConvertTo-Json -InputObject $record -Compress));[Console]::Out.Flush();
# Do not depend on SSH/ConsoleHost propagating stdin EOF after the exact byte budget.
[Environment]::Exit(0)
'''
    for key,value in dict(HOST=node['host'],SID=node['sid'],BASE=node['base'],STAGE=stage,NAME=name,PIN=pin,LENGTH=str(length)).items():text=text.replace('@'+key+'@',value)
    return text

def sftp_command(prefix,client,batch):
    require(prefix[-1]=='powershell.exe' and Path(prefix[0]).name.lower()=='ssh.exe' and prefix.count('-T')==1,
        'reviewed SSH prefix for SFTP')
    # Retain every identity, known-host, BindAddress and forwarding/timeout option.
    return [client,'-q','-b',str(batch)]+[x for x in prefix[1:-2] if x!='-T']+[prefix[-2]]

def sftp_stage_script(node,stage,pins,sizes,verify=False):
    require(set(pins)==set(sizes)==NAMES and all(type(n) is int and 0<n<=128*1024*1024 for n in sizes.values()),'stage manifest')
    prefix=upload_script(node,stage,'host.ps1',pins['host.ps1'],sizes['host.ps1']).split('$deadline=',1)[0]
    manifest=dict(stage=stage,pins=pins,sizes=sizes)
    # No bulk bytes enter PowerShell stdin. Preparation and verification are metadata-only.
    if not verify:
        prefix=prefix.replace("[void][IO.Directory]::CreateDirectory($dir);","if(Test-Path -LiteralPath $dir){throw 'Fresh SFTP stage required'};[void][IO.Directory]::CreateDirectory($dir);")
    code=prefix+"\n$manifest='"+json.dumps(manifest,separators=(',',':'))+"'|ConvertFrom-Json;\n"
    code+="$deadline=Join-Path '"+node['base']+"' 'bin/deadline.ps1';if((Get-FileHash -LiteralPath $deadline).Hash -ine '4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5'){throw 'SFTP metadata watchdog pin'};. ([scriptblock]::Create([IO.File]::ReadAllText($deadline)));$guard=[KilogramS5Deadline]::new(80,90);try{\n"
    if verify:code+="$stop=[DateTime]::UtcNow.AddSeconds(10);while(@(Get-Process -Name sftp-server -ErrorAction SilentlyContinue).Count -and [DateTime]::UtcNow -lt $stop){$guard.CheckWork();Start-Sleep -Milliseconds 100};\n"
    code+="if(@(Get-Process -Name sftp-server -ErrorAction SilentlyContinue).Count){throw 'SFTP subsystem still present; preserve stage'};\n"
    if not verify:
        code+="$bytes=[Text.Encoding]::UTF8.GetBytes(($manifest|ConvertTo-Json -Depth 6 -Compress));$file=[IO.File]::Open((Join-Path $dir 'upload-intent.json'),'CreateNew','Write','None');try{$file.Write($bytes,0,$bytes.Length);$file.Flush($true)}finally{$file.Dispose()};\n"
    else:
        code+=r'''
$actual=@(Get-ChildItem -LiteralPath $dir -Force);$expected=@('upload-intent.json')+@($manifest.pins.PSObject.Properties.Name)
if($actual.Count -ne $expected.Count -or @($actual|Where-Object {$_.PSIsContainer -or $_.Name -cnotin $expected -or ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)}).Count){throw 'SFTP stage file set/type'}
$intent=Get-Content -LiteralPath (Join-Path $dir 'upload-intent.json') -Raw|ConvertFrom-Json
if($intent.stage -cne $manifest.stage){throw 'SFTP intent stage'}
foreach($entry in $manifest.pins.PSObject.Properties){
 $guard.CheckWork()
 $name=$entry.Name;$path=Join-Path $dir $name
 if($intent.pins.$name -cne $entry.Value -or $intent.sizes.$name -ne $manifest.sizes.$name -or
    (Get-Item -LiteralPath $path).Length -ne $manifest.sizes.$name -or
    (Get-FileHash -LiteralPath $path).Hash -ine $entry.Value){throw 'SFTP stage content pin'}
}
'''
    return code+"\n$guard.CheckWork();@{stage=$manifest.stage;pins=$manifest.pins;sizes=$manifest.sizes;server_process_absent=$true;verified="+('$true' if verify else '$false')+"}|ConvertTo-Json -Depth 6 -Compress;[Console]::Out.Flush()}finally{$guard.Finish()};[Environment]::Exit(0)"

def sftp_client_script(command,client_pin,logs,work=160,whole=180):
    require(type(work) is int and type(whole) is int and 0<work<whole<=180,'SFTP deadline')
    literal=lambda text:"'"+str(text).replace("'","''")+"'"
    code=r'''$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";
if((Get-FileHash -LiteralPath @CLIENT@).Hash -ine '@PIN@'){throw 'Local SFTP client pin'}
$deadline=@DEADLINE@;if((Get-FileHash -LiteralPath $deadline).Hash -ine '4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5'){throw 'Local SFTP watchdog pin'}
. ([scriptblock]::Create([IO.File]::ReadAllText($deadline)));$guard=[KilogramS5Deadline]::new(@WORK@,@WHOLE@)
$child=$null;$primary=$null;$exit=$null;$empty=$false;$childPid=$null;$childStart=$null;$out=$null;$err=$null
try{
 $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName=@CLIENT@;$si.Arguments=@ARGS@;$si.UseShellExecute=$false;$si.CreateNoWindow=$true
 $si.RedirectStandardInput=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
 $child=[Diagnostics.Process]::Start($si);$guard.VerifyChild($child);$childPid=$child.Id;$childStart=$child.StartTime.ToFileTimeUtc();$child.PriorityClass='BelowNormal';$child.StandardInput.Close()
 $out=$child.StandardOutput.ReadToEndAsync();$err=$child.StandardError.ReadToEndAsync()
 while(!$child.WaitForExit(100)){$guard.CheckWork()};$guard.CheckWork();$exit=$child.ExitCode
 [IO.File]::WriteAllText(@STDOUT@,$out.GetAwaiter().GetResult(),[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText(@STDERR@,$err.GetAwaiter().GetResult(),[Text.UTF8Encoding]::new($false))
 if($exit -ne 0){throw 'SFTP client nonzero exit'}
}catch{$primary=$_.Exception.Message}
finally{
 try{$guard.CleanupChildren(5000);$empty=($guard.Children().Length -eq 0)}catch{$primary='SFTP owned cleanup: '+$_.Exception.Message}
 if($null -ne $child){if($child.HasExited){$exit=$child.ExitCode};$child.Dispose()}
 if($null -ne $out){[IO.File]::WriteAllText(@STDOUT@,$out.GetAwaiter().GetResult(),[Text.UTF8Encoding]::new($false))}
 if($null -ne $err){[IO.File]::WriteAllText(@STDERR@,$err.GetAwaiter().GetResult(),[Text.UTF8Encoding]::new($false))}
 $receipt=@{exit=$exit;failure=$primary;empty_job=$empty;child_pid=$childPid;child_started=$childStart;work_seconds=@WORK@;whole_seconds=@WHOLE@}
 [IO.File]::WriteAllText(@RECEIPT@,($receipt|ConvertTo-Json -Compress),[Text.UTF8Encoding]::new($false));$guard.Finish()
}
if($primary){throw $primary};$receipt|ConvertTo-Json -Compress
'''
    values=dict(CLIENT=literal(command[0]),PIN=client_pin,DEADLINE=literal(SCRIPTS/'migration-s5-l1-deadline.ps1'),
        ARGS=literal(subprocess.list2cmdline(command[1:])),STDOUT=literal(logs/'sftp.stdout.log'),
        STDERR=literal(logs/'sftp.stderr.log'),RECEIPT=literal(logs/'sftp-client.json'),WORK=str(work),WHOLE=str(whole))
    for key,value in values.items():code=code.replace('@'+key+'@',value)
    return code

def upload(agents,index,stage,folder,pins,node,*,evidence,transfer):
    logs=evidence/stage;logs.mkdir(parents=True,exist_ok=False)
    if index:
        sizes={name:(folder/name).stat().st_size for name in pins}
        def metadata(verify):
            label='verify' if verify else 'prepare'
            cmd=agents[index].prefix+['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',encoded_script(sftp_stage_script(node,stage,pins,sizes,verify))]
            outcome=upload_capture(cmd,logs,label,100,stdin=subprocess.DEVNULL)
            (logs/(label+'.stdout.bin')).write_bytes(outcome.stdout);(logs/(label+'.stderr.bin')).write_bytes(outcome.stderr)
            save(logs/(label+'.json'),dict(exit=outcome.returncode))
            require(not outcome.returncode and not outcome.stderr,'SFTP metadata unconfirmed; preserve stage')
            receipt=json.loads(outcome.stdout.decode('utf-8-sig'))
            require(receipt==dict(stage=stage,pins=pins,sizes=sizes,server_process_absent=True,verified=verify),'SFTP stage receipt')
        metadata(False)
        remote='/'+PureWindowsPath(node['base']).as_posix()+'/staging/'+stage
        quote=lambda value:'"'+str(value).replace('\\','/')+'"'
        require(all(not any(c in str(x) for c in ('"','\r','\n')) for x in [remote,folder,*pins]),'SFTP batch path')
        batch=logs/'upload.batch';batch.write_text('cd '+quote(remote)+'\n'+''.join('put '+quote(folder/name)+' '+quote(name)+'\n' for name in pins)+'bye\n',encoding='utf-8',newline='\n')
        cmd=sftp_command(agents[index].prefix,transfer['exe'],batch)
        save(logs/'client-intent.json',dict(command=cmd,client_sha256=transfer['sha256'],local_work_seconds=160,local_whole_seconds=180,transport_seconds=200))
        script=sftp_client_script(cmd,transfer['sha256'],logs)
        wrapper=[str(D['prior']['prior']['SYSTEM']/'WindowsPowerShell/v1.0/powershell.exe'),'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',encoded_script(script)]
        outcome=upload_capture(wrapper,logs,'wrapper',200,stdin=subprocess.DEVNULL)
        (logs/'wrapper.stdout.bin').write_bytes(outcome.stdout);(logs/'wrapper.stderr.bin').write_bytes(outcome.stderr)
        save(logs/'wrapper.json',dict(exit=outcome.returncode))
        try:client_receipt=json.loads((logs/'sftp-client.json').read_bytes())
        except (OSError,ValueError) as error:
            client_receipt=None;save(logs/'client-receipt-error.json',dict(error=str(error)))
        # Preserve client failure as primary and receiver/content diagnostics separately.
        verification_error=None
        try:metadata(True)
        except Exception as error:verification_error=str(error)
        # Windows PowerShell can emit the CLIXML preamble with no serialized records.
        # Accept only the exact observed empty framing; retain every raw stderr byte.
        stderr_records=outcome.stderr not in (b'',b'#< CLIXML\r\n')
        save(logs/'upload-result.json',dict(client_exit=outcome.returncode,client_stderr=bool(outcome.stderr),
            client_stderr_records=stderr_records,verification_error=verification_error))
        require(not outcome.returncode and not stderr_records and client_receipt and client_receipt['empty_job'] and
            client_receipt['exit']==0 and client_receipt['failure'] is None,'SFTP client failed; inspect upload receipts')
        require(verification_error is None,'SFTP receiver/content verification failed; inspect upload receipts')
        return
    # Exact byte budget + local watchdog; no read-until-EOF or orphaned writer.
    require(set(pins)==NAMES,'six stage inputs')
    for name,pin in pins.items():
        length=(folder/name).stat().st_size;text=upload_script(node,stage,name,pin,length)
        command=agents[index].prefix+['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',encoded_script(text)]
        require(len(command[-1])<7400,'stage command length')
        with (folder/name).open('rb') as stream:outcome=upload_capture(command,logs,name,100,stdin=stream)
        (logs/(name+'.stdout.bin')).write_bytes(outcome.stdout);(logs/(name+'.stderr.bin')).write_bytes(outcome.stderr)
        save(logs/(name+'.json'),dict(exit=outcome.returncode,length=length,sha256=pin))
        require(not outcome.returncode and not outcome.stderr,'staging copy unconfirmed; preserve stage')
        receipt=json.loads(outcome.stdout.decode('utf-8-sig'))
        require(receipt['name']==name and receipt['bytes']==length and receipt['sha256']==pin,'stage receipt')

def inspect_installed(agent,node):
    # A passive check, not Agent.check/Task.Run or another DPAPI probe.
    text=r'''$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";
$q=[Console]::ReadLine()|ConvertFrom-Json;$base=$q.base;
if([Environment]::MachineName -cne $q.host -or [Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne $q.sid -or
 ([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Inspection identity'}
$homeDir=Join-Path $base 'ordinary-agent';foreach($e in $q.payload_pins.PSObject.Properties){
 if((Get-FileHash -LiteralPath (Join-Path $homeDir $e.Name)).Hash -ine $e.Value){throw 'Installed payload changed'}}
. (Join-Path $homeDir 'test-terminal-t1-3e-lab-install.ps1') -Mode Library
$checkCfg=Get-LabConfig $q.side;$checkError=$null
try{$null=Assert-LabReadOnly $checkCfg $q.baseline_sha256}catch{$checkError=$_.Exception.Message}
$ready=Lab-Read (Join-Path $homeDir 'control/ready.json');$state=Lab-Read (Join-Path $homeDir 'control/state.json')
if($ready.session -cne $q.session -or $ready.pid -ne $q.pid -or $ready.started -ne $q.started -or $state.session -cne $q.session){throw 'Agent session changed'}
$bins=@{};foreach($name in @('host.ps1','deadline.ps1','mailbox.cs','setup-pipe.cs','kilogram-cli.exe','kilogram-bootstrap.exe')){$bins[$name]=Lab-Hash (Join-Path $base ('bin/'+$name))}
@{readiness_error=$checkError;bins=$bins;active=$state.active;blocked=$state.blocked;session=$ready.session;read_only=$true}|ConvertTo-Json -Depth 6 -Compress
'''
    argv=agent.prefix+['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',base64.b64encode(text.encode('utf-16-le')).decode()]
    outcome=subprocess.run(argv,input=json.dumps(node).encode()+b'\n',capture_output=True,timeout=40)
    require(not outcome.returncode and not outcome.stderr,'passive installed inspection unconfirmed')
    return json.loads(outcome.stdout.decode('utf-8-sig'))

def execute(plan,root,evidence,*,normal_factory=None,result_name='two-attempt-result.json'):
    evidence.mkdir(parents=True,exist_ok=False)
    ssh=D['pinned_ssh'](plan['ssh'],D['HOSTS'][0][2],D['HOSTS'][1][2])
    agents=[Agent(([str(D['prior']['prior']['SYSTEM']/'WindowsPowerShell/v1.0/powershell.exe')] if not i else ssh+['powershell.exe']),
        n['base'],n['script_sha'],n['config_sha'],ledger=evidence/(n['side']+'-requests')) for i,n in enumerate(plan['nodes'])]
    changed=[];attempts=[];restore=[];primary=None
    try:
        for i,agent in enumerate(agents):
            ready=agent.ready();n=plan['nodes'][i]
            require((ready['session'],ready['pid'],ready['started'])==(n['session'],n['pid'],n['started']),'fresh preparation required after agent restart')
            before=inspect_installed(agent,n);save(evidence/f'{n["side"]}-before-update.json',before)
            require(not before['active'] and not before['blocked'] and not before['readiness_error'] and before['bins']==plan['originals'],'reviewed originals and installed readiness before update')
            stage=uuid.uuid4().hex;upload(agents,i,stage,root/plan['assets'],plan['pins'],n,evidence=evidence/'uploads',transfer=plan['transfer'])
            changed.append(i)
            save(evidence/f'{n["side"]}-update-intent.json',dict(stage=stage,pins=plan['pins'],originals=plan['originals']))
            save(evidence/f'{n["side"]}-updated.json',agent.call('update',stage=stage,pins=plan['pins']))
            after=inspect_installed(agent,n);save(evidence/f'{n["side"]}-after-update.json',after)
            require(after['bins']==plan['pins'] and not after['readiness_error'],'updated inputs/readiness')
        for attempt in plan['attempts']:
            run=evidence/attempt;run.mkdir();(normal_factory or Normal)(plan,attempt,run,agents).run();attempts.append(attempt)
    except BaseException as error:primary=dict(type=type(error).__name__,message=str(error))
    finally:
        for i in changed:
            try:
                n=plan['nodes'][i];status=agents[i].call('status',attempt=plan['attempts'][0])
                require(not status['active'] and not status['blocked'],'restore requires confirmed idle')
                before=inspect_installed(agents[i],n)
                require(before['bins'] in (plan['pins'],plan['originals']),'restore refuses unexpected bin changes')
                stage=uuid.uuid4().hex;upload(agents,i,stage,root/plan['original_assets'],plan['originals'],n,evidence=evidence/'uploads',transfer=plan['transfer'])
                result=agents[i].call('update',stage=stage,pins=plan['originals']);after=inspect_installed(agents[i],n)
                require(after['bins']==plan['originals'],'original bin restoration unconfirmed')
                restore.append(dict(side=n['side'],result=result,after=after))
            except Exception as error:restore.append(dict(side=i,error=str(error)))
        save(evidence/result_name,dict(attempts=attempts,failure=primary,restoration=restore,
            no_task_or_profile_mutation=True))
    require(not primary and attempts==plan['attempts'] and len(restore)==2 and
        all('error' not in x and not x['after']['readiness_error'] for x in restore),'two attempts/restoration/readiness unconfirmed')

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('manifest',type=Path);p.add_argument('--sha256',required=True)
    p.add_argument('--run',action='store_true');p.add_argument('--evidence',type=Path);args=p.parse_args()
    require(digest(args.manifest)==args.sha256,'reviewed runtime plan pin')
    plan=json.loads(args.manifest.read_bytes());root=SCRIPTS.parent;validate(plan,root)
    if args.run:
        require(args.evidence is not None and root in args.evidence.resolve().parents,'dedicated evidence')
        execute(plan,root,args.evidence.resolve())
    else:print(json.dumps(dict(validated=True,attempts=plan['attempts'],host_mutations=0)))

if __name__=='__main__':main()
