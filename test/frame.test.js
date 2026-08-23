// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import { encode, decode, decodeDetailed, ALPHABET, ERR } from '../src/index.js';
import {
  encodeFrame, decodeFrame, frameSegments, frameChars, SEGMENT_BYTES, RS_PARITY,
} from '../src/frame.js';
import {
  charsFromSymbols, pairsFromChars, carriesSide, countSideSlots, raiseSide, lowerSide,
  SIDE_COUNT, SIDE_MAX, SIDE_MIX, SIDE_UNMIX, SYMBOL_MAX, SEPARATOR_VALUE,
} from '../src/pack.js';
import { rng, bytes } from './helpers.js';

const MODES = [
  { compress: true, protect: true },
  { compress: true, protect: false },
  { compress: false, protect: true },
  { compress: false, protect: false },
];

const label = (m) => `${m.compress ? 'lz4' : 'stored'}${m.protect ? '+rs' : ''}`;

test('every mode round-trips, at every awkward length', () => {
  const r = rng(61);
  const lengths = [0, 1, 12, 13, 14, 200, 4091 * 13, 4092 * 13, 4093 * 13];
  for (const m of MODES) {
    for (const n of lengths) {
      const input = bytes(n, () => Math.floor(r() * 256));
      const got = decodeFrame(encodeFrame(input, m), m);
      assert.equal(got.damaged.length, 0, `${label(m)} at ${n}: ${JSON.stringify(got.damaged)}`);
      assert.deepEqual(Array.from(got.bytes), Array.from(input), `${label(m)} at ${n}`);
    }
  }
});

test('round trips across the segment boundary', () => {
  const r = rng(62);
  for (const n of [SEGMENT_BYTES - 1, SEGMENT_BYTES, SEGMENT_BYTES + 1, SEGMENT_BYTES * 2 + 77]) {
    const input = bytes(n, (i) => (i * 31) & 0xff ^ Math.floor(r() * 4));
    for (const m of MODES) {
      const got = decodeFrame(encodeFrame(input, m), m);
      assert.equal(got.damaged.length, 0, `${label(m)} at ${n}`);
      assert.deepEqual(Array.from(got.bytes), Array.from(input), `${label(m)} at ${n}`);
    }
  }
});

test('frameChars predicts exactly what encodeFrame builds', () => {
  // The size comparison in encode() decides between framed and headerless
  // before building either. If this prediction were only close, the decision
  // would sometimes be wrong and nothing would ever say so.
  const r = rng(63);
  for (let n = 0; n <= 400; n++) {
    const input = bytes(n, () => Math.floor(r() * 256));
    for (const m of MODES) {
      const segments = frameSegments(input, m.compress);
      assert.equal(
        frameChars(segments, m.protect),
        charsFromSymbols(encodeFrame(segments, m)).length,
        `${label(m)} at ${n} bytes`,
      );
    }
  }
});

test('a separator appears between segments and nowhere else', () => {
  const input = bytes(SEGMENT_BYTES * 3 + 100, (i) => (i * 17) & 0xff);
  for (const m of MODES) {
    const pairs = encodeFrame(input, m);
    let separators = 0;
    for (const v of pairs) {
      if (v === SEPARATOR_VALUE) separators++;
      else assert.ok(v < SEPARATOR_VALUE, `a pair of ${v} is above the separator`);
    }
    assert.equal(separators, 3, `${label(m)}: expected three separators`);
  }
});

test('the side channel raises symbols without moving a single character', () => {
  const r = rng(64);
  // Uniform bytes are the floor for the side channel: the window is 88 of
  // 8192 symbol values, so a flat distribution gives 1.07 % and real data
  // gives far more, because its symbols crowd the bottom of the range.
  const input = bytes(200000, () => Math.floor(r() * 256));
  const m = { compress: false, protect: true };
  const pairs = encodeFrame(input, m);
  let raised = 0;
  let slots = 0;
  for (const v of pairs) {
    if (v === SEPARATOR_VALUE) continue;
    if (v >= SYMBOL_MAX) raised++;
    if (carriesSide(v >= SYMBOL_MAX ? lowerSide(v) : v)) slots++;
  }
  assert.ok(slots > 0, 'no side-channel slots at all');
  const rate = slots / pairs.length;
  assert.ok(rate > 0.008 && rate < 0.016, `slot rate ${rate} on uniform bytes`);
  assert.ok(raised > slots * 0.3 && raised < slots * 0.7, `${raised} raised of ${slots} slots`);
  // Raising never reaches the separator, and every raised value comes back.
  assert.equal(SIDE_MAX, SEPARATOR_VALUE - 1);
  assert.equal(SIDE_COUNT, SEPARATOR_VALUE - SYMBOL_MAX);
  let inWindow = 0;
  for (let v = 0; v < SYMBOL_MAX; v++) {
    if (!carriesSide(v)) continue;
    inWindow++;
    assert.equal(lowerSide(raiseSide(v)), v, `${v} does not survive the round trip`);
    assert.ok(raiseSide(v) < SEPARATOR_VALUE, `${v} raises onto the separator`);
  }
  assert.equal(inWindow, SIDE_COUNT);
  // A damaged separator must still land on a symbol the field has.
  assert.ok(lowerSide(SEPARATOR_VALUE) < SYMBOL_MAX);
});

