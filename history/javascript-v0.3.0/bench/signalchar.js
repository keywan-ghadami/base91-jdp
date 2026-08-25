// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the choice of signal character costs.
//
// Swapping `"` out of basE91's alphabet for `-` is what makes the output
// JSON-safe, and that is fixed. Which character then sits on value 90 -- and
// so which doubled character becomes the mode signal -- is a free choice.
//
// Since v0.2.0 the signal character is also an R-Set member, so a segment
// substitutes it away and its doubled form can never occur in a payload. Each
// candidate below is measured that way, R-Set membership moving with it, which
// is what makes the rows comparable. It also makes them nearly equal: the
// point of the table is that the choice has stopped mattering.
//
//   node bench/signalchar.js

import { makeCodec, alphabetWithSignal, R_CHARS } from '../src/codec.js';
import { PROFILES } from '../src/profiles.js';
import { CONSTANTS } from '../src/index.js';
import { loadCorpus, measure } from './lib.js';

const corpus = loadCorpus();
const cfg = {
  profiles: PROFILES,
  maskMode: 'exact',
  headerChars: CONSTANTS.HEADER_CHARS,
  minDpBytes: CONSTANTS.MIN_DP_BYTES,
  minBinaryRun: CONSTANTS.MIN_BINARY_RUN,
  maxDpBytes: CONSTANTS.MAX_DP_BYTES,
};

const candidates = ['-', '~', '`', '^', '|', '@', 'Q', '#'];

console.log('### Which doubled character ends a passthrough segment\n');
console.log('| signal | occurrences of the pair in the corpus | text | binary | whole corpus |');
console.log('|---|---|---|---|---|');
const results = [];
for (const ch of candidates) {
  const alphabet = alphabetWithSignal(ch);
  // The signal character is an R-Set member, and a donor may not be the signal
  // character, so both move with the candidate.
  const rChars = R_CHARS.map((c) => (c === 0x2d ? ch.charCodeAt(0) : c));
  const profiles = PROFILES.map((p) => p.replace(ch, '-')).filter((p) => !p.includes(ch));
  if (profiles.length !== PROFILES.length) continue;
  const codec = makeCodec({ ...cfg, alphabet, rChars, profiles });
  const m = measure((b) => codec.encode(b), corpus);
  let pairs = 0;
  const code = ch.charCodeAt(0);
  for (const f of corpus) {
    for (let i = 1; i < f.data.length; i++) {
      if (f.data[i] === code && f.data[i - 1] === code) pairs++;
    }
  }
  results.push({ ch, pairs, m, profiles: profiles.length });
}
const best = Math.min(...results.map((r) => r.m.total));
for (const r of results) {
  const t = r.m.total.toFixed(5);
  console.log(
    `| \`${r.ch}${r.ch}\` | ${r.pairs.toLocaleString('en-US')} | ${r.m.text.toFixed(5)} | ` +
      `${r.m.binary.toFixed(5)} | ${r.m.total === best ? `**${t}**` : t} |`,
  );
}
console.log(
  '\nA candidate that appears in a donor profile takes the place of `-` there,' +
    ' since `-` is an ordinary alphabet character once it is not the signal.',
);

// --- why segments end ------------------------------------------------
const codec = makeCodec(cfg);
console.log('\n### Why a passthrough segment ends\n');
console.log('| sample | segments | bytes in passthrough | signal pair | byte outside the alphabet | no viable profile | lookahead cap |');
console.log('|---|---|---|---|---|---|---|');
const totals = { dpSegments: 0, dpBytes: 0, stops: {} };
for (const f of corpus) {
  const s = codec.encodeStats(f.data);
  totals.dpSegments += s.dpSegments;
  totals.dpBytes += s.dpBytes;
  for (const [k, v] of Object.entries(s.stops)) totals.stops[k] = (totals.stops[k] ?? 0) + v;
  const g = (k) => (s.stops[k] ?? 0).toLocaleString('en-US');
  console.log(
    `| ${f.name} | ${s.dpSegments.toLocaleString('en-US')} | ` +
      `${((s.dpBytes / f.data.length) * 100).toFixed(1)} % | ` +
      `${g('signal')} | ${g('unrepresentable')} | ${g('donor')} | ${g('cap')} |`,
  );
}
const g = (k) => (totals.stops[k] ?? 0).toLocaleString('en-US');
console.log(
  `| whole corpus | ${totals.dpSegments.toLocaleString('en-US')} | ` +
    `${((totals.dpBytes / corpus.reduce((s, f) => s + f.data.length, 0)) * 100).toFixed(1)} % | ` +
    `${g('signal')} | ${g('unrepresentable')} | ${g('donor')} | ${g('cap')} |`,
);
