// Reconstruct exactly three pre-repair sources and verify against frozen probe1 hashes.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
const directory = path.resolve('ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence');
const parse = n => JSON.parse(fs.readFileSync(path.join(directory, n), 'utf8').replace(/^\uFEFF/, ''));
const beforeMap = new Map(parse('diagnostic-costs-64-duplex-1-source.json').map(s => [s.path, s]));
const afterMap = new Map(parse('diagnostic-costs-64-duplex-2-source.json').map(s => [s.path, s]));
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const prefix = 'apps/kilogram-cli/src/ordered_dialog/runtime2/';
const names = [prefix + 'runtime.rs', prefix + 'timing.rs', 'crates/kilogram-transport-iroh/src/ordered2.rs'];
const files = [];
const patches = [];
for (const n of names) {
  const current = fs.readFileSync(n);
  if (hash(current) !== afterMap.get(n)?.sha256) throw Error('Probe2 source drift: ' + n);
  const newline = current.includes(Buffer.from('\r\n')) ? '\r\n' : '\n';
  let old = current.toString().replace(/\r\n/g, '\n');
  if (n.endsWith('/runtime.rs')) {
    const comment = '                // Empty turns use poison-checked authenticated root/ledger counters.\n                // Nonempty work still loads its installed snapshot and checks phase,\n                // current policy/time/provider/lease before the closed commit gate.\n';
    const block = `                let installed = {
                    let _timing = timing::span(timing::Metric::OwnerInstalled);
                    owner.repository.installed(c)
                };
                match installed {
                    Ok(true) => {}
                    Ok(false) => {
                        timing::idle_sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => {
                        eprintln!("od2_context_read_blocked context={c} {error}");
                        retry_at[usize::from(c)] = Instant::now() + Duration::from_secs(1);
                        continue;
                    }
                }
`;
    if (old.split(comment).length !== 2) throw Error('Owner repair boundary');
    old = old.replace(comment, block);
  } else if (n.endsWith('/timing.rs')) {
    old = old.replace('    OwnerJob,\n', '    OwnerJob,\n    OwnerInstalled,\n');
  } else {
    const test = old.indexOf('\n#[cfg(test)]\nmod route_tests {');
    if (test < 0) throw Error('R1 test boundary');
    old = old.slice(0, test);
    const after = '            ensure!(\n                bind.ip().is_loopback(),\n                "S4 IP transports require an explicit loopback bind"\n            );';
    const before = '            if self.policy == RoutePolicy::DirectOnly || self.relay.is_none() {\n                ensure!(\n                    bind.ip().is_loopback(),\n                    "S4 direct-only requires an explicit loopback bind"\n                );\n            }';
    if (old.split(after).length !== 2) throw Error('R1 conditional boundary');
    old = old.replace(after, before);
    old = old.replace('\n\npub fn binding(', '\npub fn binding(');
  }
  const bytes = Buffer.from(old.replace(/\n/g, newline));
  if (hash(bytes) !== beforeMap.get(n)?.sha256) throw Error('Reconstruction differs from frozen probe1 source: ' + n);
  const target = path.join('.tmp/a3-s4/diagnostic-before-source', n);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, bytes);
  const diff = spawnSync('git', ['diff', '--no-index', '--', target, n], { encoding: 'utf8' });
  if (![0, 1].includes(diff.status)) throw Error(diff.stderr);
  patches.push(diff.stdout.replaceAll('a/.tmp/a3-s4/diagnostic-before-source/', 'a/'));
  files.push({ path: n, before: beforeMap.get(n), after: afterMap.get(n) });
}
const changed = [...afterMap.values()].filter(s => s.path.endsWith('.rs') && beforeMap.get(s.path)?.sha256 !== s.sha256).map(s => s.path).sort();
if (JSON.stringify(changed) !== JSON.stringify([...names].sort())) throw Error('Unexpected Rust changes between comparable probes');
const patchName = 'owner-root-counter-r1.patch';
const patch = Buffer.from(patches.join('\n'));
fs.writeFileSync(path.join(directory, patchName), patch);
fs.writeFileSync(path.join(directory, 'owner-root-counter-r1-diff.json'), JSON.stringify({
  scope: 'Exact probe1-to-probe2 delta only. No change to policy/provider/time/lease/closed writer, native batching, polling, profile, caps or protocol records.',
  controller_byte_identical: beforeMap.get('apps/kilogram-cli/tests/runtime2_process/diagnostic.rs')?.sha256 === afterMap.get('apps/kilogram-cli/tests/runtime2_process/diagnostic.rs')?.sha256,
  changed_rust_paths: changed, files,
  patch: { path: patchName, bytes: patch.length, sha256: hash(patch) },
}, null, 2) + '\n');
console.log(JSON.stringify({ paths: changed, patch_bytes: patch.length, patch_sha256: hash(patch) }));
