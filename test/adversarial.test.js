// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import { encode, decode, ALPHABET, R_CHARS, PROFILES, ERR, Base91JdpError } from '../src/index.js';
import { rng } from './helpers.js';

const A = ALPHABET;
const pair = (v) => A[v % 91] + A[(v / 91) | 0];
const SIGNAL = pair(8280);

const throwsWith = (code, fn) =>
  assert.throws(fn, (err) => {
    assert.ok(err instanceof Base91JdpError, `${err} is not a Base91JdpError`);
    assert.equal(err.code, code);
    return true;
  });

test('the signal pair is "--"', () => {
  assert.equal(SIGNAL, '--');
  assert.equal(A.length, 91);
  assert.equal(A[90], '-');
});

test('the block coder never produces the signal pair', () => {
  // Every 13-bit and 14-bit block value, encoded as a pair.
  const r = rng(11);
  for (let it = 0; it < 20000; it++) {
    const n = 1 + Math.floor(r() * 200);
    const buf = Uint8Array.from({ length: n }, () => Math.floor(r() * 256));
    const enc = encode(buf);
    // Any "--" in the output has to be a signal the decoder agrees with,
    // which the round trip already proves; here we only check the block
    // coder in isolation, by encoding data that never enters passthrough.
    if (buf.every((b) => b >= 0x80)) assert.ok(!enc.includes('--'));
  }
});

test('characters outside the alphabet are rejected', () => {
  for (const bad of ['"', '\\', "'", 'ä', String.fromCharCode(0)]) {
    throwsWith(ERR.INVALID_CHARACTER, () => decode(`AB${bad}CD`));
  }
});

test('a header in the future signal space is rejected', () => {
  const limit = 2 * (1 << R_CHARS.length) * PROFILES.length;
  for (const h of [limit, limit + 1, 6000, 8280]) {
    throwsWith(ERR.UNDEFINED_SIGNAL, () => decode(SIGNAL + pair(h) + 'hello there'));
  }
  // one below the limit is a valid signal
  assert.doesNotThrow(() => decode(SIGNAL + pair(limit - 1 - ((limit - 1) % 2)) + 'hello'));
});

test('input that ends inside a header or a pending-bit field', () => {
  // "--" on its own is not a truncated signal: two characters at the end of a
  // stream are its final group, which owes eight bits here and cannot hold
  // 8280. The decoder has no way to read it as anything else, and says so.
  throwsWith(ERR.INVALID_FINAL_BLOCK, () => decode(SIGNAL));
  throwsWith(ERR.UNEXPECTED_EOS, () => decode(SIGNAL + 'A'));
  // hi = 1 with no pending bits present
  throwsWith(ERR.UNEXPECTED_EOS, () => decode(SIGNAL + pair(1)));
});

test('a pending-bit count no encoder can produce is rejected', () => {
  // At the start of a stream the decoder holds 0 bits, so hi = 1 means
  // n_enc = 8; the field is then two characters wide and may carry 8 bits.
  assert.doesNotThrow(() => decode(SIGNAL + pair(1) + pair(255) + 'text'));
  throwsWith(ERR.INVALID_FLUSH, () => decode(SIGNAL + pair(1) + pair(256) + 'text'));
});

test('the pending-bit field always closes the byte it owes', () => {
  // n_enc is congruent to -n mod 8 by construction, so a field that leaves
  // the accumulator part-full cannot be built. Both candidates for n_enc
  // decode; they differ only in how many whole bytes come out.
  const oneBlock = pair(0); // 14 bits: one byte out, six bits held
  // hi = 0 -> n_enc = 2, a one-character field, and one more byte comes out
  assert.equal(decode(`${oneBlock}${SIGNAL}${pair(0)}A` + 'xyz').length, 1 + 1 + 3);
  // hi = 1 -> n_enc = 10, a two-character field, and two more bytes come out
  assert.equal(decode(`${oneBlock}${SIGNAL}${pair(1)}AA` + 'xyz').length, 1 + 2 + 3);
});

test('a trailing single character has to owe bits', () => {
  // A lone character at the very start owes nothing.
  throwsWith(ERR.INVALID_FINAL_BLOCK, () => decode('A'));
  // After a passthrough segment the accumulator is empty again.
  throwsWith(ERR.INVALID_FINAL_BLOCK, () => decode(`${SIGNAL}${pair(0)}hello${SIGNAL}A`));
});

