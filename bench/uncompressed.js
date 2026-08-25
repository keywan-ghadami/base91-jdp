// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Can base91-jdp beat Base85N with no compressor at all?
//
// Base85N's uncompressed advantage over this format comes from one construct
// this format does not have: a fill signal, five characters for a run that
// would otherwise cost hundreds. The 0.4 draft dropped it (its section 18.2)
// on the grounds that compression covers the same redundancy -- true, and
// beside the point when the comparison is explicitly without a compressor.
//
// This prices what putting it back would buy, together with the packed
// classes of the draft's section 9, against the real Base85N implementation.
// The estimate is deliberately conservative in three ways:
//
//   * stretches between segments are costed with THIS repository's 0.3.0
//     headerless encoder, which pays a two-character exit signal per
//     passthrough segment and spends a donor on '-'. The draft removes both.
//   * every stretch restarts the encoder, so its pending bits and its
//     passthrough state are given up at each boundary.
//   * the scan is greedy left to right, never backtracking.
//
// So the number below is a floor on what a conforming 0.4 encoder produces,
// not a ceiling.
//
//   node bench/uncompressed.js [--group core|silesia|all] [--no-run] [--no-packed]

import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { makeCodec } from '../src/codec.js';
import { PROFILES } from '../src/profiles.js';
import { BENCH_DIR, CORPUS_DIR } from './lib.js';

// --nul swaps '-' out of the R-Set for NUL. In 0.3.0 '-' had to be a member,
// because a doubled hyphen was the exit signal; the 0.4 draft delimits by
// length and frees the slot. NUL is the byte that breaks passthrough over the
// string tables of an object file, which is where Base85N's fill wins here.
// This measures size only: under 0.3.0 framing the result would not round
// trip, because a literal '--' in a payload still ends a segment there.
const R_NUL = [0x20, 0x22, 0x0a, 0x5c, 0x0d, 0x27, 0x09, 0x00];
const codec = makeCodec({
  profiles: PROFILES, maskMode: 'exact', headerChars: 2,
  minDpBytes: 26, minBinaryRun: 4, maxDpBytes: 65536,
  ...(process.argv.includes('--nul') ? { rChars: R_NUL } : {}),
});

const arg = (n, d) => { const i = process.argv.indexOf(n); return i >= 0 ? process.argv[i + 1] : d; };
const GROUP = arg('--group', 'core');
const USE_RUN = !process.argv.includes('--no-run');
const USE_PACKED = !process.argv.includes('--no-packed');
const MAX_SEG = 65536;
// Shortest zero run worth a segment. Three characters carry any run up to 89,
// so it undercuts block mode from two bytes up where the flush is free.
const ZMIN = Number(arg('--zmin', '2'));

const blockChars = (n) => 2 * Math.ceil((8 * n) / 13);
const lengthChars = (n) => (n < 90 ? 1 : n < 8370 ? 3 : 7);

