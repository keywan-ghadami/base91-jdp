// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// How often each alphabet character occurs in the textual part of the corpus.
// A donor has to be a character the text does not use, so the rarest ones are
// the candidate pool that tools/deriveprofiles.js searches.

import { loadCorpus } from '../bench/lib.js';
import { ALPHABET, R_CHARS, R_NAMES } from '../src/codec.js';

const corpus = loadCorpus();
const counts = new Map([...ALPHABET].map((c) => [c, 0]));
const perFile = new Map([...ALPHABET].map((c) => [c, 0]));
let total = 0;

for (const f of corpus) {
  if (!f.textual) continue;
  const seen = new Set();
  for (const byte of f.data) {
    const ch = String.fromCharCode(byte);
    if (counts.has(ch)) {
      counts.set(ch, counts.get(ch) + 1);
      seen.add(ch);
      total++;
    }
  }
  for (const ch of seen) perFile.set(ch, perFile.get(ch) + 1);
}

const rows = [...counts.entries()]
  .filter(([c]) => c !== '-')
  .sort((a, b) => a[1] - b[1]);

console.log('rarest alphabet characters in the textual corpus');
console.log('char  count        per-mille  files');
for (const [ch, n] of rows.slice(0, 24)) {
  console.log(
    `  ${ch}   ${String(n).padStart(9)}  ${((n / total) * 1000).toFixed(3).padStart(9)}  ${perFile.get(ch)}/8`,
  );
}

console.log('\nR-Set frequency (the bytes DP has to substitute)');
for (let j = 0; j < R_CHARS.length; j++) {
  let n = 0;
  for (const f of corpus) {
    if (!f.textual) continue;
    for (const byte of f.data) if (byte === R_CHARS[j]) n++;
  }
  console.log(`  j=${j} ${R_NAMES[j].padEnd(6)} ${String(n).padStart(9)}`);
}
