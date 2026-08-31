// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Derives the donor profile table of spec section 4.2.
//
// A profile is an ordered ranking of seven alphabet characters. A segment
// whose mask has k bits set spends the first k of them as stand-ins, so only
// those k become unusable as literals inside it -- which is why the table is
// searched by encoded size and not by character frequency: what matters is
// how often a donor's own character turns up in the text it would have to
// stand in for.
//
// The search is greedy in two nested ways. Profiles are added one at a time,
// each one chosen to help most given the ones already in the table; within a
// profile, donor positions are filled left to right. Position r only has an
// effect on segments with k > r, and k > 3 is rare, so the later positions
// mostly fall back to the rarity order -- which is the right default anyway.
//
// Usage:  node tools/deriveprofiles.js [count]

import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { makeCodec, ALPHABET, R_LEN } from '../src/codec.js';
import { BENCH_DIR } from '../bench/lib.js';

// Like CORPUS_DIR, the training data stays at the repository root: it is
// written by tools/traincorpus.py, which derives a table the current
// specification still carries.
const TRAIN_DIR = join(BENCH_DIR, '..', '..', '..', 'bench', 'train');
const WANT = Number(process.argv[2] ?? 32);
const POOL_SIZE = 20;

const files = readdirSync(TRAIN_DIR)
  .filter((n) => n.endsWith('.train'))
  .sort()
  .map((n) => new Uint8Array(readFileSync(join(TRAIN_DIR, n))));
if (!files.length) {
  throw new Error(`no training data -- run: python3 tools/traincorpus.py`);
}
const totalIn = files.reduce((s, f) => s + f.length, 0);
process.stderr.write(`training on ${files.length} files, ${totalIn} bytes\n`);

// Candidate pool: the rarest alphabet characters in the training text.
const counts = new Map([...ALPHABET].map((c) => [c, 0]));
for (const f of files) {
  for (const b of f) {
    const ch = String.fromCharCode(b);
    if (counts.has(ch)) counts.set(ch, counts.get(ch) + 1);
  }
}
// Letters and digits are excluded from the pool on principle rather than by
// frequency. A rare capital is rare when all text is counted together and
// common in the one file that happens to use it -- identifiers and words are
// made of letters -- so a letter donor breaks segments in bursts. Measured,
// the difference runs the wrong way on the training corpus and the right way
// on the hold-out: letters win by 0.1 % where they were derived and lose by
// 0.1 % where they were not (bench/results/RESULTS.md).
const pool = [...counts.entries()]
  .filter(([c]) => c !== '-' && !/[0-9A-Za-z]/.test(c))
  .sort((a, b) => a[1] - b[1])
  .slice(0, POOL_SIZE)
  .map(([c]) => c);
process.stderr.write(`pool: ${pool.join(' ')}\n`);

const cost = (profiles) => {
  const codec = makeCodec({ profiles, minDpBytes: 24, minBinaryRun: 1 });
  let n = 0;
  for (const f of files) n += codec.encode(f).length;
  return n;
};

const rarityFill = (prefix) => {
  const out = [...prefix];
  for (const c of pool) {
    if (out.length === R_LEN) break;
    if (!out.includes(c)) out.push(c);
  }
  return out;
};

const table = [];
const curve = [];
for (let p = 0; p < WANT; p++) {
  const chosen = [];
  for (let r = 0; r < R_LEN; r++) {
    let best = null;
    let bestCost = Infinity;
    for (const c of pool) {
      if (chosen.includes(c)) continue;
      const cand = rarityFill([...chosen, c]);
      const n = cost([...table, cand]);
      if (n < bestCost) {
        bestCost = n;
        best = c;
      }
    }
    chosen.push(best);
  }
  const profile = rarityFill(chosen);
  table.push(profile);
  const n = cost(table);
  curve.push(n);
  process.stderr.write(
    `profile ${String(p).padStart(2)}  ${profile.join(' ')}   ` +
      `${n}  (${(n / totalIn).toFixed(5)})\n`,
  );
}

console.log('\n// derived by tools/deriveprofiles.js');
console.log('export const PROFILES = [');
for (const p of table) console.log(`  ${JSON.stringify(p.join(''))},`);
console.log('];');

console.log('\n| profiles | encoded chars | chars per byte | gain |');
console.log('|---|---|---|---|');
for (let i = 0; i < curve.length; i++) {
  const gain = i === 0 ? '' : (((curve[i - 1] - curve[i]) / totalIn) * 100).toFixed(4) + ' %';
  console.log(
    `| ${i + 1} | ${curve[i]} | ${(curve[i] / totalIn).toFixed(5)} | ${gain} |`,
  );
}
