"""Focused watchdog-pin regression: generated PS5, disposable files and real owned Jobs only."""
import ast,base64,hashlib,json,runpy,subprocess,sys,time,unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
PS=Path('C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe')
BASELINE='5714080abbcfae199beab3907417bc97eb624737'
OUT=Path(sys.argv[1]).resolve()
if OUT.exists() or ROOT not in OUT.parents:raise RuntimeError('fresh workspace evidence required')
OUT.mkdir(parents=True)
N=runpy.run_path(str(ROOT/'scripts/test-terminal-t1-3e-normal.py'))
def sha(data):return hashlib.sha256(data).hexdigest()
def save(path,value):path.write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
source=(ROOT/'scripts/migration-s5-l1-deadline.ps1').read_bytes();SOURCE_PIN=sha(source)
original=subprocess.run(['git','show',BASELINE+':scripts/test-terminal-t1-3e-normal.py'],capture_output=True,check=True,timeout=15).stdout
(OUT/'original-normal.py').write_bytes(original)
# Unmodified original generator bodies with their original mutual dependency.
tree=ast.parse(original);names={'upload_script','sftp_stage_script','sftp_client_script'}
old=dict(N);exec(compile(ast.Module(body=[x for x in tree.body if isinstance(x,ast.FunctionDef) and x.name in names],type_ignores=[]),'<published-original>','exec'),old)
save(OUT/'inputs.json',dict(baseline=BASELINE,original_source_sha256=sha(original),current_source_sha256=sha((ROOT/'scripts/test-terminal-t1-3e-normal.py').read_bytes()),source_watchdog_sha256=SOURCE_PIN))

