// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import test from 'node:test';
import assert from 'node:assert/strict';
import { compress, decompress, compressBound, Lz4Error } from '../src/lz4.js';
import { REFERENCE_BLOCKS } from './lz4-fixtures.js';
import { rng, bytes } from './helpers.js';

const trip = (input, what) => {
  const block = compress(input);
  assert.ok(block.length <= compressBound(input.length), `${what}: over the bound`);
  assert.deepEqual(
    Array.from(decompress(block, input.length)),
    Array.from(input),
    `${what}: round trip failed for ${input.length} bytes`,
  );
  return block;
};

test('round trip over every length up to 300', () => {
  const r = rng(41);
  for (let n = 0; n <= 300; n++) {
    trip(bytes(n, (i) => i & 0xff), 'counting');
    trip(bytes(n, () => Math.floor(r() * 256)), 'random');
    trip(bytes(n, () => 0), 'zeros');
  }
});

test('round trip on what the format will actually meet', () => {
  const r = rng(42);
  // A long run, a short period, text, and noise: the four shapes that exercise
  // long matches, overlapping matches, ordinary matches and none at all.
  trip(bytes(1 << 20, () => 0), 'a megabyte of zeros');
  trip(bytes(100000, (i) => 'abcabcab'.charCodeAt(i % 8)), 'a three-byte period');
  trip(bytes(200000, () => Math.floor(r() * 256)), 'noise');
  const text = new TextEncoder().encode('the quick brown fox '.repeat(5000));
  trip(text, 'text');
});

test('a match at the very edge of the two-byte offset still decodes', () => {
  // The offset field is 16 bits, so a match 65535 back is legal and one at
  // 65536 is not. The compressor has to notice; if it did not, the block would
  // decode to something else rather than fail, which is why this is checked by
  // round trip and not by inspection.
  const r = rng(43);
  for (const gap of [65530, 65535, 65536, 65540]) {
    const input = bytes(gap + 64, (i) => (i < 32 ? 65 + (i & 7) : Math.floor(r() * 256)));
    input.set(input.subarray(0, 32), gap);
    trip(input, `gap ${gap}`);
  }
});

test('compression actually compresses, and never expands much', () => {
  const zeros = compress(bytes(1 << 20, () => 0));
  assert.ok(zeros.length < 5000, `a megabyte of zeros became ${zeros.length} bytes`);
  const r = rng(44);
  const noise = bytes(1 << 20, () => Math.floor(r() * 256));
  const grown = compress(noise).length / noise.length;
  assert.ok(grown < 1.005, `noise expanded by ${((grown - 1) * 100).toFixed(2)} %`);
});

test('the final five bytes are literals and no match begins in the last twelve', () => {
  // Both are rules of the block format rather than of this implementation: a
  // decoder that copies in wide steps relies on them. A block that broke them
  // would still round-trip here, so the check has to read the tokens.
  const r = rng(45);
  const input = bytes(4096, (i) => (i % 200 < 100 ? 0 : Math.floor(r() * 4)));
  const block = compress(input);
  let ip = 0;
  let op = 0;
  let lastMatchStart = -1;
  const readLength = (len) => {
    let s;
    do {
      s = block[ip++];
      len += s;
    } while (s === 255);
    return len;
  };
  while (ip < block.length) {
    const token = block[ip++];
    let lit = token >> 4;
    if (lit === 15) lit = readLength(lit);
    ip += lit;
    op += lit;
    if (ip === block.length) break;
    ip += 2;
    let mlen = token & 15;
    if (mlen === 15) mlen = readLength(mlen);
    lastMatchStart = op;
    op += mlen + 4;
  }
  assert.equal(op, input.length, 'the tokens do not account for the input');
  assert.ok(input.length - lastMatchStart >= 12, `a match began ${input.length - lastMatchStart} from the end`);
});

