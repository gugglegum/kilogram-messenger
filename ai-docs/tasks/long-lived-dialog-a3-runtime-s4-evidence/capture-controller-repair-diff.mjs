// Exact controller-only delta from preserved failed full F source hashes.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync, execFileSync } from 'node:child_process';
const directory = path.resolve('ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence');
const parse = n => JSON.parse(fs.readFileSync(path.join(directory, n), 'utf8').replace(/^\uFEFF/, ''));
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const baseline = new Map(parse('f-full-review2-frozen-1-source.json').map(s => [s.path, s]));
const p = 'apps/kilogram-cli/tests/runtime2_process.rs';
const raw = fs.readFileSync(p);
const newline = raw.includes(Buffer.from('\r\n')) ? '\r\n' : '\n';
let old = raw.toString().replace(/\r\n/g, '\n');
old = old.replace('#[path = "runtime2_process/schedule_control.rs"]\nmod schedule_control;\n', '');
const start = old.indexOf('    fn schedule(&self, value: serde_json::Value) -> Result<()> {');
const end = old.indexOf('    fn route(', start);
if (start < 0 || end < 0) throw Error('Controller helper boundaries');
old = old.slice(0, start) + `    fn schedule(&self, value: serde_json::Value) -> Result<()> {
        use std::io::Write;
        let mut file =
            tempfile::NamedTempFile::new_in(self.schedule.parent().context("schedule parent")?)?;
        file.write_all(&serde_json::to_vec(&value)?)?;
        file.as_file().sync_all()?;
        file.persist(&self.schedule).map_err(|e| e.error)?;
        Ok(())
    }
` + old.slice(end);
const before = Buffer.from(old.replace(/\n/g, newline));
if (hash(before) !== baseline.get(p)?.sha256) throw Error('Controller reconstruction differs from frozen full F source');
const target = path.join('.tmp/a3-s4/controller-before-review3', p);
fs.mkdirSync(path.dirname(target), { recursive: true });
fs.writeFileSync(target, before);
const diff = spawnSync('git', ['diff', '--no-index', '--', target, p], { encoding: 'utf8' });
if (![0, 1].includes(diff.status)) throw Error(diff.stderr);
const added = 'apps/kilogram-cli/tests/runtime2_process/schedule_control.rs';
const addedBytes = fs.readFileSync(added);
const lines = addedBytes.toString().replace(/\r\n/g, '\n').trimEnd().split('\n');
const blob = execFileSync('git', ['hash-object', '--', added], { encoding: 'utf8' }).trim();
const patch = Buffer.from(diff.stdout.replaceAll('a/.tmp/a3-s4/controller-before-review3/', 'a/') + `\ndiff --git a/${added} b/${added}\nnew file mode 100644\nindex 0000000..${blob.slice(0, 7)}\n--- /dev/null\n+++ b/${added}\n@@ -0,0 +1,${lines.length} @@\n` + lines.map(s => '+' + s).join('\n') + '\n');
const frozen = parse('review2-production-freeze.json');
for (const f of [...frozen.product_files, ...frozen.images]) if (hash(fs.readFileSync(f.path)) !== f.sha256) throw Error('Frozen product/image drift: ' + f.path);
fs.writeFileSync(path.join(directory, 'controller-schedule-repair.patch'), patch);
fs.writeFileSync(path.join(directory, 'controller-schedule-repair-diff.json'), JSON.stringify({
  baseline: 'f-full-review2-frozen-1-source.json',
  changed_controller_paths: [
    { path: p, before_sha256: hash(before), after_sha256: hash(raw) },
    { path: added, before_sha256: null, after_sha256: hash(addedBytes), git_blob: blob },
  ],
  patch: { path: 'controller-schedule-repair.patch', bytes: patch.length, sha256: hash(patch) },
  production_files_identical: frozen.product_files.length,
  production_images_identical: frozen.images,
  retry_budget_ms: 500, raw_windows_codes_only: [5, 32, 33], same_temp_and_bytes: true,
  destination_delete_truncate_attribute_ACL_or_non_atomic_fallback: false,
  deterministic_release: 'Private observer after real failed persist verifies old bytes and drops actual deny-delete handle; ordinary write uses no-op observer.',
  historical_os5_cause_still_unclassified: true,
}, null, 2) + '\n');
console.log(JSON.stringify({ patch_bytes: patch.length, patch_sha256: hash(patch), production_sources: frozen.product_files.length, image: frozen.images[0] }));
