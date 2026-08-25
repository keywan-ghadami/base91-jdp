// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import { encode, decode, decodeDetailed, ALPHABET, ERR, Base91JdpError } from '../src/index.js';
import {
  MODES, ESCAPE, MARKER_MIN, MARKER_MAX, markerChars, readMarker,
} from '../src/marker.js';
import { encodeSynchronous, pairsFromChars, SYMBOL_MAX, SEPARATOR_VALUE } from '../src/pack.js';
import { rng, bytes } from './helpers.js';

test('the marker range is exactly what the packer cannot write', () => {
  assert.equal(MARKER_MIN, SYMBOL_MAX); // 8192
  assert.equal(MARKER_MAX, SEPARATOR_VALUE - 1); // 8279
  assert.equal(MARKER_MAX - MARKER_MIN + 1, 88);
});

test('no packed stream can begin with a marker -- this is the whole rule', () => {
  // Detection is total rather than probable, and that is what the format spends
  // a fixed thirteen bits per pair to buy. A counterexample here would mean an
  // encoder somewhere needs an escape clause, which is exactly what the design
  // is built to avoid.
  const r = rng(51);
  for (let it = 0; it < 40000; it++) {
    const n = 1 + Math.floor(r() * 40);
    const input = bytes(n, () => Math.floor(r() * 256));
    for (const text of [encodeSynchronous(input), encode(input, { compress: 'never' })]) {
      if (text.length < 2) continue;
      const first = pairsFromChars(text.slice(0, 2))[0];
      assert.ok(
        first < MARKER_MIN || first > MARKER_MAX,
        `${JSON.stringify(text.slice(0, 2))} = ${first} is in the marker range`,
      );
    }
  }
});

test('every marker ends in a hyphen, which classic basE91 cannot write', () => {
  for (let v = MARKER_MIN; v <= MARKER_MAX; v++) {
    const chars = markerChars(v);
    assert.equal(chars.length, 2);
    assert.equal(chars[1], '-', `${v} spells ${chars}`);
    assert.equal(pairsFromChars(chars)[0], v);
  }
  // 91 * 90 = 8190 is where the high digit reaches 90, and 90 carries '-'.
  assert.equal(ALPHABET[90], '-');
});

test('the modes are distinct, in range, and tell each other apart', () => {
  const seen = new Set();
  for (const [name, mode] of Object.entries(MODES)) {
    assert.ok(mode.value >= MARKER_MIN && mode.value <= MARKER_MAX, `${name} is out of range`);
    assert.ok(!seen.has(mode.value), `${name} collides on ${mode.value}`);
    seen.add(mode.value);
    assert.equal(readMarker(mode.value).mode.name, name);
  }
  assert.equal(seen.size, 4);
});

test('a pair outside the range means a headerless stream', () => {
  for (const v of [0, 1, 88, 8191, SEPARATOR_VALUE]) {
    assert.equal(readMarker(v).headerless, true, `${v} should be headerless`);
  }
  // "--" at the head is a stream that opens in passthrough, not a marker.
  assert.equal(readMarker(SEPARATOR_VALUE).headerless, true);
});

test('an unclaimed marker is refused with a code, not guessed at', () => {
  const claimed = new Set(Object.values(MODES).map((m) => m.value));
  let checked = 0;
  for (let v = MARKER_MIN; v <= MARKER_MAX; v++) {
    if (claimed.has(v)) continue;
    assert.throws(
      () => readMarker(v),
      (err) => {
        assert.ok(err instanceof Base91JdpError);
        assert.equal(err.code, v === ESCAPE ? ERR.EXTENDED_HEADER : ERR.UNKNOWN_MODE);
        return true;
      },
    );
    checked++;
  }
  assert.equal(checked, 84);
});

test('the escape says a longer header follows, and says so distinctly', () => {
  // It is reserved so that eighty-eight values are not a ceiling. Nothing
  // spends it, and a reader meeting one must say what it cannot do rather than
  // read the stream wrongly.
  assert.throws(
    () => decode(markerChars(ESCAPE) + 'ABCDEFGH'),
    (err) => err.code === ERR.EXTENDED_HEADER,
  );
});

test('every mode round-trips and announces itself', () => {
  const r = rng(52);
  for (const [name, mode] of Object.entries(MODES)) {
    for (const n of [0, 1, 13, 200, 5000, 70000]) {
      const input = bytes(n, (i) => (i % 3 ? Math.floor(r() * 256) : 0));
      const opts = {
        compress: mode.compress ? 'always' : 'never',
        protect: mode.protect ? true : 'check',
      };
      const text = encode(input, opts);
      const seen = decodeDetailed(text);
      assert.ok(seen.framed, `${name} at ${n} bytes did not frame`);
      assert.equal(seen.mode, name, `${name} at ${n} bytes came back as ${seen.mode}`);
      assert.deepEqual(Array.from(decode(text)), Array.from(input), `${name} at ${n} bytes`);
    }
  }
});