test('the side channel does not collapse on any shape of input', () => {
  // The reason the window is a stride and not a contiguous run. Thirteen-bit
  // symbols are nothing like uniform, and a contiguous window measured 0.000 %
  // on some of these -- a check that can vanish is not a check. The floor here
  // is deliberately well under the 0.834 % actually measured, so that this
  // fails on a real regression rather than on noise.
  const enc = new TextEncoder();
  const shapes = {
    'repeated text': enc.encode('the quick brown fox jumps over the lazy dog. '.repeat(3000)),
    prose: enc.encode(
      Array.from({ length: 4000 }, (_, i) => `line ${i}: ordinary prose about nothing much.`).join('\n'),
    ),
    json: enc.encode(JSON.stringify(Array.from({ length: 8000 }, (_, i) => ({ id: i, name: `item ${i}` })))),
    uniform: bytes(200000, (i) => ((i * 1103515245 + 12345) >>> 24) & 0xff),
  };
  for (const [name, data] of Object.entries(shapes)) {
    for (const compress of [false, true]) {
      const pairs = encodeFrame(data, { compress, protect: false });
      let slots = 0;
      let n = 0;
      for (const v of pairs) {
        if (v === SEPARATOR_VALUE) continue;
        n++;
        if (carriesSide(v >= SYMBOL_MAX ? lowerSide(v) : v)) slots++;
      }
      const rate = slots / n;
      assert.ok(
        rate > 0.004,
        `${name}${compress ? ' (lz4)' : ''} gave only ${(rate * 100).toFixed(3)} % of symbols a slot`,
      );
    }
  }
  assert.equal((SIDE_MIX * SIDE_UNMIX) & (SYMBOL_MAX - 1), 1);
});

test('the side channel survives a repaired symbol', () => {
  // A symbol error correction put back costs one bit, at a slot the reader
  // knows it cannot trust -- not an offset in every bit after it. Without that,
  // one repair would fail the whole codeword's check.
  const r = rng(65);
  const input = bytes(120000, () => Math.floor(r() * 256));
  const m = { compress: false, protect: true };
  const pairs = encodeFrame(input, m);
  for (let trial = 0; trial < 200; trial++) {
    const bad = pairs.slice();
    for (let k = 0; k < 2; k++) {
      let at = Math.floor(r() * bad.length);
      while (bad[at] === SEPARATOR_VALUE) at = Math.floor(r() * bad.length);
      bad[at] = Math.floor(r() * SYMBOL_MAX);
    }
    const got = decodeFrame(bad, m);
    assert.equal(got.damaged.length, 0, `trial ${trial}: ${JSON.stringify(got.damaged)}`);
    assert.deepEqual(Array.from(got.bytes), Array.from(input), `trial ${trial}`);
  }
});

test('error correction repairs what it promises, and reports the rest', () => {
  const r = rng(66);
  const input = bytes(60000, () => Math.floor(r() * 256));
  const m = { compress: false, protect: true };
  const pairs = encodeFrame(input, m);
  // Two symbol errors per codeword is the capacity of four parity symbols.
  for (const errors of [1, 2]) {
    const bad = pairs.slice();
    for (let k = 0; k < errors; k++) bad[10 + k * 3] ^= 0x155;
    const got = decodeFrame(bad, m);
    assert.equal(got.damaged.length, 0, `${errors} errors should be repaired`);
    assert.equal(got.repaired, errors);
    assert.deepEqual(Array.from(got.bytes), Array.from(input));
  }
  // Three is past it, and the answer must be "this segment is gone", never
  // bytes that look fine.
  const bad = pairs.slice();
  for (let k = 0; k < 3; k++) bad[10 + k * 3] ^= 0x155;
  const got = decodeFrame(bad, m);
  assert.ok(got.damaged.length > 0, 'three errors passed unnoticed');
  assert.equal(got.bytes.length, 0, 'a lost segment still produced bytes');
});

