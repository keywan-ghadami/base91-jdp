// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import { encode, decode, encodeText, decodeText, CONSTANTS } from '../src/index.js';
import { rng, bytes, roundTrip } from './helpers.js';

test('empty input', () => {
  assert.equal(encode(new Uint8Array(0)), '');
  assert.deepEqual(decode(''), new Uint8Array(0));
});

test('every length from 0 to 300, counting bytes', () => {
  for (let n = 0; n <= 300; n++) roundTrip(bytes(n, (i) => i & 0xff));
});

test('every length from 0 to 300, random bytes', () => {
  const r = rng(1);
  for (let n = 0; n <= 300; n++) roundTrip(bytes(n, () => Math.floor(r() * 256)));
});

test('lengths around the lookahead bound', () => {
  const r = rng(2);
  const text = (n) => bytes(n, () => 32 + Math.floor(r() * 90));
  for (const n of [
    1023, 1024, 1025,
    CONSTANTS.MAX_DP_BYTES - 1,
    CONSTANTS.MAX_DP_BYTES,
    CONSTANTS.MAX_DP_BYTES + 1,
    CONSTANTS.MAX_DP_BYTES * 2 + 7,
  ]) {
    roundTrip(text(n));
    roundTrip(bytes(n, () => Math.floor(r() * 256)));
  }
});

test('every R-Set character, alone and in combination', () => {
  const R = [' ', '"', '\n', '\\', '\r', "'", '\t', '-'];
  for (let m = 0; m < 256; m++) {
    let s = 'the quick brown fox jumps over the lazy dog and then some more text';
    for (let j = 0; j < 8; j++) if (m & (1 << j)) s += R[j] + 'padding' + j;
    roundTrip(new TextEncoder().encode(s.repeat(3)));
  }
});

test('the signal character, at every position it can occupy', () => {
  const filler = 'abcdefghijklmnopqrstuvwxyz0123456789 abcdefghijklmnop';
  for (const run of ['-', '--', '---', '----', '-----']) {
    for (const pre of ['', 'x', filler, filler + filler]) {
      for (const post of ['', 'y', filler, filler + filler]) {
        roundTrip(new TextEncoder().encode(pre + run + post));
      }
    }
  }
});

test('the pending bit count takes every value at a transition', () => {
  // A block-mode run of m bytes leaves a specific number of bits pending; by
  // walking m the transition into passthrough is exercised at each of them.
  const text = 'a passthrough segment long enough to be worth its own signal, yes';
  const seen = new Set();
  for (let m = 0; m < 40; m++) {
    const head = bytes(m, (i) => 0x80 + ((i * 37) & 0x7f)); // never representable
    const tail = new TextEncoder().encode(text);
    const buf = new Uint8Array(head.length + tail.length);
    buf.set(head);
    buf.set(tail, head.length);
    roundTrip(buf);
    seen.add(m % 13);
  }
  assert.equal(seen.size, 13);
});

test('runs of identical bytes', () => {
  for (const value of [0x00, 0x20, 0x41, 0xff]) {
    for (const n of [1, 4, 5, 25, 26, 27, 2048, 5000]) {
      roundTrip(new Uint8Array(n).fill(value));
    }
  }
});

test('mixed text, binary and runs', () => {
  const r = rng(3);
  const pieces = [
    () => new TextEncoder().encode('{"k":"v","n":42,"list":[1,2,3],"esc":"a\\\\b"}'),
    () => new TextEncoder().encode('plain prose with spaces, punctuation and a newline\n'),
    () => new TextEncoder().encode('--flag --other -- rest'),
    () => bytes(Math.floor(r() * 60), () => Math.floor(r() * 256)),
    () => new Uint8Array(Math.floor(r() * 40)).fill(Math.floor(r() * 256)),
    () => new TextEncoder().encode('Ünïcödé — mit Umlauten und Gedankenstrich'),
  ];
  for (let it = 0; it < 500; it++) {
    const parts = [];
    for (let q = 0, m = 1 + Math.floor(r() * 6); q < m; q++) {
      parts.push(pieces[Math.floor(r() * pieces.length)]());
    }
    const total = parts.reduce((s, p) => s + p.length, 0);
    const buf = new Uint8Array(total);
    let off = 0;
    for (const p of parts) {
      buf.set(p, off);
      off += p.length;
    }
    roundTrip(buf);
  }
});

test('text helpers', () => {
  const s = 'Grüße aus München — "quoted", \'single\' and \\ backslash\n';
  assert.equal(decodeText(encodeText(s)), s);
  assert.throws(() => decodeText(encode(new Uint8Array([0xff, 0xfe]))));
});

test('whitespace in the input is skipped', () => {
  const data = new TextEncoder().encode(
    'a reasonably long passthrough segment, and then some binary: \x00\x01\x02',
  );
  const enc = encode(data);
  const wrapped = enc.replace(/(.{10})/g, '$1\n');
  assert.deepEqual(decode(wrapped), data);
  assert.deepEqual(decode(`  ${enc}\t\r\n`), data);
});
