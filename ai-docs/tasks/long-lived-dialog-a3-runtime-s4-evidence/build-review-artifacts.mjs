// Read-only repository/fixture inspection; writes derived review JSON in this directory.
// Run from repository root with Node after all selected runners have finished.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(directory, '../../..');
process.chdir(repo);
const relative = p => path.relative(repo, p).split(path.sep).join('/');
const read = p => fs.readFileSync(p, 'utf8').replace(/^\uFEFF/, '');
const json = p => JSON.parse(read(p));
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8' }).trim();
const save = (name, value) => fs.writeFileSync(path.join(directory, name), JSON.stringify(value, null, 2) + '\n');
const evidence = name => path.join(directory, name);
const artifact = p => {
  const b = fs.readFileSync(p);
  return { path: relative(p), bytes: b.length, sha256: hash(b) };
};
const unique = xs => [...new Set(xs)].sort();

const pNames = ['p-full-review4-relay-repair-1'];
const fNames = ['f-owner-eligibility-r1-guards-1', 'controller-schedule-lock-controls-1', 'f09-controller-repair-1', 'f-full-review3-frozen-1'];
const componentNames = [
  'regressions-ordered-codec-final-1',
  'regressions-ipc-final-1',
  'regressions-state-s3-final-1',
  'regressions-state-history-final-1',
  'regressions-state-component-final-1',
  'regressions-runtime-foundation-final-1',
  'regressions-wire-bound-final-1',
  'regressions-cli-s3-failure-final-1',
  'regressions-cli-s3-authority-final-1',
  'regressions-cli-history-byte-final-1',
];
const contributing = new Map([
  ...pNames.map(n => [n, 'P']),
  ...fNames.map(n => [n, 'F']),
  ...componentNames.map(n => [n, 'components']),
]);
const caveats = {
  'p-full-final-4-contract-budget': 'FAIL at controller2350s; relay/final-history/3288 gates did not execute. Immutable partial metrics are not a whole P PASS.',
  'p-full-review2-exception-1': 'Consumed one-run exception FAIL at own-relay helper startup;3287 calls and no final relay/history gates. Its relay-public-artifacts snapshot is a stale configuration transition, not URL-only/traffic proof.',
  'f-full-review2-frozen-1': 'F09 controller Windows os5 FAIL; precise historical operation/cause unclassified. Superseded by accepted controller repair and complete F, without changing this result.',
  'p-full-final-3-idle-read': 'Controlled stop from erroneous historical-budget subtraction; not a whole P PASS. Exact termination/orphan evidence is separate.',
  'regressions-protocol-ipc-final-1': 'Controlled stop after broad selector accidentally included179 unrelated protocol tests; superseded by bounded selectors.',
  'f-policy-2': 'Development-only: later actual-pause check exposed read-only marker fsync bug; do not use for acceptance.',
  'f-kills-1': 'Development-only: later actual-pause check exposed read-only marker fsync bug; do not use for acceptance.',
};
const ledger = fs.readdirSync(directory).filter(n => n.endsWith('-result.json')).sort().map(n => {
  const name = n.slice(0, -12);
  const result = json(evidence(n));
  const source = evidence(name + '-source.json');
  const log = evidence(name + '.log');
  const text = fs.existsSync(log) ? read(log) : '';
  const summaries = [...text.matchAll(/test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;[^\r\n]*finished in ([\d.]+)s/g)]
    .map(m => ({ result: m[1], passed: +m[2], failed: +m[3], ignored: +m[4], harness_seconds: +m[5] }));
  return {
    name,
    classification: contributing.has(name) ? 'final-candidate-' + contributing.get(name) : result.mode === 'run' ? 'development-not-contributing' : 'compile-static-build',
    ...(caveats[name] ? { caveat: caveats[name] } : {}),
    result,
    result_artifact: artifact(evidence(n)),
    source_artifact: fs.existsSync(source) ? artifact(source) : null,
    log_artifact: fs.existsSync(log) ? artifact(log) : null,
    harness_summaries: summaries,
    observed_passed: summaries.reduce((a, s) => a + s.passed, 0),
    observed_failed: summaries.reduce((a, s) => a + s.failed, 0),
    invocation: result.mode === 'run' ? {
      script: './ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence/run-check.ps1',
      Mode: 'run', Name: name, Package: result.packages, Filter: result.filter, SkipTest: result.skip,
      environment: 'CARGO_NET_OFFLINE=true; TEMP/TMP=.tmp/a3-s4/test-temp',
      native_windows_token: true,
    } : null,
  };
});
save('development-ledger.json', {
  generated_utc: new Date().toISOString(),
  rule: 'Original result/log/source artifacts are unchanged. Filename final does not imply acceptance. Failed/replaced runs are not hidden or summed into an unrelated final set.',
  commands: ledger,
  all_runner_seconds: ledger.reduce((a, r) => a + (r.result.seconds || 0), 0),
  all_run_seconds: ledger.filter(r => r.result.mode === 'run').reduce((a, r) => a + r.result.seconds, 0),
  runner_sum_scope: 'Unique run-check -result.json files, including all preserved failures and replacements; standalone static/inventory checks are separate.',
  separate_static_artifacts: fs.readdirSync(directory).filter(n => /^static-.*\.json$/.test(n)).sort().map(n => artifact(evidence(n))),
});
const suite = (names, ceiling) => {
  const runs = names.map(name => ledger.find(r => r.name === name));
  const present = runs.filter(Boolean);
  const seconds = present.reduce((a, r) => a + r.result.seconds, 0);
  return {
    names, missing: names.filter((_, i) => !runs[i]), ceiling_seconds: ceiling,
    runner_seconds: seconds,
    complete: present.length === names.length,
    pass: present.length === names.length && present.every(r => r.result.exit_code === 0) && seconds <= ceiling,
    observed_passed: present.reduce((a, r) => a + r.observed_passed, 0),
    runs: present.map(r => ({ name: r.name, seconds: r.result.seconds, exit_code: r.result.exit_code, source: r.source_artifact, result: r.result_artifact })),
  };
};
const suites = { P: suite(pNames, 3000), F: suite(fNames, 1200), components: suite(componentNames, 600) };
suites.P.authorization = 'Root review4 explicitly authorizes one replacement runner3000/controller2950 attempt after helper repair/smoke; prior review2 attempt1901.2816169s and P4 remain FAIL. All substantive gates unchanged.';
suites.P.original_target_seconds = 2400;
suites.P.original_target_met = suites.P.complete && suites.P.runs.every(r => r.exit_code === 0) && suites.P.runner_seconds <= 2400;
suites.P.exception_used = suites.P.pass && suites.P.runner_seconds > 2400;
save('final-suite-summary.json', suites);
save('f-final-aggregate.json', { ...suites.F, source_scope: 'Root conservative accounting: guard185.5564679 + lock-controls22.3189717 + actualF09 repair35.3227291 + complete post-review3 F. All use frozen d5d8c8 product; controller-only changes are explicit. Earlier failed F365.2830575 remains development FAIL, not PASS.' });
save('components-aggregate.json', { ...suites.components, broad_stopped_selector_is_development_only: true, heavy_100k_or_long_S3_not_selected: true });

