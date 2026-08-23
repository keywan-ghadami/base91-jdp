// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The LZ4 block format, compressor and decompressor, with no dependency.
//
// LZ4 is here because a specification may demand it without demanding a
// library: the block format is a token byte, a literal run, a two-byte offset
// and a match length, and that is the whole of it. Anyone implementing this
// format can write the two hundred lines below rather than take a dependency,
// which is not true of deflate.
//
// A block is a sequence of sequences. Each begins with a token whose high
// nibble is the literal length and whose low nibble is the match length less
// four; a nibble of 15 means "and more", carried in further bytes of 255 until
// one below 255 ends the run. Literals follow the token, then a little-endian
// offset back into what has already been decoded, then the match. The last
// sequence carries literals and stops -- which is why a decoder needs to know
// where the block ends, and why the frame around it (src/frame.js) says so.
//
// Two rules of the format constrain the compressor: the final five bytes of a
// block are always literals, and no match may begin in the last twelve. They
// exist so that a decoder may copy in wide steps without overrunning, and they
// are why a block shorter than thirteen bytes is all literals.

const MINMATCH = 4;
const MFLIMIT = 12; // no match may begin this close to the end
const LASTLITERALS = 5;
const MAX_DISTANCE = 65535; // the offset field is two bytes
const HASH_BITS = 16;
const HASH_SIZE = 1 << HASH_BITS;
const SKIP_TRIGGER = 6; // how fast the scan gives up on incompressible input

export class Lz4Error extends Error {
  constructor(message) {
    super(message);
    this.name = 'Lz4Error';
  }
}

/** The largest a block of `n` bytes can become: every byte a literal, plus the
 *  length bytes that a single enormous literal run needs. */
export const compressBound = (n) => n + Math.ceil(n / 255) + 16;

// ---------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------

/**
 * Compress one block. The dictionary starts empty and does not survive the
 * call, which is what makes a damaged segment cost its own bytes and no more.
 *
 * @param {Uint8Array} src
 * @returns {Uint8Array}
 */
export function compress(src) {
  const n = src.length;
  const out = new Uint8Array(compressBound(n));
  let o = 0;

  const writeLength = (len) => {
    while (len >= 255) {
      out[o++] = 255;
      len -= 255;
    }
    out[o++] = len;
  };
  const emitLiterals = (from, to) => {
    for (let i = from; i < to; i++) out[o++] = src[i];
  };

  // Too short for a match to be legal: the whole thing is one literal run.
  if (n < MFLIMIT + 1) {
    out[o++] = (n < 15 ? n : 15) << 4;
    if (n >= 15) writeLength(n - 15);
    emitLiterals(0, n);
    return out.subarray(0, o);
  }

  const hashTable = new Int32Array(HASH_SIZE).fill(-1);
  const read32 = (i) => src[i] | (src[i + 1] << 8) | (src[i + 2] << 16) | (src[i + 3] << 24);
  const hash = (i) => (Math.imul(read32(i), 2654435761) >>> (32 - HASH_BITS)) >>> 0;

  const mflimit = n - MFLIMIT;
  const matchlimit = n - LASTLITERALS;
  let anchor = 0;
  let ip = 0;

  while (ip < mflimit) {
    // Find a match. On input that does not compress, the step grows so that
    // the scan walks out rather than hashing every byte for nothing.
    let ref = -1;
    let searchAttempts = 1 << SKIP_TRIGGER;
    for (;;) {
      const h = hash(ip);
      const candidate = hashTable[h];
      hashTable[h] = ip;
      if (
        candidate >= 0 &&
        ip - candidate <= MAX_DISTANCE &&
        read32(candidate) === read32(ip)
      ) {
        ref = candidate;
        break;
      }
      ip += searchAttempts++ >> SKIP_TRIGGER;
      if (ip >= mflimit) break;
    }
    if (ref < 0) break;

    // How far the match runs, stopping short of the five literals the format
    // reserves at the end.
    let mlen = MINMATCH;
    while (ip + mlen < matchlimit && src[ip + mlen] === src[ref + mlen]) mlen++;

    const litLen = ip - anchor;
    const mlCode = mlen - MINMATCH;
    out[o++] = ((litLen < 15 ? litLen : 15) << 4) | (mlCode < 15 ? mlCode : 15);
    if (litLen >= 15) writeLength(litLen - 15);
    emitLiterals(anchor, ip);
    const offset = ip - ref;
    out[o++] = offset & 0xff;
    out[o++] = offset >>> 8;
    if (mlCode >= 15) writeLength(mlCode - 15);

    ip += mlen;
    anchor = ip;
    // Two positions inside the match are worth remembering: they are where the
    // next match is most likely to be found.
    if (ip < mflimit) {
      hashTable[hash(ip - 2)] = ip - 2;
    }
  }

  const litLen = n - anchor;
  out[o++] = (litLen < 15 ? litLen : 15) << 4;
  if (litLen >= 15) writeLength(litLen - 15);
  emitLiterals(anchor, n);
  return out.subarray(0, o);
}

// ---------------------------------------------------------------------
// Decompressor
// ---------------------------------------------------------------------

/**
 * Decompress one block.
 *
 * Every read is bounds-checked, because this runs on data that may have
 * arrived damaged: a stream whose error correction was overwhelmed still
 * reaches here, and it must fail rather than run away or read past its input.
 *
 * @param {Uint8Array} src the block, exactly -- no trailing padding
 * @param {number} [sizeHint] expected output size, an allocation hint only
 * @returns {Uint8Array}
 */
export function decompress(src, sizeHint = 0) {
  let out = new Uint8Array(Math.max(sizeHint, src.length * 3, 64));
  let op = 0;
  const room = (extra) => {
    if (op + extra <= out.length) return;
    const bigger = new Uint8Array(Math.max(out.length * 2, op + extra));
    bigger.set(out.subarray(0, op));
    out = bigger;
  };

  let ip = 0;
  const readLength = (len) => {
    for (;;) {
      if (ip >= src.length) throw new Lz4Error('a length runs past the end of the block');
      const s = src[ip++];
      len += s;
      if (s !== 255) return len;
    }
  };

  while (ip < src.length) {
    const token = src[ip++];

    let litLen = token >> 4;
    if (litLen === 15) litLen = readLength(litLen);
    if (ip + litLen > src.length) throw new Lz4Error('literals run past the end of the block');
    room(litLen);
    for (let i = 0; i < litLen; i++) out[op++] = src[ip++];

    // The last sequence is literals and nothing else.
    if (ip === src.length) break;
    if (ip + 2 > src.length) throw new Lz4Error('an offset runs past the end of the block');

    const offset = src[ip] | (src[ip + 1] << 8);
    ip += 2;
    if (offset === 0 || offset > op) throw new Lz4Error(`offset ${offset} points outside the block`);

    let mlen = token & 15;
    if (mlen === 15) mlen = readLength(mlen);
    mlen += MINMATCH;

    room(mlen);
    let ref = op - offset;
    // Byte at a time on purpose: an overlapping match is how LZ4 spells a run,
    // and it only works if each byte is copied after the one it repeats.
    for (let i = 0; i < mlen; i++) out[op++] = out[ref++];
  }

  return out.subarray(0, op);
}
