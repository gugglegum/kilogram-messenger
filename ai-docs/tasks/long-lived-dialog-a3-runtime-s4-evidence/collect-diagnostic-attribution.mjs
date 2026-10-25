// Read only numerical diagnostic/public logs, never vaults or private checkpoints.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(directory, '../../..');
const name = process.argv[2] || 'diagnostic-costs-64-duplex-1';
const outputName = process.argv[3] || 'diagnostic-attribution.json';
const imageName = process.argv[4] || 'diagnostic-image-2.json';
const read = p => fs.readFileSync(p, 'utf8').replace(/^\uFEFF/, '');
const controllerPath = path.join(directory, name + '.log');
const controller = read(controllerPath);
const fixtureMatch = controller.match(/fixture=(.+?) cli_sha256_external_manifest_required=true/);
if (!fixtureMatch) throw Error('Missing owned diagnostic fixture');
const fixture = path.resolve(repo, fixtureMatch[1]);
if (!fixture.startsWith(path.resolve(repo, '.tmp/a3-s4/process') + path.sep)) throw Error('Unexpected fixture');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const artifact = p => {
  const b = fs.readFileSync(p);
  return { path: path.relative(repo, p).split(path.sep).join('/'), bytes: b.length, sha256: hash(b) };
};
const perProcess = [];
const logs = [];
const totals = {};
const closed = {};
for (const n of fs.readdirSync(fixture).filter(n => /^profile[01]\.1\.stderr\.log$/.test(n)).sort()) {
  const p = path.join(fixture, n);
  const text = read(p);
  const metrics = {};
  let pid = null;
  for (const m of text.matchAll(/^od2_diag pid=(\d+) metric=(\w+) count=(\d+) nanos=(\d+) max_nanos=(\d+)$/gm)) {
    pid = +m[1];
    const value = { count: +m[3], elapsed_seconds: +m[4] / 1e9, average_ms: +m[3] ? +m[4] / +m[3] / 1e6 : 0, max_ms: +m[5] / 1e6 };
    metrics[m[2]] = value;
    const total = totals[m[2]] || { count: 0, elapsed_seconds: 0, max_ms: 0 };
    total.count += value.count;
    total.elapsed_seconds += value.elapsed_seconds;
    total.max_ms = Math.max(total.max_ms, value.max_ms);
    totals[m[2]] = total;
  }
  for (const m of text.matchAll(/^od2_commit_end operation=(\w+) context=\d+ generation=\d+ ok=(\w+) micros=(\d+)$/gm)) {
    if (m[2] !== 'true') continue;
    const value = closed[m[1]] || { count: 0, elapsed_seconds: 0, max_ms: 0 };
    value.count++;
    value.elapsed_seconds += +m[3] / 1e6;
    value.max_ms = Math.max(value.max_ms, +m[3] / 1000);
    closed[m[1]] = value;
  }
  logs.push(artifact(p));
  perProcess.push({ pid, profile: n, metrics });
}
if (perProcess.some(p => p.pid === null)) throw Error('Numerical report missing; owned child may not have stopped gracefully');
for (const value of Object.values(totals)) value.average_ms = value.elapsed_seconds * 1000 / value.count;
const done = controller.split(/\r?\n/).find(l => l.startsWith('S4_DIAGNOSTIC_DONE '));
if (!done) throw Error('Diagnostic did not complete');
const scalar = key => Number(done.match(new RegExp('(?:^| )' + key + '=([0-9.]+)(?: |$)'))?.[1]);
const optionArray = key => done.match(new RegExp(key + '=\\[([^\\]]+)\\]'))?.[1].split(',').map(x => x.trim() === 'None' ? null : Number(x.trim().replace(/^Some\(/, '').replace(/\)$/, '')));
const pids = done.match(/pids=\[([^\]]+)\]/)?.[1].split(',').map(Number);
const before = optionArray('cpu_before_ms');
const after = optionArray('cpu_after_ms');
const cpu = pids?.map((pid, i) => ({ pid, data_cpu_seconds: before?.[i] == null || after?.[i] == null ? null : (after[i] - before[i]) / 1000 }));
const ownerWork = ['OwnerJob', 'MaterializeTurn', 'DrainTurn', 'OwnerInstalled'].reduce((a, key) => a + (totals[key]?.elapsed_seconds || 0), 0);
const ranked = Object.entries(totals).map(([metric, value]) => ({ metric, ...value, fraction_of_owner_service: value.elapsed_seconds / ownerWork })).sort((a, b) => b.elapsed_seconds - a.elapsed_seconds);
const output = {
  name, acceptance_P: false, watchdog_seconds: 280,
  fixture, controller: artifact(controllerPath),
  source: artifact(path.join(directory, name + '-source.json')),
  image: JSON.parse(read(path.join(directory, imageName))),
  summary_line: done,
  elapsed: Object.fromEntries(['setup_seconds', 'one_way64_seconds', 'duplex34_seconds', 'data_elapsed_seconds', 'history_seconds', 'controller_seconds'].map(k => [k, scalar(k)])),
  cpu, owner_service_seconds_sum: ownerWork,
  nesting_rule: 'OwnerJob, MaterializeTurn, DrainTurn, OwnerInstalled are disjoint owner service categories. Other spans are nested or async waiting and MUST NOT be added as independent wall time. Lifetime counters include startup/history, CPU delta covers data phase only. Closed-writer micros begin later than ClosedCommit span.',
  ranking: ranked, totals, per_process: perProcess, closed_writer_by_operation: closed,
  child_log_artifacts: logs,
  privacy: 'Only metric labels/counts/nanoseconds/PIDs/public log artifact hashes are used. No Root/Device key, IPC token, state vault or Native checkpoint read.',
};
const asynchronous = new Set(['MailboxSubmit', 'MailboxQueue', 'OwnerIdle', 'Rpc', 'DataRound', 'PushLive', 'ServerSession', 'ReadRound', 'TakeDelivery', 'TakeAck', 'TakePrefix']);
output.owner_cost_ranking = ranked.filter(r => !asynchronous.has(r.metric));
output.wait_or_async_ranking = ranked.filter(r => asynchronous.has(r.metric));
const out = path.join(directory, outputName);
fs.writeFileSync(out, JSON.stringify(output, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ elapsed: output.elapsed, cpu, owner_service_seconds_sum: ownerWork, top_metrics: ranked.slice(0, 18), artifact: artifact(out) }, null, 2));
