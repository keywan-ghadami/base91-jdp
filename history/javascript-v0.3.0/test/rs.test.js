// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import { GF8, GF13, RS8, RS13, makeField, UncorrectableError } from '../src/rs.js';
import { rng } from './helpers.js';

for (const [name, field] of [['GF(2^8)', GF8], ['GF(2^13)', GF13]]) {
  test(`${name}: alpha generates the whole multiplicative group`, () => {
    const seen = new Set();
    for (let i = 0; i < field.order; i++) seen.add(field.EXP[i]);
    assert.equal(seen.size, field.order);
    assert.ok(!seen.has(0));
  });

  test(`${name}: multiplication and inversion agree`, () => {
    const step = Math.max(1, Math.floor(field.q / 400));
    for (let i = 1; i < field.q; i += step) {
      assert.equal(field.mul(i, field.inv(i)), 1, `${i} has the wrong inverse`);
    }
  });
}

test('a polynomial that is not primitive is refused', () => {
  // x^8 + x^4 + x^3 + x^2 + 1 is primitive; x^8 + 1 is not.
  assert.throws(() => makeField(8, 0x101), /not primitive/);
});

for (const [name, rs] of [['GF(2^8)', RS8], ['GF(2^13)', RS13]]) {
  const lengths = rs.maxCodeword > 300 ? [1, 13, 414, 4000] : [1, 13, 100, 249];

  test(`${name}: parity is appended, the message is left alone`, () => {
    const r = rng(31);
    for (const nsym of [2, 4, 6]) {
      for (const k of lengths) {
        if (k + nsym > rs.maxCodeword) continue;
        const msg = Array.from({ length: k }, () => Math.floor(r() * rs.field.q));
        const cw = rs.encode(msg, nsym);
        assert.equal(cw.length, k + nsym);
        assert.deepEqual(Array.from(cw.subarray(0, k)), msg);
        assert.equal(rs.decode(cw.slice(), nsym), 0, 'a clean codeword reported errors');
      }
    }
  });

  test(`${name}: exactly t errors are repaired`, () => {
    const r = rng(32);
    for (const nsym of [2, 4, 6]) {
      const t = nsym / 2;
      for (const k of lengths) {
        if (k + nsym > rs.maxCodeword) continue;
        for (let it = 0; it < 12; it++) {
          const msg = Array.from({ length: k }, () => Math.floor(r() * rs.field.q));
          const cw = rs.encode(msg, nsym);
          const bad = cw.slice();
          const picked = new Set();
          while (picked.size < Math.min(t, cw.length)) picked.add(Math.floor(r() * cw.length));
          for (const p of picked) {
            let d = 0;
            while (d === 0) d = Math.floor(r() * rs.field.q);
            bad[p] ^= d;
          }
          assert.equal(rs.decode(bad, nsym), picked.size, `${nsym} parity, ${k} data`);
          assert.deepEqual(Array.from(bad), Array.from(cw));
        }
      }
    }
  });

  test(`${name}: a codeword longer than the field is refused`, () => {
    assert.throws(() => rs.encode(new Uint16Array(rs.maxCodeword), 2), RangeError);
  });
}

test('beyond capacity the decoder says so far more often than it guesses', () => {
  // Reed-Solomon can land on a different valid codeword when it is overwhelmed.
  // That is a property of the code; what matters is how often, and that a
  // bigger field makes it rarer.
  const r = rng(33);
  const rates = {};
  for (const [name, rs] of [['GF(2^8)', RS8], ['GF(2^13)', RS13]]) {
    const nsym = 4;
    let detected = 0;
    let miscorrected = 0;
    for (let it = 0; it < 300; it++) {
      const msg = Array.from({ length: 200 }, () => Math.floor(r() * rs.field.q));
      const cw = rs.encode(msg, nsym);
      const bad = cw.slice();
      const picked = new Set();
      while (picked.size < nsym / 2 + 1) picked.add(Math.floor(r() * cw.length));
      for (const p of picked) {
        let d = 0;
        while (d === 0) d = Math.floor(r() * rs.field.q);
        bad[p] ^= d;
      }
      try {
        rs.decode(bad, nsym);
        if (Array.from(bad).every((v, i) => v === cw[i])) detected++;
        else miscorrected++;
      } catch (err) {
        assert.ok(err instanceof UncorrectableError);
        detected++;
      }
    }
    rates[name] = miscorrected / (detected + miscorrected);
  }
  assert.ok(rates['GF(2^8)'] < 0.35, `GF(2^8) miscorrected ${rates['GF(2^8)']}`);
  assert.ok(
    rates['GF(2^13)'] < rates['GF(2^8)'],
    `the larger field should miscorrect less: ${rates['GF(2^13)']} vs ${rates['GF(2^8)']}`,
  );
});