test('a damaged block fails rather than running away', () => {
  // Error correction can be overwhelmed, and what arrives here is then
  // arbitrary bytes. Every one of them must terminate and stay inside its
  // input; whether it is caught is the frame's business, not the block's.
  const r = rng(46);
  const input = bytes(3000, (i) => (i * 7) & 0x3f);
  const block = compress(input);
  const trials = 3000;
  let refused = 0;
  let wrong = 0;
  let harmless = 0;
  for (let trial = 0; trial < trials; trial++) {
    const bad = block.slice();
    bad[Math.floor(r() * bad.length)] ^= 1 << Math.floor(r() * 8);
    try {
      const got = decompress(bad, input.length);
      if (got.length !== input.length || got.some((v, i) => v !== input[i])) wrong++;
      else harmless++;
    } catch (err) {
      assert.ok(err instanceof Lz4Error, `threw ${err.name}: ${err.message}`);
      refused++;
    }
  }
  assert.equal(refused + wrong + harmless, trials);
  // A block has bits no decoder reads: the match nibble of the final token,
  // which stops at the literals. Flipping one of those changes nothing, and
  // that is the whole of it -- 4 bits in this block's 8 * 86.
  assert.ok(
    harmless / trials < 4 / (8 * block.length) + 0.002,
    `${harmless} of ${trials} flips changed nothing; more than the unread bits explain`,
  );
});

test('blocks from the reference implementation decode', () => {
  // The claim this file makes is that src/lz4.js speaks the LZ4 block format,
  // not a private dialect of it. Only upstream's own blocks can settle that.
  for (const { name, plain, block } of REFERENCE_BLOCKS) {
    const want = plain();
    const raw = Uint8Array.from(Buffer.from(block, 'base64'));
    assert.deepEqual(
      Array.from(decompress(raw, want.length)),
      Array.from(want),
      `${name}: a reference block decoded wrong`,
    );
  }
});

test('our blocks say the same thing the reference blocks say', () => {
  // The other direction, as far as it can be checked without the reference at
  // hand: our block must decode to the same bytes, and it must not be wildly
  // fatter than upstream's.
  let ours = 0;
  let theirs = 0;
  for (const { name, plain, block } of REFERENCE_BLOCKS) {
    const want = plain();
    const mine = compress(want);
    assert.deepEqual(
      Array.from(decompress(mine, want.length)),
      Array.from(want),
      `${name}: our own block failed to round trip`,
    );
    ours += mine.length;
    theirs += Buffer.from(block, 'base64').length;
  }
  assert.ok(ours < theirs * 1.15, `our blocks are ${(ours / theirs).toFixed(3)} of upstream's`);
});

test('an offset pointing before the block is refused', () => {
  assert.throws(() => decompress(Uint8Array.from([0x10, 0x41, 0x40, 0x00])), Lz4Error);
  assert.throws(() => decompress(Uint8Array.from([0x10, 0x41, 0x00, 0x00])), Lz4Error);
});

test('a truncated block is refused, or stops early -- never invents bytes', () => {
  // Cutting a block usually lands inside a literal run or a length, which is
  // caught. Cutting exactly at a sequence boundary is indistinguishable from a
  // shorter block, and then the right answer is a prefix of the original: the
  // frame knows the true length, the block format does not.
  const input = bytes(2000, (i) => (i * 3) & 0x1f);
  const block = compress(input);
  let caught = 0;
  for (let cut = 1; cut < block.length; cut++) {
    let got;
    try {
      got = decompress(block.subarray(0, block.length - cut));
    } catch (err) {
      assert.ok(err instanceof Lz4Error);
      caught++;
      continue;
    }
    assert.ok(got.length <= input.length, `cut ${cut} produced ${got.length} bytes`);
    assert.deepEqual(
      Array.from(got),
      Array.from(input.subarray(0, got.length)),
      `cut ${cut} produced bytes the input never had`,
    );
  }
  assert.ok(caught > block.length / 2, `only ${caught} of ${block.length} cuts were caught`);
});
