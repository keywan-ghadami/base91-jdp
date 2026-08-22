// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The size benchmark behind bench/results/RESULTS.md.
//
//   node bench/bench.js            raw and JSON-embedded ratios, per file
//   node bench/bench.js --json     machine-readable
//
// Base85N sizes come from bench/base85n (the upstream Go implementation,
// v0.5.1) when Go is available; without it that column is left out rather
// than filled in from its documentation.

import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { encode, decode } from '../src/index.js';
import { loadCorpus, BENCH_DIR, CORPUS_DIR } from './lib.js';
import {
  base91Encode,
  base91Decode,
  base64Encode,
  ascii85Encode,
  jsonEmbeddedLength,
} from './refcodecs.js';

const corpus = loadCorpus();

function base85nSizes() {
  try {
    const out = execFileSync(
      'go',
      ['run', '.', ...corpus.map((f) => join(CORPUS_DIR, f.name))],
      { cwd: join(BENCH_DIR, 'base85n'), encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
    );
    const map = new Map();
    for (const line of out.trim().split('\n')) {
      const [path, , enc] = line.split('\t');
      map.set(path.split('/').pop(), Number(enc));
    }
    return map;
  } catch (err) {
    process.stderr.write(`Base85N column skipped: ${err.message.split('\n')[0]}\n`);
    return null;
  }
}

const b85 = base85nSizes();

const CODECS = [
  { key: 'base64', label: 'Base64', encode: base64Encode },
  { key: 'ascii85', label: 'Ascii85', encode: ascii85Encode },
  { key: 'base91', label: 'basE91', encode: base91Encode, decode: base91Decode },
  ...(b85 ? [{ key: 'base85n', label: 'Base85N', sizes: b85, jsonSafe: true }] : []),
  { key: 'jdp', label: 'base91-jdp', encode, decode, jsonSafe: true },
];

const rows = [];
for (const f of corpus) {
  const row = { name: f.name, category: f.category, textual: f.textual, size: f.data.length, c: {} };
  for (const c of CODECS) {
    let raw, embedded;
    if (c.sizes) {
      raw = c.sizes.get(f.name);
      embedded = raw; // escape-free by construction
    } else {
      const text = c.encode(f.data);
      raw = text.length;
      embedded = jsonEmbeddedLength(text);
      if (c.jsonSafe && embedded !== raw) {
        throw new Error(`${c.label} produced output that JSON has to escape`);
      }
      if (c.decode) {
        const back = c.decode(text);
        if (back.length !== f.data.length || !back.every((v, i) => v === f.data[i])) {
          throw new Error(`${c.label} does not round-trip ${f.name}`);
        }
      }
    }
    row.c[c.key] = { raw, embedded };
  }
  rows.push(row);
}

function rollup(filter) {
  const sel = rows.filter(filter);
  const inBytes = sel.reduce((s, r) => s + r.size, 0);
  const out = {};
  for (const c of CODECS) {
    out[c.key] = {
      raw: sel.reduce((s, r) => s + r.c[c.key].raw, 0) / inBytes,
      embedded: sel.reduce((s, r) => s + r.c[c.key].embedded, 0) / inBytes,
    };
  }
  return { inBytes, ...out };
}

const groups = {
  text: rollup((r) => r.textual),
  binary: rollup((r) => !r.textual),
  whole: rollup(() => true),
};

if (process.argv.includes('--json')) {
  console.log(JSON.stringify({ rows, groups }, null, 2));
} else {
  const head = (which) => {
    console.log(`| sample | input | ${CODECS.map((c) => c.label).join(' | ')} |`);
    console.log(`|---|---|${CODECS.map(() => '---').join('|')}|`);
    for (const r of rows) {
      const best = Math.min(...CODECS.map((c) => r.c[c.key][which]));
      const cells = CODECS.map((c) => {
        const v = (r.c[c.key][which] / r.size).toFixed(3);
        return r.c[c.key][which] === best ? `**${v}**` : v;
      });
      console.log(`| ${r.name} | ${r.size.toLocaleString('en-US')} B | ${cells.join(' | ')} |`);
    }
    for (const [name, g] of Object.entries(groups)) {
      const label = { text: 'text files', binary: 'binary files', whole: 'whole corpus' }[name];
      const best = Math.min(...CODECS.map((c) => g[c.key][which]));
      const cells = CODECS.map((c) => {
        const v = g[c.key][which].toFixed(5);
        return g[c.key][which] === best ? `**${v}**` : v;
      });
      console.log(`| ${label} | ${g.inBytes.toLocaleString('en-US')} B | ${cells.join(' | ')} |`);
    }
  };

  console.log('### Encoded characters per input byte\n');
  head('raw');
  console.log('\n### Characters per input byte once the output sits in a JSON string\n');
  head('embedded');
}
