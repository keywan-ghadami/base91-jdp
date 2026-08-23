// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// How many bytes do you have to look at to decide whether to deflate first?
//
// base91-jdp deliberately has no run-length or dictionary construct: the
// answer to a payload with structure in it is a real compressor, not a mode.
// That leaves the caller with one decision -- deflate first, or encode
// directly -- and the decision is not obvious, because passthrough already
// carries text at 1.0 and deflate output always encodes at ~1.23.
//
// The rule under test estimates both costs from the first N bytes:
//
//     a = |encode(prefix)|      / |prefix|     jdp cost per byte
//     b = |deflateRaw(prefix)|  / |prefix|     compression ratio
//     deflate  iff  BLOCK * b * len + K  <  a * len
//
// and the question is how large N has to be before the estimate stops making
// decisions that cost more than they save. A short prefix understates
// compression -- a cold dictionary finds no long-range matches -- so the rule
// is biased towards encoding directly, and the bias is what N buys off.
//
//   node bench/gzipdecision.js [--gzip] [--json]

import { deflateRawSync, gzipSync } from 'node:zlib';
import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { encode, ALPHABET, R_CHARS } from '../src/index.js';
import { loadCorpus, BENCH_DIR } from './lib.js';

const USE_GZIP = process.argv.includes('--gzip');
const compress = (b) =>
  USE_GZIP ? gzipSync(b, { level: 9 }) : deflateRawSync(b, { level: 9 });

// Block mode charges 16/13 characters per byte, and deflate output is
// incompressible, so it never enters passthrough.
const BLOCK = 16 / 13;

// A passthrough byte costs 1.0 and a block byte 16/13, so the share of bytes
// passthrough can represent estimates the direct cost without encoding
// anything. 98 of the 256 byte values are representable.
const REPRESENTABLE = new Uint8Array(256);
for (const c of ALPHABET) REPRESENTABLE[c.charCodeAt(0)] = 1;
for (const c of R_CHARS) REPRESENTABLE[c] = 1;

function jdpCostByScan(bytes) {
  let ok = 0;
  for (const b of bytes) ok += REPRESENTABLE[b];
  const share = ok / bytes.length;
  return share * 1.0 + (1 - share) * BLOCK;
}

// ---------------------------------------------------------------------
// Payloads: slices of the benchmark and training corpora at many sizes
// ---------------------------------------------------------------------

function rng(seed) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const SIZES = [64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 65536, 262144];
const SLICES_PER_SIZE = 6;

// Payloads are classed by what deflate actually does to them, not by which
// file they came from: a 512-byte slice of a WebAssembly module compresses
// well even though the module as a whole is called binary, and classing by
// file would hide where the decision is hard.
const classOf = (ratio) =>
  ratio < 0.8 ? 'compressible' : ratio <= 1.0 ? 'borderline' : 'incompressible';

function buildPayloads() {
  const sources = loadCorpus().map((f) => ({ name: f.name, data: f.data }));
  const trainDir = join(BENCH_DIR, 'train');
  if (existsSync(trainDir)) {
    for (const n of readdirSync(trainDir).filter((n) => n.endsWith('.train')).sort()) {
      sources.push({ name: n, data: new Uint8Array(readFileSync(join(trainDir, n))) });
    }
  }
  const r = rng(4242);
  const out = [];
  for (const s of sources) {
    for (const size of SIZES) {
      if (size > s.data.length) continue;
      for (let k = 0; k < SLICES_PER_SIZE; k++) {
        const off = Math.floor(r() * (s.data.length - size + 1));
        out.push({ size, data: s.data.subarray(off, off + size) });
      }
    }
  }
  return out;
}

const payloads = buildPayloads();

// ---------------------------------------------------------------------
// Ground truth
// ---------------------------------------------------------------------

for (const p of payloads) {
  p.direct = encode(p.data).length;
  const packed = compress(p.data);
  p.deflated = encode(packed).length;
  p.best = Math.min(p.direct, p.deflated);
  p.truth = p.deflated < p.direct;
  p.kind = classOf(packed.length / p.data.length);
  p.gap = Math.abs(p.direct - p.deflated) / p.best;
}

// ---------------------------------------------------------------------
// The rule, at each prefix size
// ---------------------------------------------------------------------