const base = json(evidence('baseline.json'));
const protectedFiles = base.protected_files.map(old => {
  const current = artifact(path.resolve(repo, old.path));
  if (!old.root_owned && current.sha256 !== old.sha256) throw Error('Frozen input drift: ' + old.path);
  return { ...current, initial_sha256: old.sha256, unchanged: current.sha256 === old.sha256, root_owned: old.root_owned, allowlist: false };
});
const head = git('rev-parse', 'HEAD');
const parent = git('rev-parse', 'HEAD^');
const staged = git('diff', '--cached', '--name-only');
if (head !== base.baseline || parent !== base.parent || staged) throw Error('Unexpected HEAD/parent/index change');
save('protected-inputs-final.json', {
  baseline: head, sole_parent: parent, index_empty: true,
  files: protectedFiles,
  additional_root_reference: artifact(path.resolve(repo, 'ai-docs/tasks/long-lived-dialog-a3-runtime-s4-time-accounting-clarification-1.md')),
  coordination_note: 'Root changed coordination and added time-accounting clarification during verification. Preserve current root-owned files; exclude from executor allowlist.',
});

const excluded = new Set([
  'ai-docs/README.md', 'ai-docs/chat-coordination.md',
  'ai-docs/tasks/long-lived-dialog-a3-runtime-s4-implementation-task.md',
  'ai-docs/tasks/long-lived-dialog-a3-runtime-s4-time-accounting-clarification-1.md',
  'ai-docs/tasks/long-lived-dialog-a3-runtime-s4-implementation-review-1.md',
  'ai-docs/tasks/long-lived-dialog-a3-runtime-s4-implementation-review-2.md',
  'ai-docs/tasks/long-lived-dialog-a3-runtime-s4-implementation-review-3.md',
]);
for (const n of fs.readdirSync(path.resolve(repo, 'ai-docs/tasks'))) {
  if (/^long-lived-dialog-a3-runtime-s4-implementation-review-\d+\.md$/.test(n)) excluded.add('ai-docs/tasks/' + n);
}
const rootPaths = [...excluded].sort();
const rootBlobs = execFileSync('git', ['hash-object', '--stdin-paths'], {
  cwd: repo, encoding: 'utf8', input: rootPaths.map(n => JSON.stringify(n)).join('\n') + '\n',
}).trim().split(/\r?\n/);
if (rootBlobs.length !== rootPaths.length) throw Error('Root document blob manifest length mismatch');
save('root-documents-reference.json', {
  status: 'Root-owned files preserved; separate reference for eventual accepted allowlist, no publication permission',
  files: rootPaths.map((n, i) => ({ ...artifact(path.resolve(repo, n)), git_blob: rootBlobs[i] })),
});
const changed = () => unique((git('ls-files', '--modified') + '\n' + git('ls-files', '--others', '--exclude-standard')).split(/\r?\n/).filter(Boolean));
const sources = changed().filter(n => !excluded.has(n) && /\.rs$|\.ps1$|(^|\/)Cargo\.(toml|lock)$/.test(n)).map(n => artifact(path.resolve(repo, n)));
const snapshotMap = name => new Map(json(evidence(name + '-source.json')).map(r => [r.path, r]));
const product = n => n.endsWith('.rs') && !/(?:^|[/_])tests?(?:[/_.]|$)/.test(n);
const finalMap = new Map(sources.map(s => [s.path, s]));
const compare = name => {
  const old = snapshotMap(name);
  return sources.filter(s => product(s.path) && old.get(s.path)?.sha256 !== s.sha256)
    .map(s => ({ path: s.path, old_sha256: old.get(s.path)?.sha256 || null, final_sha256: s.sha256 }));
};
const pDrift = compare(pNames[0]);
const pSnapshot = snapshotMap(pNames[0]);
const pRustDrift = sources.filter(s => s.path.endsWith('.rs') && pSnapshot.get(s.path)?.sha256 !== s.sha256);
const authorizedAfterP = new Set([
  'apps/kilogram-cli/src/ordered_dialog/runtime2/mod.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/runtime.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/owner.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/data.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/native.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/network.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/hooks.rs',
  'apps/kilogram-cli/src/ordered_dialog/runtime2/timing.rs',
  'crates/kilogram-transport-iroh/src/ordered2.rs',
  'crates/kilogram-transport-iroh/examples/s4_loopback_relay.rs',
]);
if (pDrift.length || pRustDrift.length) throw Error('Frozen Rust sources changed during final review4 P: ' + JSON.stringify(pRustDrift));
const p4Changes = compare('p-full-final-4-contract-budget');
if (p4Changes.some(f => !authorizedAfterP.has(f.path))) throw Error('Unexpected product drift after P4: ' + JSON.stringify(p4Changes));
save('final-source-correspondence.json', {
  current_sources: sources,
  final_P_product_changes: pDrift,
  final_P_all_Rust_changes: pRustDrift,
  P4_product_changes_after_failed_run: p4Changes,
  authorization: 'Root review4 accepts helper-only readiness/liveness repair and real relay smoke, then one replacement complete P. CLI d5d8c8 and all42 in-src Rust files stay unchanged. Historical P4/review2 failures remain unchanged.',
  complete_F_product_changes: compare(fNames[3]),
  guard_product_changes: compare(fNames[0]),
  lock_controls_product_changes: compare(fNames[1]),
  targeted_F09_product_changes: compare(fNames[2]),
  required_review: 'Review4 helper change is outside exercised F paths; positive::hit/selected and all F functions unchanged, current complete F reused under explicit root decision. Inspect helper/source correspondence and component scopes. Hash differences alone are not semantic equivalence.',
  component_source_scope: 'component-source-reuse.json',
  relay_and_F_source_scope: 'review4-relay-source-correspondence.json',
});

