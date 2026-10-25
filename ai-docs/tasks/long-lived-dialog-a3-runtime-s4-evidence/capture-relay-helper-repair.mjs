// Exact review4 helper/controller delta; no P fixture vault is opened.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
const directory = path.resolve('ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence');
const parse = n => JSON.parse(fs.readFileSync(path.join(directory, n), 'utf8').replace(/^\uFEFF/, ''));
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const baseline = new Map(parse('p-full-review2-exception-1-source.json').map(s => [s.path, s]));
const controller = 'apps/kilogram-cli/tests/runtime2_process/positive.rs';
const current = fs.readFileSync(controller);
const newline = current.includes(Buffer.from('\r\n')) ? '\r\n' : '\n';
let old = current.toString().replace(/\r\n/g, '\n');
const smoke = old.indexOf('#[tokio::test]\nasync fn a3_s4_relay_smoke_exact_b_only_after_rebind');
if (smoke < 0) throw Error('Smoke scope boundary');
old = old.slice(0, smoke).trimEnd() + '\n';
const first = old.indexOf('struct Relay {');
const end = old.indexOf('impl Drop for Relay {');
if (first < 0 || end < 0) throw Error('Relay controller boundaries');
const oldRelay = `struct Relay {
    child: OwnedChild,
    stop: PathBuf,
    url: String,
}
impl Relay {
    async fn start(base: &Path) -> Result<Self> {
        let ready = base.join("relay.ready");
        let stop = base.join("relay.stop");
        let mut command = Command::new(
            repo().join("target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe"),
        );
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_4000);
        }
        let mut child = OwnedChild(
            command
                .arg(&ready)
                .arg(&stop)
                .stdout(Stdio::null())
                .stderr(Stdio::from(std::fs::File::create(
                    base.join("relay.stderr.log"),
                )?))
                .spawn()?,
        );
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                anyhow::bail!("relay exited {status}");
            }
            if ready.exists() {
                let url = std::fs::read_to_string(&ready)?;
                println!("operator_relay pid={} url={url}", child.id());
                return Ok(Self { child, stop, url });
            }
            ensure!(
                start.elapsed() < Duration::from_secs(15),
                "relay registration"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}
`;
old = old.slice(0, first) + oldRelay + old.slice(end);
old = old.replace('        let mut relay=Relay::start(&pair.base).await?;', '        let relay=Relay::start(&pair.base).await?;');
old = old.replace('        relay.start_node(&mut pair.nodes[0]).await?;relay.start_node(&mut pair.nodes[1]).await?;', '        pair.nodes[0].start().await?;pair.nodes[1].start().await?;');
old = old.replace('        relay.check_alive("before final RelayOnly restart")?;\n        let restart=pair.restarted(false).await;\n        if let Err(error)=restart{return Err(error).context(format!("final RelayOnly restart: {}",relay_stderr(&relay.stderr)));}\n        relay.check_alive("after final RelayOnly restart")?;converged(&pair,0,1578).await?;', '        pair.restarted(false).await?;converged(&pair,0,1578).await?;');
const before = Buffer.from(old.replace(/\n/g, newline));
if (hash(before) !== baseline.get(controller)?.sha256) throw Error('Relay controller baseline reconstruction is not exact');
const oldController = path.join('.tmp/a3-s4/controller-before-review4', controller);
fs.mkdirSync(path.dirname(oldController), { recursive: true });
fs.writeFileSync(oldController, before);
const helper = 'crates/kilogram-transport-iroh/examples/s4_loopback_relay.rs';
const oldHelper = '.tmp/a3-s4/relay-helper-review4-original/s4_loopback_relay.rs';
if (hash(fs.readFileSync(oldHelper)) !== baseline.get(helper)?.sha256) throw Error('Old helper source mismatch');
const patchParts = [[oldController, controller], [oldHelper, helper]].map(([a, b]) => {
  const r = spawnSync('git', ['diff', '--no-index', '--', a, b], { encoding: 'utf8' });
  if (![0, 1].includes(r.status)) throw Error(r.stderr);
  return r.stdout.replaceAll('a/' + a.replaceAll('\\', '/'), 'a/' + b);
});
const patch = Buffer.from(patchParts.join('\n'));
const frozen = parse('review2-production-freeze.json');
const changed = frozen.product_files.filter(s => hash(fs.readFileSync(s.path)) !== s.sha256);
if (changed.length !== 1 || changed[0].path !== helper) throw Error('Unexpected frozen application source change');
for (const s of frozen.images.filter(s => !s.path.endsWith('kilogram-relay.exe'))) if (hash(fs.readFileSync(s.path)) !== s.sha256) throw Error('Frozen application image drift');
const slice = (s, a, b) => { const first = s.indexOf(a), end = s.indexOf(b, first); if (first < 0 || end < 0) throw Error('F helper boundaries'); return s.slice(first, end); };
const now = current.toString().replace(/\r\n/g, '\n');
for (const [a, b] of [['pub(super) async fn hit(', 'async fn send_range('], ['pub(super) fn selected(', 'async fn exact_history(']]) if (slice(now, a, b) !== slice(old, a, b)) throw Error('Exercised F helper path changed');
const image = parse('review4-relay-image.json');
fs.writeFileSync(path.join(directory, 'review4-relay-helper.patch'), patch);
fs.writeFileSync(path.join(directory, 'review4-relay-source-correspondence.json'), JSON.stringify({
  helper_source: { path: helper, old_sha256: baseline.get(helper)?.sha256, new_sha256: hash(fs.readFileSync(helper)) },
  helper_image: image,
  controller: { path: controller, old_sha256: hash(before), new_sha256: hash(current) },
  patch: { path: 'review4-relay-helper.patch', bytes: patch.length, sha256: hash(patch) },
  application_source_files_unchanged: 41, mechanical_frozen_set_note: 'The original42-file classifier included this standalone helper example. Exactly that explicitly authorized helper path differs; all41 application/transport/crypto/state sources stay frozen.',
  application_CLI_predecessor_L1_images_identical: true,
  exercised_F_paths_unchanged: 'Only positive.rs Relay/P-start diagnostics/new smoke and standalone example differ. F process/actor/state/route functions and shared positive::hit/selected are unchanged. No full F repetition requested by review4.',
  runtime_transport_scenario_timeouts_counts_gates_unchanged: true,
}, null, 2) + '\n');
console.log(JSON.stringify({ patch_bytes: patch.length, patch_sha256: hash(patch), application_sources_unchanged: 41, new_helper_sha256: image.sha256 }));
