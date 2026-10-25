// Explain precise source scopes for bounded regression reuse, not a new test run.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
const directory = path.resolve('ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence');
const parse = n => JSON.parse(fs.readFileSync(path.join(directory, n), 'utf8').replace(/^\uFEFF/, ''));
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const current = parse('review2-production-freeze.json').product_files;
const groups = [
  ['regressions-ordered-codec-final-1', 'crates/kilogram-protocol/'],
  ['regressions-wire-bound-final-1', 'crates/kilogram-protocol/'],
  ['regressions-ipc-final-1', 'crates/kilogram-runtime-ipc/'],
  ['regressions-state-s3-final-1', 'crates/kilogram-state/'],
  ['regressions-state-history-final-1', 'crates/kilogram-state/'],
  ['regressions-state-component-final-1', 'crates/kilogram-state/'],
  ['regressions-runtime-foundation-final-1', 'crates/kilogram-state/'],
];
const scope = groups.map(([run, prefix]) => {
  const old = new Map(parse(run + '-source.json').map(s => [s.path, s]));
  const runtimeSnapshotOnly = ['regressions-state-s3-final-1', 'regressions-state-history-final-1'].includes(run)
    ? ['crates/kilogram-state/src/vault/indexed_dialog/runtime2.rs'] : [];
  const files = current.filter(s => s.path.startsWith(prefix) && !runtimeSnapshotOnly.includes(s.path));
  const changes = files.filter(f => old.get(f.path)?.sha256 !== f.sha256 || hash(fs.readFileSync(f.path)) !== f.sha256);
  if (changes.length) throw Error('Unexpected component scope change: ' + run);
  return { run, scope_prefix: prefix, modified_product_files_identical: files,
    separately_revalidated_S4_snapshot_scope: runtimeSnapshotOnly.map(p => ({ path: p, old: old.get(p), current: current.find(s => s.path === p), basis: 'S3/history selected functions exercise ComponentRoot5/history APIs, not RuntimeRepository snapshot. Later S4-only single-read-tx snapshot change is tested on exact current source by runtime-foundation/component runs and complete F.' })) };
});
const legacy = ['protocol2_component_tests.rs', 'protocol2_failure_tests.rs', 'engine.rs'].map(n => 'apps/kilogram-cli/src/ordered_dialog/' + n);
const clean = legacy.map(p => ({ path: p, blob: execFileSync('git', ['hash-object', '--', p], { encoding: 'utf8' }).trim(), head_blob: execFileSync('git', ['rev-parse', 'HEAD:' + p], { encoding: 'utf8' }).trim() }));
if (clean.some(f => f.blob !== f.head_blob)) throw Error('Legacy S3 regression scope changed');
const runtime = 'apps/kilogram-cli/src/ordered_dialog/runtime2/runtime.rs';
const ipcHistory = p => {
  const s = fs.readFileSync(p, 'utf8').replace(/\r\n/g, '\n');
  const start = s.indexOf('fn ipc2(');
  const first = s.indexOf('\n        4 => {', start);
  const end = s.indexOf('\n        5 => {', first);
  if (start < 0 || first < 0 || end < 0) throw Error('History opcode scope');
  return s.slice(first, end);
};
const oldHistory = ipcHistory(path.join('.tmp/a3-s4/diagnostic-basis', runtime));
const newHistory = ipcHistory(runtime);
if (oldHistory !== newHistory) throw Error('History IPC response/cursor path changed');
fs.writeFileSync(path.join(directory, 'component-source-reuse.json'), JSON.stringify({
  groups: scope,
  CLI_S3_legacy_sources_unchanged_from_baseline: clean,
  escaped_history: { run: 'regressions-cli-history-byte-final-1', IPC_opcode4_normalized_sha256: hash(Buffer.from(newHistory)), opcode_identical_to_P4_basis: true, state_reader_scope_identical: true },
  later_changes: 'CLI numerical timing instrumentation, owner empty-turn scheduling removal and test-controller atomic persist repair. Nonempty guards are revalidated by current complete F; current full P validates actual data scheduling/history separately. None changes the reused protocol/state/IPC codecs or byte-budget/cursor branch.',
  limits: 'This correspondence explains reuse; source hashes alone do not claim whole-runtime acceptance. Heavy100k/S3 long probes were not rerun.',
}, null, 2) + '\n');
console.log('component_scope_reuse_verified=true history_opcode_identical=true legacy_S3_clean=true');
