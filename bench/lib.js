// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export const BENCH_DIR = dirname(fileURLToPath(import.meta.url));
export const CORPUS_DIR = join(BENCH_DIR, 'corpus');

// The benchmark corpus is the one base85n publishes its numbers on: 13 real
// files, 6.52 MB, fetched by bench/corpus.py from pinned upstream archives.
// Using it unchanged is what makes the comparison in RESULTS.md a comparison.
export const CATEGORY = {
  'sql-wasm.wasm': 'binary',
  '_cffi_backend.so': 'binary',
  'DejaVuSans.ttf': 'binary',
  'requests-2.32.3.tar': 'archive',
  'countries.json': 'json',
  'countries.min.json': 'json',
  'lodash.js': 'code',
  'bootstrap.css': 'code',
  'requests-models.py': 'code',
  'commonmark-spec.txt': 'spec',
  'requests-history.md': 'prose',
  'grace_hopper.jpg': 'image',
  'minduka_present.png': 'image',
};

// "text" and "binary" as base85n's results table splits them.
export const TEXTUAL = new Set(['json', 'code', 'spec', 'prose']);

export function loadCorpus() {
  if (!existsSync(CORPUS_DIR)) {
    throw new Error(
      `corpus missing -- run: python3 ${join(BENCH_DIR, 'corpus.py')}`,
    );
  }
  const names = readdirSync(CORPUS_DIR).filter((n) => CATEGORY[n]);
  names.sort(
    (a, b) =>
      Object.keys(CATEGORY).indexOf(a) - Object.keys(CATEGORY).indexOf(b),
  );
  return names.map((name) => ({
    name,
    category: CATEGORY[name],
    textual: TEXTUAL.has(CATEGORY[name]),
    data: new Uint8Array(readFileSync(join(CORPUS_DIR, name))),
  }));
}

/** Encoded characters per input byte, per file and rolled up. */
export function measure(encode, corpus) {
  const per = [];
  let text = { in: 0, out: 0 };
  let bin = { in: 0, out: 0 };
  for (const f of corpus) {
    const out = encode(f.data).length;
    per.push({ ...f, out, ratio: out / f.data.length });
    const bucket = f.textual ? text : bin;
    bucket.in += f.data.length;
    bucket.out += out;
  }
  return {
    per,
    text: text.out / text.in,
    binary: bin.out / bin.in,
    total: (text.out + bin.out) / (text.in + bin.in),
  };
}

export const fmt = (x, d = 5) => x.toFixed(d);
