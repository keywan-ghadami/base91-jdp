// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  encodeSynchronous, decodeSynchronous, encodeAdaptive, decodeAdaptive,
  symbolsFromBytes, bytesFromSymbols, charsFromSymbols, symbolsFromChars,
  charValue, TAIL_CHARS, GROUP_BYTES, GROUP_CHARS, PackError,
} from '../src/pack.js';
import { ALPHABET } from '../src/index.js';
import { rng, bytes } from './helpers.js';

const ALPHABET_SET = new Set(ALPHABET);

function packRoundTrip(input) {
  for (const [name, encode, decode] of [
    ['synchronous', encodeSynchronous, decodeSynchronous],
    ['adaptive', encodeAdaptive, decodeAdaptive],
  ]) {
    const text = encode(input);
    for (const ch of text) {
      assert.ok(ALPHABET_SET.has(ch), `${name}: ${JSON.stringify(ch)} is not in the alphabet`);
    }
    assert.equal(JSON.stringify(text).slice(1, -1), text, `${name}: would need JSON escaping`);
    assert.deepEqual(
      Array.from(decode(text)),
      Array.from(input),
      `${name}: round trip failed for ${input.length} bytes`,
    );
  }
}

test('round trip over every length up to 200', () => {
  const r = rng(7);
  for (let n = 0; n <= 200; n++) {
    packRoundTrip(bytes(n, (i) => i & 0xff));
    packRoundTrip(bytes(n, () => Math.floor(r() * 256)));
  }
});

test('round trip across group and buffer boundaries', () => {
  const r = rng(8);
  for (const n of [4095, 4096, 4097, GROUP_BYTES * 1000, GROUP_BYTES * 1000 + 7, 65537]) {
    packRoundTrip(bytes(n, () => Math.floor(r() * 256)));
  }
});

test('a whole group is exactly 13 bytes in 16 characters', () => {
  const r = rng(9);
  for (let groups = 1; groups <= 20; groups++) {
    const input = bytes(groups * GROUP_BYTES, () => Math.floor(r() * 256));
    assert.equal(encodeSynchronous(input).length, groups * GROUP_CHARS);
  }
});

test('the trailing group is self-delimiting', () => {
  // Every partial group has its own character count, and no two share one, so
  // a decoder recovers the byte length from the character count alone.
  const r = rng(10);
  const seen = new Map();
  for (let rem = 0; rem < GROUP_BYTES; rem++) {
    const input = bytes(GROUP_BYTES * 3 + rem, () => Math.floor(r() * 256));
    const chars = encodeSynchronous(input).length % GROUP_CHARS;
    assert.equal(chars, TAIL_CHARS[rem], `remainder ${rem}`);
    assert.ok(!seen.has(chars), `remainder ${rem} collides with ${seen.get(chars)}`);
    seen.set(chars, rem);
  }
});

test('character counts that no encoder can produce are rejected', () => {
  for (const rem of [1, 6, 11]) {
    assert.ok(!TAIL_CHARS.includes(rem));
    assert.throws(() => decodeSynchronous('A'.repeat(GROUP_CHARS + rem)), PackError);
  }
});

test('the symbol layer round-trips, and every symbol fits 13 bits', () => {
  const r = rng(11);
  for (const n of [1, 12, 13, 14, 1000, 1001]) {
    const input = bytes(n, () => Math.floor(r() * 256));
    const { symbols, tailBits } = symbolsFromBytes(input);
    for (const s of symbols) assert.ok(s < 8192, `${s} does not fit 13 bits`);
    assert.deepEqual(Array.from(bytesFromSymbols(symbols, tailBits)), Array.from(input));
    assert.deepEqual(
      Array.from(symbolsFromChars(charsFromSymbols(symbols))),
      Array.from(symbols),
    );
  }
});

test('one flipped character damages at most three bytes', () => {
  // The property the whole containment argument rests on: a pair carries 13
  // bits, which touch two or three bytes and nothing beyond them.
  const r = rng(12);
  const input = bytes(9000, () => Math.floor(r() * 256));
  const text = encodeSynchronous(input);
  let worst = 0;
  for (let trial = 0; trial < 400; trial++) {
    const pos = Math.floor(r() * text.length);
    let code = text.charCodeAt(pos) ^ (1 << Math.floor(r() * 7));
    if (charValue(code) < 0) code = ALPHABET.charCodeAt(0); // a reader substitutes
    const bad = text.slice(0, pos) + String.fromCharCode(code) + text.slice(pos + 1);
    const got = decodeSynchronous(bad);
    assert.equal(got.length, input.length, 'the byte length must not move');
    let wrong = 0;
    for (let i = 0; i < input.length; i++) if (input[i] !== got[i]) wrong++;
    worst = Math.max(worst, wrong);
  }
  assert.ok(worst <= 3, `worst case was ${worst} bytes`);
});

test('characters outside the alphabet are reported, not guessed at', () => {
  assert.equal(charValue('"'.charCodeAt(0)), -1);
  assert.equal(charValue(0), -1);
  assert.ok(charValue('A'.charCodeAt(0)) >= 0);
  assert.throws(() => decodeSynchronous('AB"D'), PackError);
});