// The packed classes of the draft's section 9, as byte-membership tables.
const CLASSES = [
  ['DEC', 4, '0123456789'],
  ['HEXL', 4, '0123456789abcdef'],
  ['HEXU', 4, '0123456789ABCDEF'],
  ['HEXL_D', 5, '0123456789abcdef-'],
  ['HEXU_D', 5, '0123456789ABCDEF-'],
  ['ALPHA_L', 5, 'abcdefghijklmnopqrstuvwxyz'],
  ['ALPHA_U', 5, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'],
  ['B32', 5, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'],
  ['B64', 6, 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'],
  ['B64U', 6, 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'],
  ['ALNUM', 6, '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'],
].map(([name, w, chars]) => {
  const member = new Uint8Array(256);
  for (const c of chars) member[c.charCodeAt(0)] = 1;
  return { name, w, member };
});

// Segment overhead: the signal, plus the flush field. Block mode restarts at
// n = 0 after every segment, so a segment that begins where the previous one
// ended owes no pending bits and its flush field is empty. Anywhere else the
// flush is charged at its maximum of two characters, in our disfavour.
const signalChars = (adjacent) => (adjacent ? 2 : 4);
const packedChars = (L, w, adj) => signalChars(adj) + lengthChars(L) + 2 * Math.ceil((L * w) / 13);
// A run: signal, flush, the run length, and one pair naming the byte.
const runChars = (L, adj) => signalChars(adj) + lengthChars(L) + 2;
// A zero run needs no byte value: the class is the value. Three characters
// carry up to 89 zero bytes, five up to 8 369 -- where Base85N's fill spends
// five on at most 2 048 and always carries the byte it repeats.
const zeroRunChars = (L, adj) => signalChars(adj) + lengthChars(L);

/**
 * Greedy segmentation. Returns the character count and which classes paid.
 * Everything not claimed by a segment goes through the 0.3.0 encoder.
 */
function encodeChars(data) {
  let chars = 0;
  let plainFrom = 0;
  const used = new Map();
  const flushPlain = (to) => {
    if (to > plainFrom) chars += codec.encode(data.subarray(plainFrom, to)).length;
  };

  for (let i = 0; i < data.length; ) {
    let best = null;
    const adj = plainFrom === i && i > 0;

    if (USE_RUN) {
      const b = data[i];
      let j = i + 1;
      while (j < data.length && data[j] === b && j - i < MAX_SEG) j++;
      const L = j - i;
      // Worth it only against the cheaper of block mode and passthrough: a run
      // of spaces costs one character per byte in passthrough, not 1.23.
      if (L >= (b === 0 ? ZMIN : 4)) {
        const alt = Math.min(blockChars(L), L);
        const zero = b === 0;
        const cost = zero ? zeroRunChars(L, adj) : runChars(L, adj);
        if (cost < alt) best = { name: zero ? 'ZRUN' : 'RUN', L, cost };
      }
    }

    if (USE_PACKED) {
      for (const { name, w, member } of CLASSES) {
        if (!member[data[i]]) continue;
        let j = i;
        while (j < data.length && member[data[j]] && j - i < MAX_SEG) j++;
        const L = j - i;
        if (L < 5) continue;
        const cost = packedChars(L, w, adj);
        // A packed stretch competes with block mode; passthrough would carry
        // these bytes at 1.0, so charge against the cheaper of the two.
        const alt = Math.min(blockChars(L), L);
        if (cost < alt && (!best || cost - alt < best.cost - Math.min(blockChars(best.L), best.L))) {
          best = { name, L, cost };
        }
      }
    }

    if (best) {
      flushPlain(i);
      chars += best.cost;
      used.set(best.name, (used.get(best.name) ?? 0) + best.L);
      i += best.L;
      plainFrom = i;
    } else {
      i++;
    }
  }
  flushPlain(data.length);
  return { chars, used };
}

function files(group) {
  const dirs = group === 'silesia' ? [join(CORPUS_DIR, 'silesia')]
    : group === 'core' ? [CORPUS_DIR] : [CORPUS_DIR, join(CORPUS_DIR, 'silesia')];
  const out = [];
  for (const dir of dirs) {
    for (const name of readdirSync(dir).sort()) {
      const p = join(dir, name);
      if (statSync(p).isFile()) out.push({ name, path: p });
    }
  }
  return out;
}

function base85n(paths) {
  const out = execFileSync('go', ['run', '.', ...paths], {
    cwd: join(BENCH_DIR, 'base85n'), encoding: 'utf8', maxBuffer: 1 << 28,
  });
  const m = new Map();
  for (const line of out.trim().split('\n')) {
    const [p, , enc] = line.split('\t');
    m.set(p.split('/').pop(), Number(enc));
  }
  return m;
}

const corpus = files(GROUP);
const b85 = base85n(corpus.map((f) => f.path));

console.log(`### No compressor on either side -- group "${GROUP}"\n`);
console.log('| sample | input | Base85N | jdp 0.3 | jdp 0.4 proj. | vs Base85N | classes that paid |');
console.log('|---|---|---|---|---|---|---|');
let inB = 0, s85 = 0, s03 = 0, s04 = 0;
for (const f of corpus) {
  const data = new Uint8Array(readFileSync(f.path));
  const old = codec.encode(data).length;
  const { chars, used } = encodeChars(data);
  inB += data.length; s85 += b85.get(f.name); s03 += old; s04 += chars;
  const top = [...used.entries()].sort((a, b) => b[1] - a[1]).slice(0, 2)
    .map(([n, L]) => `${n} ${(100 * L / data.length).toFixed(0)} %`).join(', ');
  const win = (1 - chars / b85.get(f.name)) * 100;
  console.log(`| ${f.name} | ${data.length.toLocaleString('en-US')} B | ${(b85.get(f.name) / data.length).toFixed(4)} | ` +
    `${(old / data.length).toFixed(4)} | **${(chars / data.length).toFixed(4)}** | ` +
    `${win >= 0 ? '+' : ''}${win.toFixed(1)} % | ${top || '--'} |`);
}
const win = (1 - s04 / s85) * 100;
console.log(`| **whole group** | ${inB.toLocaleString('en-US')} B | ${(s85 / inB).toFixed(5)} | ${(s03 / inB).toFixed(5)} | ` +
  `**${(s04 / inB).toFixed(5)}** | ${win >= 0 ? '+' : ''}${win.toFixed(1)} % | |`);
