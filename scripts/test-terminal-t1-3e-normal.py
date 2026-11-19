"""Ordinary two-attempt messenger adapter. No task/firewall installation or fault gates."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
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
    return base64.b64encode(loader.encode('utf-16-le')).decode()

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

    def exchange(self,request):
        path=self.ledger/(request['id']+'.json')
        if not path.exists():save(path,request)
        else:require(json.loads(path.read_bytes())==request,'immutable request ledger')
        for retry in range(2):
            try:return super().exchange(request)
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
    def __init__(self,plan,attempt,evidence,agents):
        # Deliberately do not call the fault-suite constructor/run/phase/rollback.
        self.attempt=attempt;self.evidence=evidence;self.started=time.monotonic();self.end=self.started+1000
        self.phase_end=self.end;self.source=(SCRIPTS/'test-terminal-t1-3e-observe.ps1').read_bytes()
        self.source=b"[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal';\n"+self.source
        self.agents=agents;self.plan=plan;self.processes=[];self.receipts=set();self.messages=[];self.transfers=[]
        self.instances=[set(),set()];self.pair=[None,None];self.roles=[None,None];self.rollback_started=False
        ssh=D['pinned_ssh'](plan['ssh'],D['HOSTS'][0][2],D['HOSTS'][1][2])
        self.hosts=[Host(n['side'],dict(hostname=n['host'],ipv4=n['ip'],base=n['base'],udp_ephemeral=[49152,65535]),
            n['peer'],attempt,plan['pins'],ssh if i else [],evidence,self.end) for i,n in enumerate(plan['nodes'])]
        self.owner_tokens=[n['agent_token'] for n in plan['nodes']];self.generations=[None,None]

    def rollback(self):raise RuntimeError('fault-suite operation forbidden in ordinary adapter')
    def gate(self,*args,**kwargs):raise RuntimeError('fault gate forbidden in ordinary adapter')

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

def validate(plan,root):
    require(plan['v']==1 and len(plan['attempts'])==len(set(plan['attempts']))==2 and
        all(re.fullmatch(r'[a-z0-9][a-z0-9-]{0,30}',x) for x in plan['attempts']),'two fresh attempts')
    require(type(plan['work_seconds']) is int and 120<=plan['work_seconds']<=1000,'work budget')
    require(set(plan['pins'])==NAMES and set(plan['originals'])==NAMES,'six stable inputs')
    require(set(plan['sources'])==SOURCES,'complete reviewed source set')
    for section in ('sources','pins','originals'):
        require(all(re.fullmatch('[0-9a-f]{64}',x) for x in plan[section].values()),'exact content pins')
    for rel,pin in plan['sources'].items():
        path=root/rel;require('..' not in Path(rel).parts and not Path(rel).is_absolute() and digest(path)==pin,'source pin')
    for kind,pins in (('assets',plan['pins']),('original_assets',plan['originals'])):
        folder=root/plan[kind]
        require(root in folder.resolve().parents,'dedicated local assets')
        require({p.name for p in folder.iterdir()}==NAMES,'exact asset directory')
        for name,pin in pins.items():require(digest(folder/name)==pin,'asset pin')
    require(plan['pins']['kilogram-cli.exe']==D['PINS']['kilogram-cli.exe'],'reviewed product image')
    require(plan['pins']['kilogram-bootstrap.exe']==D['PINS']['kilogram-bootstrap.exe'],'reviewed bootstrap image')
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

def upload(agents,index,stage,folder,pins,node):
    # Exact byte budget + local watchdog; no read-until-EOF or orphaned writer.
    require(set(pins)==NAMES,'six stage inputs')
    for name,pin in pins.items():
        length=(folder/name).stat().st_size;text=upload_script(node,stage,name,pin,length)
        command=agents[index].prefix+['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',encoded_script(text)]
        require(len(command[-1])<7400,'stage command length')
        outcome=subprocess.run(command,input=(folder/name).read_bytes(),capture_output=True,timeout=100)
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

def execute(plan,root,evidence):
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
            stage=uuid.uuid4().hex;upload(agents,i,stage,root/plan['assets'],plan['pins'],n)
            changed.append(i)
            save(evidence/f'{n["side"]}-update-intent.json',dict(stage=stage,pins=plan['pins'],originals=plan['originals']))
            save(evidence/f'{n["side"]}-updated.json',agent.call('update',stage=stage,pins=plan['pins']))
            after=inspect_installed(agent,n);save(evidence/f'{n["side"]}-after-update.json',after)
            require(after['bins']==plan['pins'] and not after['readiness_error'],'updated inputs/readiness')
        for attempt in plan['attempts']:
            run=evidence/attempt;run.mkdir();Normal(plan,attempt,run,agents).run();attempts.append(attempt)
    except BaseException as error:primary=dict(type=type(error).__name__,message=str(error))
    finally:
        for i in changed:
            try:
                n=plan['nodes'][i];status=agents[i].call('status',attempt=plan['attempts'][0])
                require(not status['active'] and not status['blocked'],'restore requires confirmed idle')
                before=inspect_installed(agents[i],n)
                require(before['bins'] in (plan['pins'],plan['originals']),'restore refuses unexpected bin changes')
                stage=uuid.uuid4().hex;upload(agents,i,stage,root/plan['original_assets'],plan['originals'],n)
                result=agents[i].call('update',stage=stage,pins=plan['originals']);after=inspect_installed(agents[i],n)
                require(after['bins']==plan['originals'],'original bin restoration unconfirmed')
                restore.append(dict(side=n['side'],result=result,after=after))
            except Exception as error:restore.append(dict(side=i,error=str(error)))
        save(evidence/'two-attempt-result.json',dict(attempts=attempts,failure=primary,restoration=restore,
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
