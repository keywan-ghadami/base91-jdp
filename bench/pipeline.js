// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the pipeline does that a size table cannot show: which mode each file
// lands in, how fast each layer runs, how much the side channel actually
// carries, and where the marker starts paying for itself.
//
//   node bench/pipeline.js

import { encode, decode, decodeDetailed, CONSTANTS } from '../src/index.js';
import { compress, decompress } from '../src/lz4.js';
import { encodeFrame, decodeFrame, frameSegments, SEGMENT_BYTES } from '../src/frame.js';
import {
  charsFromSymbols, pairsFromChars, symbolsFromBytes,
  carriesSide, lowerSide, SIDE_COUNT, SIDE_MIX, SYMBOL_MAX, SEPARATOR_VALUE,
} from '../src/pack.js';
import { loadCorpus } from './lib.js';

const corpus = loadCorpus();
const MB = 1 << 20;

const time = (bytes, runs, fn) => {
  fn(); // warm
  const t0 = process.hrtime.bigint();
  for (let i = 0; i < runs; i++) fn();
  const ms = Number(process.hrtime.bigint() - t0) / 1e6 / runs;
  return { ms, rate: bytes / MB / (ms / 1000) };
};

// ---------------------------------------------------------------------
console.log('### Which mode each file lands in\n');
console.log('| sample | input | mode | segments | chars/byte |');
console.log('|---|---|---|---|---|');
let totalIn = 0;
let totalOut = 0;
for (const f of corpus) {
  const text = encode(f.data);
  const seen = decodeDetailed(text);
  totalIn += f.data.length;
  totalOut += text.length;
  console.log(
    `| ${f.name} | ${f.data.length.toLocaleString('en-US')} B | ` +
      `${seen.framed ? seen.mode : 'headerless'} | ${seen.segments} | ` +
      `${(text.length / f.data.length).toFixed(4)} |`,
  );
}
console.log(
  `| **whole corpus** | ${totalIn.toLocaleString('en-US')} B | | | ` +
    `**${(totalOut / totalIn).toFixed(5)}** |`,
);

// ---------------------------------------------------------------------
console.log('\n### Where the marker starts paying for itself\n');
console.log('A framed stream costs two characters that a headerless one does not.');
console.log('Below some size the compressor cannot make them back. That size is not');
console.log('a constant in the format -- `encode` compares the two candidates and');
console.log('takes the shorter -- so this is a measurement, not a threshold.\n');
console.log('| payload | text | JSON | source | random bytes |');
console.log('|---|---|---|---|---|');
const SAMPLES = {
  text: (n) => new TextEncoder().encode('the quick brown fox jumps over the lazy dog. '.repeat(Math.ceil(n / 45))).subarray(0, n),
  json: (n) => new TextEncoder().encode(JSON.stringify(Array.from({ length: Math.ceil(n / 24) }, (_, i) => ({ id: i, name: `item ${i}` })))).subarray(0, n),
  source: (n) => new TextEncoder().encode('function f(x) { return x * 2; }\n'.repeat(Math.ceil(n / 32))).subarray(0, n),
  random: (n) => {
    let s = 9;
    return Uint8Array.from({ length: n }, () => ((s = (s * 1103515245 + 12345) >>> 0) >>> 24));
  },
};
for (const n of [16, 32, 64, 128, 256, 512, 1024, 4096]) {
  const cells = Object.values(SAMPLES).map((make) => {
    const data = make(n);
    return decodeDetailed(encode(data)).framed ? 'framed' : 'headerless';
  });
  console.log(`| ${n} B | ${cells.join(' | ')} |`);
}

// ---------------------------------------------------------------------
console.log('\n### Side channel, on real data\n');
console.log('| sample | symbols | slots | rate | bits per segment |');
console.log('|---|---|---|---|---|');
for (const f of corpus.filter((x) => x.data.length > 200000)) {
  const pairs = encodeFrame(f.data, { compress: true, protect: true });
  let slots = 0;
  let separators = 0;
  for (const v of pairs) {
    if (v === SEPARATOR_VALUE) {
      separators++;
      continue;
    }
    if (carriesSide(v >= SYMBOL_MAX ? lowerSide(v) : v)) slots++;
  }
  const symbols = pairs.length - separators;
  console.log(
    `| ${f.name} | ${symbols.toLocaleString('en-US')} | ${slots.toLocaleString('en-US')} | ` +
      `${((slots / symbols) * 100).toFixed(3)} % | ` +
      `${Math.round(slots / (separators + 1)).toLocaleString('en-US')} |`,
  );
}
console.log(
  `\nThe window is ${SIDE_COUNT} of ${SYMBOL_MAX} symbol values, scattered by ` +
    `v * ${SIDE_MIX} mod ${SYMBOL_MAX}, so uniform data would give ` +
    `${((SIDE_COUNT / SYMBOL_MAX) * 100).toFixed(3)} % and real data gives more.`,
);
console.log('Those bits cost no characters at all: a symbol in the window is written');
console.log('as its reserved pair value, and the pair is still two characters wide.');

// ---------------------------------------------------------------------
console.log('\n### Throughput, by layer\n');
const sample = corpus.reduce((a, b) => (a.data.length > b.data.length ? a : b));
const data = sample.data;
const n = data.length;
console.log(`Measured on ${sample.name}, ${n.toLocaleString('en-US')} bytes.\n`);
console.log('| layer | encode | decode |');
console.log('|---|---|---|');

const block = compress(data);
const segments = frameSegments(data, true);
const pairsProtected = encodeFrame(segments, { compress: true, protect: true });
const pairsChecked = encodeFrame(segments, { compress: true, protect: false });
const textProtected = charsFromSymbols(pairsProtected);
const encoded = encode(data);

const row = (label, enc, dec) =>
  console.log(`| ${label} | ${enc ? `${enc.rate.toFixed(1)} MB/s` : '--'} | ${dec ? `${dec.rate.toFixed(1)} MB/s` : '--'} |`);

row('LZ4 alone', time(n, 3, () => compress(data)), time(n, 3, () => decompress(block, n)));
row('bytes to symbols', time(n, 3, () => symbolsFromBytes(block)), null);
row(
  'frame, check only',
  time(n, 3, () => encodeFrame(segments, { compress: true, protect: false })),
  time(n, 3, () => decodeFrame(pairsChecked, { compress: true, protect: false })),
);
row(
  'frame with Reed-Solomon',
  time(n, 3, () => encodeFrame(segments, { compress: true, protect: true })),
  time(n, 3, () => decodeFrame(pairsProtected, { compress: true, protect: true })),
);
row(
  'symbols to characters',
  time(n, 3, () => charsFromSymbols(pairsProtected)),
  time(n, 3, () => pairsFromChars(textProtected)),
);
row('whole pipeline', time(n, 3, () => encode(data)), time(n, 3, () => decode(encoded)));
row(
  'whole pipeline, protected',
  time(n, 3, () => encode(data, { protect: true })),
  time(n, 3, () => decode(encode(data, { protect: true }))),
);

console.log(
  `\nSegments are ${(SEGMENT_BYTES / 1024).toFixed(0)} KiB of payload and codewords are ` +
    `${CONSTANTS.RS_DATA} data symbols plus ${CONSTANTS.RS_PARITY} parity, ` +
    `so the parity costs ${((CONSTANTS.RS_PARITY / (CONSTANTS.RS_DATA + CONSTANTS.RS_PARITY)) * 100).toFixed(3)} %.`,
);
