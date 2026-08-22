// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Upper-bound estimate for what a run-length (Fill) mode would be worth.
//
//   node bench/fillbound.js [minimum run length]
//
// For every maximal run of identical bytes of length >= MIN, assume the run
// could be carried by a 5-character signal covering up to 2048 bytes, and that
// those bytes currently cost what the real encoder spends on them: 1.0 inside
// a passthrough segment, 16/13 in block mode. This overstates the gain (it
// ignores the cost of leaving and re-entering passthrough around a run inside
// one) but bounds it.
import { encode } from '../src/index.js';
import { loadCorpus } from './lib.js';
import { makeCodec } from '../src/codec.js';
import { PROFILES } from '../src/profiles.js';

const corpus = loadCorpus();
const codec = makeCodec({
  profiles: PROFILES,
  maskMode: 'exact',
  headerChars: 2,
  minDpBytes: 26,
  minBinaryRun: 4,
  maxDpBytes: 65536,
});

const MIN = Number(process.argv[2] ?? 5);
const CAP = 2048;
const SIGNAL_COST = 5;

let totIn = 0, totOut = 0, totGain = 0;
console.log(`| sample | now | runs >= ${MIN} | bytes in runs | estimated with Fill | Base85N |`);
console.log('|---|---|---|---|---|---|');

// Base85N's published per-file figures, for the last column only; the tables in
// RESULTS.md measure it live.
const b85 = {
  'sql-wasm.wasm': 1.239, '_cffi_backend.so': 0.965, 'DejaVuSans.ttf': 1.232,
  'requests-2.32.3.tar': 0.767, 'countries.json': 0.935, 'countries.min.json': 1.003,
  'lodash.js': 1.004, 'bootstrap.css': 1.003, 'requests-models.py': 0.973,
  'commonmark-spec.txt': 0.859, 'requests-history.md': 0.979,
  'grace_hopper.jpg': 1.249, 'minduka_present.png': 1.250,
};

for (const f of corpus) {
  const out = encode(f.data).length;
  const st = codec.encodeStats(f.data);
  // What fraction of the file is carried by passthrough, as a proxy for what
  // a run inside it currently costs.
  const dpShare = st.dpBytes / f.data.length;
  const perByteNow = dpShare * 1.0 + (1 - dpShare) * (16 / 13);

  let runs = 0, runBytes = 0, fillCost = 0;
  let i = 0;
  while (i < f.data.length) {
    let j = i + 1;
    while (j < f.data.length && f.data[j] === f.data[i]) j++;
    const len = j - i;
    if (len >= MIN) {
      runs++;
      runBytes += len;
      fillCost += Math.ceil(len / CAP) * SIGNAL_COST;
    }
    i = j;
  }
  const gain = runBytes * perByteNow - fillCost;
  totIn += f.data.length;
  totOut += out;
  totGain += Math.max(0, gain);
  console.log(
    `| ${f.name} | ${(out / f.data.length).toFixed(3)} | ${runs.toLocaleString('en-US')} | ` +
      `${((runBytes / f.data.length) * 100).toFixed(1)} % | ` +
      `${((out - Math.max(0, gain)) / f.data.length).toFixed(3)} | ${b85[f.name]} |`,
  );
}
console.log(
  `| whole corpus | ${(totOut / totIn).toFixed(5)} | | | ` +
    `${((totOut - totGain) / totIn).toFixed(5)} | 1.00698 |`,
);
