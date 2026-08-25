// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Parameter sweeps behind the constants in spec section 6.4. Every number in
// bench/results/RESULTS.md that is a choice rather than a measurement comes
// from here.
//
// Usage:  node bench/sweep.js [name ...]

import { makeCodec } from '../src/codec.js';
import { PROFILES } from '../src/profiles.js';
import { loadCorpus, measure } from './lib.js';

const corpus = loadCorpus();
const base = { profiles: PROFILES, minDpBytes: 26, minBinaryRun: 4, maxDpBytes: 65536 };

function run(cfg) {
  const codec = makeCodec(cfg);
  return measure((b) => codec.encode(b), corpus);
}

function table(title, rows, best = 'total') {
  console.log(`\n### ${title}\n`);
  console.log('| setting | text | binary | whole corpus |');
  console.log('|---|---|---|---|');
  let bestVal = Infinity;
  for (const r of rows) bestVal = Math.min(bestVal, r.m[best]);
  for (const r of rows) {
    const mark = (v, k) => (r.m[best] === bestVal && k === best ? `**${v}**` : v);
    console.log(
      `| ${r.label} | ${mark(r.m.text.toFixed(5), 'text')} | ` +
        `${mark(r.m.binary.toFixed(5), 'binary')} | ${mark(r.m.total.toFixed(5), 'total')} |`,
    );
  }
}

const SWEEPS = {
  minBinaryRun() {
    const rows = [];
    for (const v of [1, 2, 3, 4, 5, 6, 8, 12, 16, 24, 32]) {
      rows.push({ label: `\`MIN_BINARY_RUN\` = ${v}`, m: run({ ...base, minBinaryRun: v }) });
    }
    table('MIN_BINARY_RUN: block-mode bytes before passthrough may resume', rows);
  },

  minDpBytes() {
    const rows = [];
    for (const v of [16, 18, 20, 22, 23, 24, 25, 26, 27, 28, 30, 32, 36, 40, 48]) {
      rows.push({ label: `\`MIN_DP_BYTES\` = ${v}`, m: run({ ...base, minDpBytes: v }) });
    }
    table('MIN_DP_BYTES: shortest segment worth a passthrough signal', rows);
  },

  maxDpBytes() {
    const rows = [];
    for (const v of [256, 512, 1024, 2048, 4096, 8192, 16384, 65536, 1 << 28]) {
      const label = v === 1 << 28 ? 'unbounded' : `\`MAX_DP_BYTES\` = ${v}`;
      rows.push({ label, m: run({ ...base, maxDpBytes: v }) });
    }
    table('MAX_DP_BYTES: the encoder lookahead bound', rows);
  },

  profiles() {
    const rows = [];
    for (const n of [1, 2, 3, 4, 6, 8, 10, 12]) {
      if (n > PROFILES.length) break;
      rows.push({
        label: `${n} profile${n > 1 ? 's' : ''}`,
        m: run({ ...base, profiles: PROFILES.slice(0, n) }),
      });
    }
    table('NUM_PROFILES: donor rankings the header can select', rows);
  },

  header() {
    // Each variant carries a different entry cost, so each gets its own best
    // MIN_DP_BYTES before they are compared.
    const variants = [
      ['2 chars, exact mask, 4 profiles (this version)', { headerChars: 2, maskMode: 'exact', profiles: PROFILES }],
      ['2 chars, exact mask, 1 profile', { headerChars: 2, maskMode: 'exact', profiles: PROFILES.slice(0, 1) }],
      ['2 chars, prefix mask, 4 profiles', { headerChars: 2, maskMode: 'prefix', profiles: PROFILES }],
      ['1 char, prefix mask, 4 profiles', { headerChars: 1, maskMode: 'prefix', profiles: PROFILES }],
      ['1 char, prefix mask, 1 profile', { headerChars: 1, maskMode: 'prefix', profiles: PROFILES.slice(0, 1) }],
      ['1 char, no mask, 4 profiles', { headerChars: 1, maskMode: 'none', profiles: PROFILES }],
      ['1 char, no mask, 1 profile', { headerChars: 1, maskMode: 'none', profiles: PROFILES.slice(0, 1) }],
    ];
    const rows = [];
    for (const [label, cfg] of variants) {
      let best = null;
      for (let m = 16; m <= 40; m++) {
        const r = run({ ...base, ...cfg, minDpBytes: m });
        if (!best || r.total < best.m.total) best = { m: r, minDp: m };
      }
      rows.push({ label: `${label} — best \`MIN_DP_BYTES\` ${best.minDp}`, m: best.m });
    }
    table('Header width: what the passthrough signal carries', rows);
  },
};

const names = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(SWEEPS);
for (const n of names) {
  if (!SWEEPS[n]) throw new Error(`unknown sweep ${n}`);
  SWEEPS[n]();
}