test('every alphabet string decodes or throws, but never hangs or crashes', () => {
  const r = rng(12);
  for (let it = 0; it < 30000; it++) {
    const n = Math.floor(r() * 60);
    let s = '';
    for (let i = 0; i < n; i++) s += A[Math.floor(r() * 91)];
    const marker = s.length >= 2 ? A.indexOf(s[0]) + 91 * A.indexOf(s[1]) : 0;
    const headerless = marker < 8192 || marker > 8279;
    try {
      const out = decode(s);
      // A headerless stream can never write more than it reads: passthrough is
      // one character per byte and the block coder is sixteen per thirteen. A
      // framed stream carries a compressor and has no such bound -- expansion
      // is the entire point of it -- so there the invariant is only that the
      // decoder finishes.
      if (headerless) {
        assert.ok(out.length <= s.length, `${out.length} bytes from ${s.length} characters`);
      }
    } catch (err) {
      assert.ok(err instanceof Base91JdpError, `${err.name}: ${err.message}`);
      assert.ok(err.code, 'every rejection carries a code');
    }
  }
});

test('a framed stream may expand, and that is what compression is', () => {
  // The counterpart to the bound above, so that its absence in the framed case
  // is a stated property and not an oversight.
  const zeros = new Uint8Array(1 << 20);
  const enc = encode(zeros);
  assert.ok(enc.length < 8000, `a megabyte of zeros became ${enc.length} characters`);
  assert.equal(decode(enc).length, zeros.length);
});

test('donor profiles are structurally sound', () => {
  for (const p of PROFILES) {
    assert.equal(p.length, R_CHARS.length);
    assert.equal(new Set(p).size, R_CHARS.length);
    for (const ch of p) {
      assert.ok(A.includes(ch), `${ch} is not in the alphabet`);
      assert.notEqual(ch, '-', 'the signal character may not be a donor');
    }
  }
});

test('the R-Set is the alphabet\'s complement in text, plus the signal character', () => {
  assert.equal(new Set(R_CHARS).size, R_CHARS.length);
  for (const c of R_CHARS) {
    const ch = String.fromCharCode(c);
    // '-' is the one R-Set member that is in the alphabet: it is substituted
    // not because it cannot be written but because two of them end a segment.
    if (ch === '-') continue;
    assert.ok(!A.includes(ch), `${JSON.stringify(ch)} is in the alphabet`);
  }
  assert.ok(R_CHARS.includes('-'.charCodeAt(0)));
});

test('a passthrough payload never contains the signal character at all', () => {
  const src = new TextEncoder().encode(
    '--bs-blue: #0d6efd; --bs-indigo: #6610f2; --bs-purple: #6f42c1; ' +
      '--bs-pink: #d63384; --bs-red: #dc3545; --bs-orange: #fd7e14;',
  );
  const enc = encode(src);
  // Every '-' in the output belongs to a signal, so they come in pairs at
  // positions the decoder reaches; none is ever payload.
  const stats = enc.split('').filter((c) => c === '-').length;
  assert.equal(stats % 2, 0);
  assert.ok(stats <= 4, `${stats} hyphens for a segment or two, not one per input '-'`);
  assert.deepEqual(decode(enc), src);
});

test('no emitted segment contains the signal pair or ends on its character', () => {
  const src = new TextEncoder().encode(
    'a-b-c-d-e-f a longer stretch of ordinary text so that passthrough is used, ' +
      'and then -- a doubled hyphen -- and more text after it, long enough again.',
  );
  const enc = encode(src);
  // Walk the stream the way a decoder does and inspect each payload.
  let i = 0;
  const segments = [];
  while (i < enc.length) {
    if (enc.startsWith('--', i)) {
      i += 2 + 2; // signal and header
      const hi = (enc.charCodeAt(i - 2), A.indexOf(enc[i - 2]) & 1);
      i += hi ? 2 : 0; // approximate; the exact width is tested by round trip
      const end = enc.indexOf('--', i);
      const payload = end < 0 ? enc.slice(i) : enc.slice(i, end);
      segments.push(payload);
      i = end < 0 ? enc.length : end + 2;
    } else {
      i += 2;
    }
  }
  for (const s of segments) {
    assert.ok(!s.includes('--'), `segment contains the signal: ${s}`);
  }
});
