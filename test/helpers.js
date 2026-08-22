// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import assert from 'node:assert/strict';
import { encode, decode, ALPHABET } from '../src/index.js';

/** A small deterministic PRNG, so a failing case can be reproduced. */
export function rng(seed) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export const bytes = (n, f) => Uint8Array.from({ length: n }, (_, i) => f(i));

const ALPHABET_SET = new Set(ALPHABET);

/** Encode, decode, and check everything that has to hold of the result. */
export function roundTrip(input) {
  const enc = encode(input);
  for (const ch of enc) {
    assert.ok(ALPHABET_SET.has(ch), `${JSON.stringify(ch)} is not in the alphabet`);
  }
  // JSON.stringify escapes exactly what a JSON string cannot hold verbatim;
  // if it changes nothing, the output needs no escaping.
  assert.equal(JSON.stringify(enc).slice(1, -1), enc, 'output would need escaping in JSON');
  const back = decode(enc);
  assert.deepEqual(
    Array.from(back),
    Array.from(input),
    `round trip failed for ${input.length} bytes`,
  );
  return enc;
}