test('damage costs at most two segments, whatever its shape', () => {
  // The bound the whole design exists for: a flipped bit in a 2 GB stream must
  // not cost more than a bounded piece of it. One unrepairable codeword costs
  // its segment; a separator destroyed outright costs the two it divides,
  // because they merge. Nothing costs a third.
  //
  // This is measured with bursts rather than scattered flips, because scattered
  // flips are simply repaired -- which is the point, but not the bound.
  const r = rng(67);
  const A = ALPHABET;
  const payload = bytes(SEGMENT_BYTES * 6, (i) => ((i * 7) & 0xff) ^ ((i >> 11) & 0x3f));
  const text = encode(payload, { protect: true });
  const diff = (a, b) => {
    const n = Math.min(a.length, b.length);
    let wrong = 0;
    for (let i = 0; i < n; i++) if (a[i] !== b[i]) wrong++;
    return wrong + Math.abs(a.length - b.length);
  };
  let worst = 0;
  let silent = 0;
  for (const width of [4, 64, 1024]) {
    for (let trial = 0; trial < 40; trial++) {
      const at = Math.floor(r() * (text.length - width));
      const chars = [...text];
      for (let i = at; i < at + width; i++) chars[i] = A[Math.floor(r() * 91)];
      const got = decodeDetailed(chars.join(''));
      const wrong = diff(payload, got.bytes);
      if (wrong > 0 && got.damaged.length === 0) silent++;
      worst = Math.max(worst, wrong);
    }
  }
  assert.equal(silent, 0, `${silent} runs returned wrong bytes without saying so`);
  assert.ok(
    worst <= 2 * SEGMENT_BYTES,
    `worst damage was ${worst} bytes, ${(worst / SEGMENT_BYTES).toFixed(2)} segments`,
  );
});

test('a scattered flipped bit costs nothing at all', () => {
  const r = rng(68);
  const A = ALPHABET;
  const payload = bytes(400000, (i) => ((i * 13) & 0xff) ^ ((i >> 9) & 0x1f));
  const text = encode(payload, { protect: true });
  for (let trial = 0; trial < 60; trial++) {
    const chars = [...text];
    const at = Math.floor(r() * chars.length);
    let code = chars[at].charCodeAt(0) ^ (1 << Math.floor(r() * 7));
    if (!A.includes(String.fromCharCode(code))) code = A.charCodeAt(Math.floor(r() * 91));
    chars[at] = String.fromCharCode(code);
    assert.deepEqual(
      Array.from(decode(chars.join(''))),
      Array.from(payload),
      `trial ${trial}: one flipped character was not repaired`,
    );
  }
});

test('a damaged stream reports rather than throws when asked to', () => {
  const r = rng(69);
  const A = ALPHABET;
  const payload = bytes(SEGMENT_BYTES * 3, (i) => (i * 5) & 0xff);
  const text = encode(payload, { protect: true });
  const chars = [...text];
  // Enough damage in one place to overwhelm a codeword outright.
  for (let i = 1000; i < 1400; i++) chars[i] = A[Math.floor(r() * 91)];
  const broken = chars.join('');
  assert.throws(() => decode(broken), (err) => err.code === ERR.DAMAGED_SEGMENT);
  const got = decodeDetailed(broken);
  assert.ok(got.damaged.length >= 1);
  assert.ok(got.bytes.length > 0, 'the undamaged segments were thrown away too');
  assert.equal(decode(broken, { partial: true }).length, got.bytes.length);
});

test('a segment whose padding does not add up is refused', () => {
  const input = bytes(500, (i) => i & 0xff);
  const m = { compress: false, protect: false };
  const pairs = encodeFrame(input, m);
  // The pad count is the first byte of the segment, so it lives in the top
  // bits of the first symbol. Thirteen is past the group size and cannot be.
  const bad = pairs.slice();
  bad[0] = (13 << 5) | (bad[0] & 0x1f);
  const got = decodeFrame(bad, m);
  assert.ok(got.damaged.length > 0, 'an impossible pad count was accepted');
});

test('countSideSlots agrees with what the writer actually filled', () => {
  const r = rng(70);
  const symbols = Uint16Array.from({ length: 5000 }, () => Math.floor(r() * SYMBOL_MAX));
  let counted = 0;
  for (const v of symbols) if (carriesSide(v)) counted++;
  assert.equal(countSideSlots(symbols), counted);
  assert.ok(counted > 0);
});