def invoke(label,script,data=None):
    folder=OUT/label;folder.mkdir();(folder/'generated.ps1').write_text(script,encoding='utf-8')
    command=[str(PS),'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',N['encoded_script'](script)]
    start=time.monotonic()
    result=subprocess.run(command,input=data,capture_output=True,timeout=30,
        creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    (folder/'stdout.bin').write_bytes(result.stdout);(folder/'stderr.bin').write_bytes(result.stderr)
    save(folder/'result.json',dict(exit=result.returncode,elapsed=time.monotonic()-start,timeout=False))
    return result

class Checks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        r=invoke('identity',"@{host=[Environment]::MachineName;sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value;administrator=([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);ps=$PSVersionTable.PSVersion.Major}|ConvertTo-Json -Compress")
        if r.returncode:raise RuntimeError(r.stderr)
        cls.identity=json.loads(r.stdout)
        if cls.identity['administrator'] or cls.identity['ps']!=5:raise RuntimeError('ordinary PS5 required')

    def fixture(self,label,body):
        # Keep the native receiver path within the production encoded-command cap.
        folder=ROOT/'.tmp'/('wg-'+hashlib.md5((str(OUT)+label).encode()).hexdigest()[:12]);(folder/'bin').mkdir(parents=True,exist_ok=False)
        (folder/'bin/deadline.ps1').write_bytes(body)
        save(OUT/(label+'.json'),dict(disposable_path=str(folder),installed_watchdog_sha256=sha(body)))
        return dict(host=self.identity['host'],sid=self.identity['sid'],base=str(folder)),sha(body)

    def assert_refused(self,result,message):
        self.assertEqual(result.returncode,1,result.stderr);self.assertIn(message.encode(),result.stderr)

    def test_actual_host_command_lengths_initial_restore_and_already_original(self):
        p=json.loads((ROOT/'.tmp/terminal-t1-3e/agent-refresh-metadata-activation-01/field/runtime-plan.json').read_bytes())
        rows=[]
        for node in p['nodes']:
            for phase,current,destination,assets in [('initial',p['originals'],p['pins'],p['assets']),
                ('restore',p['pins'],p['originals'],p['original_assets']),('already-original',p['originals'],p['originals'],p['original_assets'])]:
                sizes={name:(ROOT/assets/name).stat().st_size for name in destination}
                for verify in (False,True):
                    script=N['sftp_stage_script'](node,'f'*32,destination,sizes,verify,receiver_deadline_pin=current['deadline.ps1'])
                    rows.append(dict(host=node['side'],phase=phase,verify=verify,length=len(N['encoded_script'](script))))
                for name,pin in destination.items():
                    script=N['upload_script'](node,'f'*32,name,pin,sizes[name],receiver_deadline_pin=current['deadline.ps1'])
                    self.assertLess(len(N['encoded_script'](script)),7400)
        save(OUT/'actual-host-command-lengths.json',rows)

    def test_local_source_pin_original_fails_corrected_runs_and_wrong_refuses_before_child(self):
        for case in ('original','corrected','stale','destination'):
            logs=OUT/('client-'+case);logs.mkdir();marker=logs/'child-ran'
            child="[IO.File]::WriteAllText('"+str(marker)+"','ran');[Environment]::Exit(0)"
            command=[str(PS),'-NoProfile','-NonInteractive','-EncodedCommand',base64.b64encode(child.encode('utf-16-le')).decode()]
            pin={'corrected':SOURCE_PIN,'stale':'4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5','destination':sha(b'destination payload')}.get(case)
            fn=old['sftp_client_script'] if case=='original' else N['sftp_client_script']
            script=fn(command,sha(PS.read_bytes()),logs,8,10,**({} if case=='original' else dict(source_deadline_pin=pin)))
            r=invoke('wrapper-'+case,script)
            if case=='corrected':
                self.assertEqual(r.returncode,0,r.stderr);self.assertTrue(marker.is_file())
                receipt=json.loads((logs/'sftp-client.json').read_bytes())
                self.assertEqual(receipt['exit'],0);self.assertIsNone(receipt['failure']);self.assertTrue(receipt['empty_job'])
                self.assertGreater(receipt['child_pid'],0);self.assertGreater(receipt['child_started'],0)
            else:
                self.assert_refused(r,'Local SFTP watchdog pin');self.assertFalse(marker.exists());self.assertFalse((logs/'sftp-client.json').exists())

    def test_receiver_initial_restore_and_metadata_pins(self):
        # Historical guard for initial deployment; a comment-only candidate fixture makes
        # source, CURRENT installed, and incoming payload pins all distinct on restore.
        historical=ROOT/'.tmp/terminal-t1-3e/offline-recipient-prep-01/package/originals/deadline.ps1'
        bodies={'initial':historical.read_bytes(),'restore':b'# disposable installed candidate guard\r\n'+source}
        destination=b'bounded incoming bytes';destination_pin=sha(destination)
        for phase,body in bodies.items():
            node,installed=self.fixture('receiver-'+phase,body)
            self.assertEqual(len({SOURCE_PIN,installed,destination_pin}),3)
            for case in ('original','corrected','wrong-destination','wrong-source'):
                stage=hashlib.md5((phase+case).encode()).hexdigest()
                chosen=destination_pin if case=='wrong-destination' else SOURCE_PIN if case=='wrong-source' else installed
                fn=old['upload_script'] if case=='original' else N['upload_script']
                script=fn(node,stage,'host.ps1',destination_pin,len(destination),**({} if case=='original' else dict(receiver_deadline_pin=chosen)))
                r=invoke(phase+'-bytes-'+case,script,destination)
                path=Path(node['base'])/'staging'/stage/'host.ps1'
                passed=case=='corrected' or (case=='original' and phase=='initial')
                if passed:self.assertEqual(r.returncode,0,r.stderr);self.assertEqual(path.read_bytes(),destination)
                else:
                    self.assert_refused(r,'Stage watchdog pin');self.assertFalse(path.exists());self.assertFalse(Path(str(path)+'.upload-started.json').exists())
            data={name:('destination-'+name).encode() for name in N['NAMES']}
            pins={name:sha(value) for name,value in data.items()};sizes={name:len(value) for name,value in data.items()}
            # Both generated prepare and verify execute with real guard/file IO.
            for case in ('original','corrected','wrong-destination','wrong-source'):
                stage=hashlib.md5((phase+'metadata'+case).encode()).hexdigest()
                chosen=pins['deadline.ps1'] if case=='wrong-destination' else SOURCE_PIN if case=='wrong-source' else installed
                fn=old['sftp_stage_script'] if case=='original' else N['sftp_stage_script']
                kwargs={} if case=='original' else dict(receiver_deadline_pin=chosen)
                r=invoke(phase+'-prepare-'+case,fn(node,stage,pins,sizes,**kwargs))
                path=Path(node['base'])/'staging'/stage
                passed=case=='corrected' or (case=='original' and phase=='initial')
                if passed:self.assertEqual(r.returncode,0,r.stderr)
                else:
                    self.assert_refused(r,'SFTP metadata watchdog pin');self.assertFalse((path/'upload-intent.json').exists())
                    # Isolate verify's gate with an otherwise exact, prepared stage.
                    (path/'upload-intent.json').write_text(json.dumps(dict(stage=stage,pins=pins,sizes=sizes)))
                for name,value in data.items():(path/name).write_bytes(value)
                before={p.name:sha(p.read_bytes()) for p in path.iterdir()}
                r=invoke(phase+'-verify-'+case,fn(node,stage,pins,sizes,True,**kwargs))
                if passed:self.assertEqual(r.returncode,0,r.stderr);self.assertTrue(json.loads(r.stdout)['verified'])
                else:self.assert_refused(r,'SFTP metadata watchdog pin')
                self.assertEqual(before,{p.name:sha(p.read_bytes()) for p in path.iterdir()})

if __name__=='__main__':
    result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(Checks))
    save(OUT/'checks.json',dict(tests=result.testsRun,failures=len(result.failures),errors=len(result.errors),ok=result.wasSuccessful(),installed_mutations=0,live_sftp=False,real_PS5=True,real_guard=True))
    sys.exit(0 if result.wasSuccessful() else 1)
