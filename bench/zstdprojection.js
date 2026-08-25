// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Does a zstd-carrying base91-jdp beat the alternatives, and at what setting?
//
// The question this answers is a decision, not a release number: if the
// compressed segment of the 0.4 draft carries a zstd frame, then every
// contender in the table below runs the *same* compressor, and what is left
// is the density of the container around it. That is arithmetic the format
// already fixes -- sixteen characters per thirteen bytes -- so the projection
// needs no implementation of the draft to be exact:
//
//     characters = signal(2) + length field + 2 * ceil(8 * |frame| / 13)
//
// Base85N runs the upstream Go implementation over the same frames.
//
//   node bench/zstdprojection.js [--group core|silesia|all] [--levels 1,3,9]
//   node bench/zstdprojection.js --segments   what 64 KiB segmenting costs
//   node bench/zstdprojection.js --speed      throughput per level

import { execFileSync } from 'node:child_process';
import { zstdCompressSync, constants } from 'node:zlib';
import { readFileSync, readdirSync, statSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { BENCH_DIR, CORPUS_DIR } from './lib.js';

const arg = (name, fallback) => {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : fallback;
};
const GROUP = arg('--group', 'core');
const LEVELS = arg('--levels', '1,3,9').split(',').map(Number);

const zstd = (bytes, level) =>
  zstdCompressSync(bytes, { params: { [constants.ZSTD_c_compressionLevel]: level } });

/** The block coder, exactly: thirteen bytes to sixteen characters. */
const blockChars = (n) => 2 * Math.ceil((8 * n) / 13);
/** Section 7.3 of the 0.4 draft: one, three or seven characters. */
const lengthChars = (n) => (n < 90 ? 1 : n < 8370 ? 3 : 7);
/** One compressed segment: signal, length, payload. */
const segChars = (n) => 2 + lengthChars(n) + blockChars(n);
const b64Chars = (n) => 4 * Math.ceil(n / 3);

function files(group) {
  const dirs = group === 'silesia' ? [join(CORPUS_DIR, 'silesia')]
    : group === 'core' ? [CORPUS_DIR]
    : [CORPUS_DIR, join(CORPUS_DIR, 'silesia')];
  const out = [];
  for (const dir of dirs) {
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name);
      if (!statSync(path).isFile()) continue;
      out.push({ name, path, group: dir.endsWith('silesia') ? 'silesia' : 'core' });
    }
  }
  return out;
}

/** Base85N encoded sizes, from the upstream Go implementation. */
function base85n(paths) {
  const out = execFileSync('go', ['run', '.', ...paths], {
    cwd: join(BENCH_DIR, 'base85n'),
    encoding: 'utf8',
    maxBuffer: 1 << 28,
  });
  const map = new Map();
  for (const line of out.trim().split('\n')) {
    const [path, , enc] = line.split('\t');
    map.set(path.split('/').pop(), Number(enc));
  }
  return map;
}

