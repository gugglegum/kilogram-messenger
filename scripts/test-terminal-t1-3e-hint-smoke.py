"""One Auto-LAN/two-context admission, four texts, existing cleanup/restore. Validate by default."""
import argparse,json,runpy
from pathlib import Path
SCRIPTS=Path(__file__).resolve().parent
M=runpy.run_path(str(SCRIPTS/'test-terminal-t1-3e-admission-diagnostic.py'))
N=M['N'];D=M['D'];require=M['require'];save=M['save']
SOURCES=M['SOURCES']|{'scripts/test-terminal-t1-3e-hint-smoke.py','scripts/test-terminal-t1-3e-hint-smoke-check.py'}
PURPOSE='clock-independent-hint-smoke-v1'

class HintSmoke(M['Diagnostic']):
    def functional(self):
        # Existing actual Setup, fresh route selection, typed runtime observation,
        # original90s admission and separate failure-only diagnostic window.
        super().functional()
        self.monitors('A',self.end,watching=True)
        for slot in (0,1):
            for sender in (0,1):
                self.deliver(self.send(sender,slot,f'hint-{sender}-{slot}',self.end),self.end)
        self.histories('hint',self.end)
        events=[event for _,term in self.processes for event in term.events]
        require(len(self.messages)==4 and len(events)==4 and
            all(D['prior']['incoming_result'](events,row)['status']=='confirmed' for row in self.messages),'four exact text notifications')

    def run(self):
        passed=False
        try:
            super().run();passed=True
        finally:
            save(self.evidence/'hint-smoke-result.json',dict(schema=1,purpose=PURPOSE,passed=passed,
                messages=self.messages,expected_messages=4,offline_or_restart=False,
                cleanup_in_application_and_diagnostic_receipts=True))

def validate(plan,root):
    require(plan['purpose']==PURPOSE and plan['attempts']==['hint-a2-01'],'single scoped smoke')
    require(plan['connect_seconds']==85 and plan['admission_seconds']==90 and plan['probe_seconds']==60 and
        plan['minimum_call_seconds']==2 and plan['additional_network_seconds']==90 and
        plan['additional_network_mode']=='diagnostic_only_after_original_window' and
        plan['original_window_origin']=='before_first_connect_submit','existing startup budgets/origins')
    path=root/plan['build_manifest'];require(root in path.resolve().parents and N['digest'](path)==plan['build_manifest_sha256'],'build pin')
    build=json.loads(path.read_bytes())
    require(build['commit']=='a61385a00e12fbe0b78805fe7655afb5b3d6bcbe' and build['clean_published_worktree'] is True,'published A2 build')
    for row in build['inputs']:
        source=root/row['path'];require(root in source.resolve().parents and N['digest'](source)==row['sha256'],'frozen build input')
    N['validate'](plan,root,attempt_count=1,source_paths=SOURCES,product_pins=build['images'])

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('manifest',type=Path);parser.add_argument('--sha256',required=True)
    parser.add_argument('--run',action='store_true');parser.add_argument('--evidence',type=Path);args=parser.parse_args()
    require(N['digest'](args.manifest)==args.sha256,'reviewed smoke plan');plan=json.loads(args.manifest.read_bytes());root=SCRIPTS.parent;validate(plan,root)
    if args.run:
        require(args.evidence is not None and root in args.evidence.resolve().parents,'fresh smoke evidence')
        N['execute'](plan,root,args.evidence.resolve(),normal_factory=HintSmoke,result_name='single-hint-smoke-suite-result.json')
    else:print(json.dumps(dict(validated=True,attempts=plan['attempts'],messages=4,host_mutations=0)))

if __name__=='__main__':main()