function evaluate(N, margin, scanRule = false) {
  let wrong = 0;
  let regret = 0;
  let bestTotal = 0;
  const perKind = {
    compressible: { wrong: 0, n: 0 },
    borderline: { wrong: 0, n: 0 },
    incompressible: { wrong: 0, n: 0 },
  };
  let hard = 0;
  let hardWrong = 0;
  for (const p of payloads) {
    if (p.data.length <= N) continue; // the prefix is the payload: not an estimate
    const n = Math.min(N, p.data.length);
    const prefix = p.data.subarray(0, n);
    const a = scanRule ? jdpCostByScan(prefix) : encode(prefix).length / n;
    const b = compress(prefix).length / n;
    const predictDeflate = BLOCK * b * p.data.length + margin < a * p.data.length;
    const chose = predictDeflate ? p.deflated : p.direct;
    regret += chose - p.best;
    bestTotal += p.best;
    if (p.gap < 0.05) hard++;
    if (predictDeflate !== p.truth) {
      wrong++;
      perKind[p.kind].wrong++;
      if (p.gap < 0.05) hardWrong++;
    }
    perKind[p.kind].n++;
  }
  const n = Object.values(perKind).reduce((s, k) => s + k.n, 0);
  const acc = (k) => (perKind[k].n ? 1 - perKind[k].wrong / perKind[k].n : NaN);
  return {
    N,
    margin,
    wrong,
    n,
    accuracy: 1 - wrong / n,
    regret: regret / bestTotal,
    compressible: acc('compressible'),
    borderline: acc('borderline'),
    incompressible: acc('incompressible'),
    hardShare: hard / n,
    hardAccuracy: hard ? 1 - hardWrong / hard : NaN,
  };
}

const pct = (x) => (Number.isNaN(x) ? '--' : `${(x * 100).toFixed(1)} %`);

// ---------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------

const MARGINS = [0, 8, 16, 24, 32, 48, 64];
const PREFIXES = [32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384];

const pick = (N, scanRule) => {
  let best = null;
  for (const m of MARGINS) {
    const r = evaluate(N, m, scanRule);
    if (!best || r.regret < best.regret) best = r;
  }
  return best;
};
const rows = PREFIXES.map((N) => pick(N, false));
const scanRows = PREFIXES.map((N) => pick(N, true));

if (process.argv.includes('--json')) {
  console.log(JSON.stringify({ rows, whole, payloads: payloads.length }, null, 2));
} else {
  console.log(
    `${payloads.length} payloads, 64 B to 256 KiB, sliced from the benchmark and ` +
      `training corpora\ncompressor: ${USE_GZIP ? 'gzip -9' : 'raw deflate -9'}\n`,
  );
  console.log(
    'Only payloads longer than the prefix are counted in each row: where the\n' +
      'prefix is the whole payload the rule is not estimating anything.\n',
  );
  const table = (rs) => {
    console.log(
      '| bytes inspected | payloads | correct | compressible | borderline | ' +
        'incompressible | near-ties correct | bytes lost vs the oracle |',
    );
    console.log('|---|---|---|---|---|---|---|---|');
    for (const r of rs) {
      console.log(
        `| ${r.N} | ${r.n} | ${pct(r.accuracy)} | ${pct(r.compressible)} | ` +
          `${pct(r.borderline)} | ${pct(r.incompressible)} | ${pct(r.hardAccuracy)} | ` +
          `${(r.regret * 100).toFixed(3)} % |`,
      );
    }
  };
  console.log('### Rule A: deflate the prefix, encode the prefix\n');
  table(rows);
  console.log(
    '\n### Rule B: deflate the prefix, estimate the direct cost by a byte scan\n' +
      '\nNo encoding at all -- just the share of bytes passthrough can carry.\n',
  );
  table(scanRows);

  // What the oracle itself decides, by payload size -- the shape of the problem
  console.log('\n### What the right answer actually is, by payload size\n');
  console.log('| size | payloads | compressible | borderline | incompressible | overall |');
  console.log('|---|---|---|---|---|---|');
  for (const size of SIZES) {
    const sel = payloads.filter((p) => p.size === size);
    if (!sel.length) continue;
    const share = (kind) => {
      const s = kind ? sel.filter((p) => p.kind === kind) : sel;
      return s.length
        ? `${((s.filter((q) => q.truth).length / s.length) * 100).toFixed(0)} % (${s.length})`
        : '--';
    };
    console.log(
      `| ${size} B | ${sel.length} | ${share('compressible')} | ${share('borderline')} | ` +
        `${share('incompressible')} | ${share(null)} |`,
    );
  }
  console.log(
    '\nShare of payloads where deflating first gives the smaller output, with the' +
      '\nnumber of payloads in that cell in brackets.',
  );

  // How much the decision is worth at all
  console.log('\n### How far apart the two paths are\n');
  console.log('| payload | share | median gap | 90th percentile gap | share within 5 % |');
  console.log('|---|---|---|---|---|');
  for (const kind of ['compressible', 'borderline', 'incompressible']) {
    const sel = payloads.filter((p) => p.kind === kind);
    if (!sel.length) continue;
    const gaps = sel.map((p) => p.gap).sort((a, b) => a - b);
    console.log(
      `| ${kind} | ${((sel.length / payloads.length) * 100).toFixed(0)} % (${sel.length}) | ` +
        `${(gaps[gaps.length >> 1] * 100).toFixed(1)} % | ` +
        `${(gaps[Math.floor(gaps.length * 0.9)] * 100).toFixed(1)} % | ` +
        `${((gaps.filter((g) => g < 0.05).length / gaps.length) * 100).toFixed(0)} % |`,
    );
  }
}