/** Base85N over a set of byte blobs, written out under one temp directory. */
function base85nOf(blobs) {
  const dir = mkdtempSync(join(tmpdir(), 'b91-proj-'));
  try {
    const paths = [];
    for (const [name, bytes] of blobs) {
      const p = join(dir, name);
      writeFileSync(p, bytes);
      paths.push(p);
    }
    return base85n(paths);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const corpus = files(GROUP);
if (corpus.length === 0) throw new Error(`no corpus files for group ${GROUP} -- run python3 bench/corpus.py`);

if (process.argv.includes('--speed')) {
  const f = corpus.find((x) => x.name === 'countries.json') ?? corpus[0];
  const data = readFileSync(f.path);
  console.log(`### Throughput, ${f.name}, ${data.length.toLocaleString('en-US')} B\n`);
  console.log('| level | compressed | ratio | compress | + block coder | end to end |');
  console.log('|---|---|---|---|---|---|');
  for (const level of [-5, -1, 1, 3, 9, 15, 19]) {
    let z, t0 = process.hrtime.bigint();
    let runs = 0;
    do { z = zstd(data, level); runs++; } while (Number(process.hrtime.bigint() - t0) < 2e9);
    const ms = Number(process.hrtime.bigint() - t0) / 1e6 / runs;
    const mbs = data.length / 1e6 / (ms / 1000);
    // The block coder over the frame, measured the same way.
    const t1 = process.hrtime.bigint();
    let packed = 0, pruns = 0;
    do { packed = packChars(z); pruns++; } while (Number(process.hrtime.bigint() - t1) < 1e9);
    const pms = Number(process.hrtime.bigint() - t1) / 1e6 / pruns;
    const pmbs = z.length / 1e6 / (pms / 1000);
    const end = data.length / 1e6 / ((ms + pms) / 1000);
    console.log(
      `| ${level} | ${z.length.toLocaleString('en-US')} B | ${(segChars(z.length) / data.length).toFixed(4)} | ` +
      `${mbs.toFixed(0)} MB/s | ${pmbs.toFixed(0)} MB/s | ${end.toFixed(0)} MB/s |`,
    );
  }
  process.exit(0);
}

/** The fixed thirteen-bit packer, for the throughput column only. */
function packChars(bytes) {
  let acc = 0, nb = 0, chars = 0;
  for (let i = 0; i < bytes.length; i++) {
    acc = ((acc << 8) | bytes[i]) >>> 0;
    nb += 8;
    if (nb >= 13) { nb -= 13; chars += 2; acc &= (1 << nb) - 1; }
  }
  return chars + (nb ? (nb <= 6 ? 1 : 2) : 0);
}

if (process.argv.includes('--segments')) {
  const SEG = 1 << 16;
  console.log('### What segmenting the compressed payload costs\n');
  console.log('| sample | one frame | 64 KiB segments | 256 KiB segments | 1 MiB segments |');
  console.log('|---|---|---|---|---|');
  const totals = [0, 0, 0, 0];
  let inBytes = 0;
  for (const f of corpus) {
    const data = readFileSync(f.path);
    inBytes += data.length;
    const whole = segChars(zstd(data, 3).length);
    const cut = [SEG, SEG * 4, SEG * 16].map((size) => {
      let chars = 0;
      for (let at = 0; at < data.length; at += size) {
        chars += segChars(zstd(data.subarray(at, Math.min(at + size, data.length)), 3).length);
      }
      return chars;
    });
    const all = [whole, ...cut];
    all.forEach((v, i) => { totals[i] += v; });
    console.log(`| ${f.name} | ${(whole / data.length).toFixed(4)} | ` +
      cut.map((c) => `${(c / data.length).toFixed(4)} (+${((c / whole - 1) * 100).toFixed(1)} %)`).join(' | ') + ' |');
  }
  console.log(`| **total** | ${(totals[0] / inBytes).toFixed(5)} | ` +
    totals.slice(1).map((t) => `${(t / inBytes).toFixed(5)} (+${((t / totals[0] - 1) * 100).toFixed(1)} %)`).join(' | ') + ' |');
  process.exit(0);
}

// --- the size table ---------------------------------------------------

const raw85 = base85n(corpus.map((f) => f.path));
const rows = [];
const roll = {};
let inBytes = 0;

for (const level of LEVELS) {
  const blobs = corpus.map((f) => [f.name, zstd(readFileSync(f.path), level)]);
  const enc85 = base85nOf(blobs);
  blobs.forEach(([name, z], i) => {
    const f = corpus[i];
    const size = statSync(f.path).size;
    if (level === LEVELS[0]) inBytes += size;
    const row = rows.find((r) => r.name === name) ?? { name, group: f.group, size, block: blockChars(size), b85: raw85.get(name), lv: {} };
    if (!rows.includes(row)) rows.push(row);
    row.lv[level] = { jdp: segChars(z.length), b85z: enc85.get(name), b64z: b64Chars(z.length), zlen: z.length };
  });
}

const sum = (pick) => rows.reduce((s, r) => s + pick(r), 0);
console.log(`### Characters per input byte -- group "${GROUP}", ${corpus.length} files, ${inBytes.toLocaleString('en-US')} B\n`);
console.log('Every contender below runs the same zstd frame; what differs is the container.\n');
console.log(`| sample | input | Base85N | jdp block | ${LEVELS.map((l) => `Base64+zstd${l} | Base85N+zstd${l} | **jdp+zstd${l}**`).join(' | ')} |`);
console.log(`|---|---|---|---|${LEVELS.map(() => '---|---|---').join('|')}|`);
for (const r of rows) {
  const cells = LEVELS.flatMap((l) => [
    (r.lv[l].b64z / r.size).toFixed(4),
    (r.lv[l].b85z / r.size).toFixed(4),
    `**${(r.lv[l].jdp / r.size).toFixed(4)}**`,
  ]);
  console.log(`| ${r.name} | ${r.size.toLocaleString('en-US')} B | ${(r.b85 / r.size).toFixed(4)} | ${(r.block / r.size).toFixed(4)} | ${cells.join(' | ')} |`);
}
const tot = LEVELS.flatMap((l) => [
  (sum((r) => r.lv[l].b64z) / inBytes).toFixed(5),
  (sum((r) => r.lv[l].b85z) / inBytes).toFixed(5),
  `**${(sum((r) => r.lv[l].jdp) / inBytes).toFixed(5)}**`,
]);
console.log(`| **whole group** | ${inBytes.toLocaleString('en-US')} B | ${(sum((r) => r.b85) / inBytes).toFixed(5)} | ${(sum((r) => r.block) / inBytes).toFixed(5)} | ${tot.join(' | ')} |`);

console.log('\n### The margin over Base85N, same compressor both sides\n');
console.log('| level | Base85N+zstd | jdp+zstd | jdp is |');
console.log('|---|---|---|---|');
for (const l of LEVELS) {
  const a = sum((r) => r.lv[l].b85z) / inBytes;
  const b = sum((r) => r.lv[l].jdp) / inBytes;
  console.log(`| ${l} | ${a.toFixed(5)} | ${b.toFixed(5)} | ${((1 - b / a) * 100).toFixed(2)} % smaller |`);
}
