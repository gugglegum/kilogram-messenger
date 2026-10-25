// Aggregate existing public instrumentation; no fixture mutation or secret reads.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(directory, '../../..');
const run = process.argv[2] || 'p-full-final-4-contract-budget';
const outputName = process.argv[3] || 'final-process-metrics.json';
const controllerPath = path.join(directory, run + '.log');
const controller = fs.readFileSync(controllerPath, 'utf8');
const match = controller.match(/fixture=(.+?) cli_sha256_external_manifest_required=true/);
if (!match) throw Error('Missing owned P fixture');
const fixture = path.resolve(repo, match[1]);
const root = path.resolve(repo, '.tmp/a3-s4/process') + path.sep;
if (!fixture.startsWith(root)) throw Error('Unexpected fixture outside task temp');
const digest = b => crypto.createHash('sha256').update(b).digest('hex');
const artifact = p => {
  const b = fs.readFileSync(p);
  return { path: path.relative(repo, p).split(path.sep).join('/'), bytes: b.length, sha256: digest(b) };
};
const number = (line, key) => {
  const m = line.match(new RegExp('(?:^| )' + key + '=([0-9]+)(?: |$)'));
  return m ? +m[1] : 0;
};
const timings = new Map();
const bindings = new Map();
const pages = [];
const logs = [];
let attempts = 0, committed = 0, accounts = 0, sessions = 0, fullHeads = 0;
for (const name of fs.readdirSync(fixture).filter(n => /^profile[01]\.\d+\.stderr\.log$/.test(n)).sort()) {
  const p = path.join(fixture, name);
  const text = fs.readFileSync(p, 'utf8');
  logs.push(artifact(p));
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith('od2_native_attempt ')) attempts++;
    if (line.startsWith('od2_native_commit ')) committed += number(line, 'calls');
    if (line.startsWith('od2_account_constructor ')) accounts++;
    if (line.startsWith('od2_session_constructor ')) sessions++;
    if (line.startsWith('od2_funded_head context=0 ordinary_pending=64 ')) fullHeads++;
    if (line.startsWith('od2_commit_end ')) {
      const op = line.match(/operation=(\w+)/)?.[1];
      if (!op) throw Error('Malformed commit instrumentation');
      const values = timings.get(op) || { ok_us: [], error_us: [] };
      values[line.includes(' ok=true ') ? 'ok_us' : 'error_us'].push(number(line, 'micros'));
      timings.set(op, values);
    }
    if (line.startsWith('od2_admitted ')) {
      const key = line.match(/binding=(\S+)/)?.[1];
      if (key) {
        const values = bindings.get(key) || new Set();
        values.add(name.slice(0, 8));
        bindings.set(key, values);
      }
    }
    if (line.startsWith('od2_prefix_saved context=0 head=false ')) pages.push({ start: number(line, 'start'), count: number(line, 'count') });
  }
}
const summarize = xs => {
  xs.sort((a, b) => a - b);
  const q = n => xs.length ? xs[Math.min(xs.length - 1, Math.ceil(n * xs.length) - 1)] / 1000 : null;
  return { count: xs.length, sum_ms: xs.reduce((a, x) => a + x, 0) / 1000, p50_ms: q(0.5), p95_ms: q(0.95), max_ms: q(1) };
};
const statuses = controller.split(/\r?\n/).filter(l => l.startsWith('P_final_status ')).map(l => JSON.parse(l.slice(15)));
const histories = controller.split(/\r?\n/).filter(l => l.startsWith('history '));
const phaseLines = controller.split(/\r?\n/).filter(l => /^P0[2-9]_|^queue |^active_progress /.test(l));
const memory = [...controller.matchAll(/private_bytes pid=(\d+) bytes=(\d+)/g)];
const maxByPid = {};
for (const m of memory) maxByPid[m[1]] = Math.max(maxByPid[m[1]] || 0, +m[2]);
const publicPaths = ['profile0.json', 'profile1.json', 'context0.bin', 'context1.bin', 'credential0.bin', 'credential1.bin'];
for (const d of ['locators0', 'locators1']) {
  for (const n of fs.readdirSync(path.join(fixture, d))) if (n.endsWith('.locator')) publicPaths.push(d + '/' + n);
}
const publicArtifacts = publicPaths.map(n => {
  const p = path.join(fixture, n);
  return { ...artifact(p), ...(n.startsWith('profile') ? { public_config: JSON.parse(fs.readFileSync(p, 'utf8')) } : n.endsWith('.locator') ? { signed_public_locator_hex: fs.readFileSync(p).toString('hex') } : {}) };
});
const result = {
  run, fixture, controller: artifact(controllerPath),
  scope: 'Aggregates existing public controller/child logs; no vault, IPC token, Root/Device key or Native checkpoint read. IO latencies are closed-writer elapsed, not whole materialize or history throughput.',
  whole_P_pass_line: controller.split(/\r?\n/).find(l => l.startsWith('S4_P_PASS ')) || null,
  successful_packet_calls: committed, packet_attempts: attempts,
  attempted_minus_committed: attempts - committed, account_constructors: accounts, session_constructors: sessions,
  closed_writer_timings: Object.fromEntries([...timings].map(([name, t]) => [name, { successful: summarize(t.ok_us), error: summarize(t.error_us) }])),
  distinct_bindings: bindings.size,
  symmetric_bindings: [...bindings.values()].filter(v => v.size === 2).length,
  binding_profile_sets: Object.fromEntries([...bindings].map(([key, values]) => [key, [...values]])),
  normal_C1_pages: pages, full64_funded_heads: fullHeads,
  actual_private_bytes_samples: memory.length, max_private_bytes_by_pid: maxByPid,
  final_statuses: statuses, history_measurements: histories, phase_measurements: phaseLines,
  child_log_artifacts: logs, final_public_artifacts: publicArtifacts,
  timing_limitations: 'P02/duplex phase elapsed includes that phase work. active_progress seconds measures its own activation wait, not all administrative setup. Queue lines measure each send-range segment. Closed-writer p50/p95 excludes prior crypto trial work. No whole-scenario time is presented as history speed.',
};
const output = path.join(directory, outputName);
fs.writeFileSync(output, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ whole_P_pass_line: result.whole_P_pass_line, committed, attempts, accounts, sessions, symmetric_bindings: result.symmetric_bindings, normal_pages: pages.length, fullHeads, history_measurements: histories, output: artifact(output) }, null, 2));