const images = [
  'target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe',
  'target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe',
  'target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe',
  'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe',
].map(p => artifact(path.resolve(repo, p)));
save('final-images.json', { images, stable_paths: true, no_EXE_revision_in_filename: true });

const manifestName = 'implementation-files.json';
const ownPaths = changed().filter(n => !excluded.has(n) && n !== relative(evidence(manifestName)));
for (const n of ownPaths) {
  if (!/^(apps\/kilogram-cli\/|crates\/kilogram-(identity|protocol|runtime-ipc|state|transport-iroh)\/|ai-docs\/tasks\/long-lived-dialog-a3-runtime-s4-(evidence\/|implementation\.md$))/.test(n)) throw Error('Unclassified changed path: ' + n);
}
const blobs = execFileSync('git', ['hash-object', '--stdin-paths'], {
  cwd: repo, encoding: 'utf8', input: ownPaths.map(n => JSON.stringify(n)).join('\n') + '\n',
}).trim().split(/\r?\n/);
if (blobs.length !== ownPaths.length) throw Error('Git blob manifest length mismatch');
const files = ownPaths.map((n, i) => ({ ...artifact(path.resolve(repo, n)), git_blob: blobs[i] }));
save(manifestName, {
  status: 'implementation-review-candidate; root acceptance pending; not a commit instruction',
  baseline: head, parent, index_empty: true,
  excluded_root_owned_paths: [...excluded],
  unchanged_development_environment_excluded: 'ai-docs/development-environment.md',
  self_excluded: relative(evidence(manifestName)),
  rule: 'All listed raw content/blob hashes describe current review files. Manifest self hash is supplied separately in callback; no recursive self hash.',
  file_count: files.length, total_bytes: files.reduce((a, f) => a + f.bytes, 0), files,
});
console.log(JSON.stringify({ suites, manifest: artifact(evidence(manifestName)), file_count: files.length }, null, 2));
