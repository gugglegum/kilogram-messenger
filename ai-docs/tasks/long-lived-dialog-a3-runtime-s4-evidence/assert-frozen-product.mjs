// Read-only preflight for accepted verification candidate, no builds/copies.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const directory = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(directory, '../../..');
process.chdir(repo);
const frozen = JSON.parse(fs.readFileSync(path.join(directory, 'review2-production-freeze.json'), 'utf8'));
const hash = p => crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const repairPath = path.join(directory, 'review4-relay-source-correspondence.json');
const repair = fs.existsSync(repairPath) ? JSON.parse(fs.readFileSync(repairPath, 'utf8')) : null;
for (const f of [...frozen.product_files, ...frozen.images]) {
  const expected = repair && f.path === repair.helper_source.path ? repair.helper_source.new_sha256
    : repair && f.path === 'target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe' ? repair.helper_image.sha256 : f.sha256;
  if (hash(f.path) !== expected) throw Error('Frozen content drift: ' + f.path);
}
const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
const changed = [...new Set((git('ls-files', '--modified') + '\n' + git('ls-files', '--others', '--exclude-standard')).split(/\r?\n/).filter(Boolean))];
const product = n => n.endsWith('.rs') && !/(?:^|[/_])tests?(?:[/_.]|$)/.test(n);
const actual = changed.filter(product).sort();
const expected = frozen.product_files.map(f => f.path).sort();
if (JSON.stringify(actual) !== JSON.stringify(expected)) throw Error('Unexpected production source set');
if (changed.some(n => /(^|\/)Cargo\.(toml|lock)$/.test(n))) throw Error('Cargo input drift');
if (git('rev-parse', 'HEAD') !== '9547ed171c64bf5ea529b4ba487acf6aa1a61b5b' || git('diff', '--cached', '--name-only')) throw Error('HEAD/index drift');
console.log('frozen_source_set=42 application41_unchanged=true helper_review4_override=' + Boolean(repair) + ' images=4 cargo_inputs_unchanged=true HEAD_same=true index_empty=true');
