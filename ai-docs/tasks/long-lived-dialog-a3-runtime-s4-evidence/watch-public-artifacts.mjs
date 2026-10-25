// Public artifact capture only; it neither gates nor drives acceptance traffic.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const directory = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(directory, '../../..');
const run = process.argv[2];
if (!run || !/^[a-z0-9-]+$/.test(run)) throw Error('Explicit evidence run name required');
const root = path.resolve(repo, '.tmp/a3-s4/process') + path.sep;
const start = Date.now();
const saved = new Set();
const pause = ms => new Promise(r => setTimeout(r, ms));
while (Date.now() - start < 3_100_000) {
  const log = path.join(directory, run + '.log');
  if (fs.existsSync(log)) {
    const text = fs.readFileSync(log, 'utf8');
    const match = text.match(/fixture=(.+?) cli_sha256_external_manifest_required=true/);
    if (match) {
      const fixture = path.resolve(repo, match[1]);
      if (!fixture.startsWith(root)) throw Error('Fixture outside task temp');
      try {
        const configs = [0, 1].map(i => JSON.parse(fs.readFileSync(path.join(fixture, 'profile' + i + '.json'), 'utf8')));
        const lists = [0, 1].map(i => fs.readdirSync(path.join(fixture, 'locators' + i)).filter(n => /^[a-f0-9]{64}\.locator$/.test(n)));
        const kind = configs.every(c => c.route.policy === 'relay-only') && /^P08_B_only_exact_progress /m.test(text) ? 'relay' : configs.every(c => c.route.policy === 'direct-only') && /P02_checkpoint through=256/.test(text) ? 'direct' : null;
        if (kind && !saved.has(kind) && lists.every(a => a.length === 2)) {
          const names = ['profile0.json', 'profile1.json'];
          lists.forEach((a, i) => a.forEach(n => names.push('locators' + i + '/' + n)));
          const files = names.map(n => {
            const p = path.join(fixture, n);
            const stat = fs.lstatSync(p);
            if (!stat.isFile() || stat.isSymbolicLink()) throw Error('Unexpected public artifact type');
            const b = fs.readFileSync(p);
            return {
              path: path.relative(repo, p).split(path.sep).join('/'), bytes: b.length,
              sha256: crypto.createHash('sha256').update(b).digest('hex'),
              ...(n.startsWith('profile') ? { public_config: JSON.parse(b.toString()) } : { signed_public_locator_hex: b.toString('hex') }),
            };
          });
          fs.writeFileSync(path.join(directory, run + '-' + kind + '-public-artifacts.json'), JSON.stringify({ run, kind, captured_utc: new Date().toISOString(), scope: 'Public artifacts only; no IPC token, vault, Root/Device key or Native checkpoint read. Actual route/endpoint assertions belong to P controller.', files }, null, 2) + '\n', { flag: 'wx' });
          saved.add(kind);
          console.log('public_capture=' + kind + ' files=' + files.length);
        }
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
      }
    }
  }
  if (fs.existsSync(path.join(directory, run + '-result.json'))) {
    console.log('P_finished public_captures=' + [...saved].join(','));
    break;
  }
  await pause(500);
}
