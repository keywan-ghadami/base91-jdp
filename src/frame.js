// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The framed body: segments, error correction, and the check pattern that
// rides in the side channel.
//
// A framed stream is one marker pair followed by segments separated by "--".
// The separator is the point of the whole arrangement: no packed symbol can
// spell 8280, so a reader that has lost its place finds the next one and
// carries on. Segment boundaries are therefore not a chain -- nothing has to
// survive in segment i for segment i+1 to be found -- and there is no length
// field anywhere in the stream that a single damaged symbol could take out.
//
//   body    = segment ( "--" segment )*
//   segment = codeword+                    each up to RS_DATA data symbols
//                                          plus RS_PARITY parity symbols
//   the segment's data symbols spell:
//           = pad ‖ block ‖ pad × padCount
//
// `pad` is one byte saying how many bytes were added at the end to bring the
// segment up to a whole number of symbols; it is what tells the decompressor
// where the LZ4 block stops, and it is the only overhead the framing has.
//
// The damage bound follows from this shape. A codeword that error correction
// cannot repair costs its segment, because the LZ4 dictionary is reset at
// every segment and nothing after the damage in that segment can be trusted.
// A separator that is itself damaged costs two, since the segments on either
// side of it merge. With SEGMENT_BYTES at 256 KiB that is at most 512 KiB of
// payload for one flipped bit, whatever the size of the stream.

import { compress, decompress, Lz4Error } from './lz4.js';
import { RS13, UncorrectableError } from './rs.js';
import {
  SYMBOL_BITS, SYMBOL_MAX, SEPARATOR_VALUE, SIDE_OFFSET, GROUP_BYTES,
  symbolsFromBytes, bytesFromSymbols, carriesSide, writeSide, readSide, countSideSlots,
} from './pack.js';

/** Payload bytes per segment. Halve it and the damage bound halves with it. */
export const SEGMENT_BYTES = 1 << 18;

/** Reed-Solomon over pair symbols: two symbol errors repaired per codeword. */
export const RS_PARITY = 4;
export const RS_DATA = 4096 - RS_PARITY;

export class FrameError extends Error {
  constructor(message, cause) {
    super(message);
    this.name = 'FrameError';
    if (cause) this.cause = cause;
  }
}

// ---------------------------------------------------------------------
// The check pattern
// ---------------------------------------------------------------------

/**
 * Sixty-four bits derived from a codeword's data symbols and its position.
 *
 * The position counts from the start of the codeword's own segment, never from
 * the start of the stream. A stream-wide counter would put every later segment
 * out of step the moment a separator was lost and two segments merged --
 * exactly the coupling between segments that the separators exist to remove,
 * reintroduced through the back door. Measured, before the index was made
 * local: one burst of 256 characters cost eleven segments of sixteen.
 *
 * Local, it still catches a codeword that arrives in the wrong place within
 * its segment, which is what a lost or invented separator does to the codeword
 * grid, and it stops there.
 */
function checkWords(symbols, from, count, index) {
  let h1 = (0x811c9dc5 ^ index) >>> 0;
  let h2 = (0x9e3779b9 + Math.imul(index, 0x85ebca6b)) >>> 0;
  for (let i = 0; i < count; i++) {
    const s = symbols[from + i];
    h1 = Math.imul(h1 ^ s, 0x01000193) >>> 0;
    h2 = (Math.imul(h2 + s, 0xc2b2ae35) ^ (h2 >>> 15)) >>> 0;
  }
  // A final round so that the last symbol reaches every bit.
  h1 = Math.imul(h1 ^ (h1 >>> 16), 0x85ebca6b) >>> 0;
  h2 = Math.imul(h2 ^ (h2 >>> 13), 0xc2b2ae35) >>> 0;
  return [h1, h2];
}

const checkBit = ([h1, h2], k) => {
  const j = k & 63;
  return (j < 32 ? h1 >>> j : h2 >>> (j - 32)) & 1;
};

// ---------------------------------------------------------------------
// Segments
// ---------------------------------------------------------------------

/** One segment's payload as bytes, padded to a whole number of symbols. */
function segmentBytes(chunk, useLz4) {
  const block = useLz4 ? compress(chunk) : chunk;
  const m = 1 + block.length;
  const padCount = (GROUP_BYTES - (m % GROUP_BYTES)) % GROUP_BYTES;
  const out = new Uint8Array(m + padCount);
  out[0] = padCount;
  out.set(block, 1);
  return out;
}

function segmentPayload(bytes, useLz4, sizeHint) {
  if (bytes.length === 0) throw new FrameError('an empty segment');
  const padCount = bytes[0];
  if (padCount >= GROUP_BYTES) throw new FrameError(`a pad count of ${padCount}`);
  const end = bytes.length - padCount;
  if (end < 1) throw new FrameError('a segment shorter than its own padding');
  const block = bytes.subarray(1, end);
  if (!useLz4) return block;
  try {
    return decompress(block, sizeHint);
  } catch (err) {
    if (err instanceof Lz4Error) throw new FrameError(err.message, err);
    throw err;
  }
}

/** Codeword boundaries within a segment of `n` data symbols. */
function codewordPlan(n, protect) {
  const plan = [];
  if (!protect) {
    plan.push({ data: n, parity: 0 });
    return plan;
  }
  for (let at = 0; at < n; at += RS_DATA) {
    plan.push({ data: Math.min(RS_DATA, n - at), parity: RS_PARITY });
  }
  if (plan.length === 0) plan.push({ data: 0, parity: 0 });
  return plan;
}

// ---------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------

/**
 * Compress and pad each segment, without yet turning any of it into symbols.
 *
 * This is separate from the rest so that a caller can find out what the framed
 * stream would cost before deciding to build it -- and, having decided, build
 * it without compressing twice.
 *
 * @returns {Uint8Array[]} one padded byte run per segment
 */
export function frameSegments(payload, useLz4) {
  const out = [];
  for (let at = 0; at < payload.length; at += SEGMENT_BYTES) {
    out.push(segmentBytes(payload.subarray(at, Math.min(at + SEGMENT_BYTES, payload.length)), useLz4));
  }
  if (out.length === 0) out.push(segmentBytes(payload.subarray(0, 0), useLz4));
  return out;
}

/** Characters the body of these segments will occupy, marker not included. */
export function frameChars(segments, protect) {
  let pairs = segments.length - 1; // the separators
  for (const seg of segments) {
    const symbols = (seg.length * 8) / SYMBOL_BITS;
    pairs += symbols + (protect ? RS_PARITY * Math.ceil(symbols / RS_DATA) : 0);
  }
  return pairs * 2;
}

/**
 * Turn a payload into the symbols of a framed body.
 *
 * @param {Uint8Array|Uint8Array[]} payload bytes, or segments from frameSegments
 * @param {{compress?: boolean, protect?: boolean}} mode
 * @returns {Uint16Array} pair values, 0..8280
 */
export function encodeFrame(payload, { compress: useLz4 = false, protect = true } = {}) {
  const chunks = Array.isArray(payload) ? payload : frameSegments(payload, useLz4);

  const pieces = [];
  let total = 0;

  for (const chunk of chunks) {
    const { symbols } = symbolsFromBytes(chunk);
    let codewordIndex = 0; // within this segment; see checkWords
    const plan = codewordPlan(symbols.length, protect);
    const seg = new Uint16Array(plan.reduce((w, c) => w + c.data + c.parity, 0));

    let src = 0;
    let dst = 0;
    for (const { data, parity } of plan) {
      const message = symbols.subarray(src, src + data);
      const words = checkWords(symbols, src, data, codewordIndex);
      if (parity) {
        seg.set(RS13.encode(message, parity), dst);
      } else {
        seg.set(message, dst);
      }
      // The slots come from the encoded codeword, parity included, and the
      // bits from the message: both are settled before a single one is
      // written, so nothing here is circular.
      writeSide(seg.subarray(dst, dst + data + parity), (slot) => checkBit(words, slot));
      src += data;
      dst += data + parity;
      codewordIndex++;
    }
    pieces.push(seg);
    total += seg.length;
  }

  const out = new Uint16Array(total + pieces.length - 1);
  let at = 0;
  pieces.forEach((piece, i) => {
    if (i) out[at++] = SEPARATOR_VALUE;
    out.set(piece, at);
    at += piece.length;
  });
  return out;
}

// ---------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------

/**
 * Read a framed body back.
 *
 * A segment that cannot be recovered does not stop the others: it is reported
 * in `damaged` and its bytes are left out, because the reason for separators
 * and per-segment dictionaries is that the rest of the stream survives. A
 * caller that wants all or nothing checks `damaged` and refuses.
 *
 * @returns {{bytes: Uint8Array, segments: number, damaged: object[], repaired: number}}
 */
export function decodeFrame(pairs, { compress: useLz4 = false, protect = true } = {}) {
  const bounds = [];
  let start = 0;
  for (let i = 0; i < pairs.length; i++) {
    if (pairs[i] === SEPARATOR_VALUE) {
      bounds.push([start, i]);
      start = i + 1;
    }
  }
  bounds.push([start, pairs.length]);

  const parts = [];
  const damaged = [];
  let repaired = 0;

  bounds.forEach(([from, to], segment) => {
    let codewordIndex = 0; // within this segment; see checkWords
    const wire = pairs.subarray(from, to);
    // Take the side channel's offset off before anything else: 8280 is not a
    // value GF(2^13) has, and the parity was computed without it.
    const symbols = new Uint16Array(wire.length);
    for (let i = 0; i < wire.length; i++) {
      const v = wire[i];
      symbols[i] = v >= SYMBOL_MAX ? Math.min(v - SIDE_OFFSET, SYMBOL_MAX - 1) : v;
    }

    const stride = protect ? RS_DATA + RS_PARITY : symbols.length;
    const words = protect ? Math.max(1, Math.ceil(symbols.length / stride)) : 1;
    // A damaged stream can present a segment too short to hold its own parity;
    // the width must not go negative before the checks below get to say so.
    const data = new Uint16Array(Math.max(0, symbols.length - (protect ? words * RS_PARITY : 0)));
    const trouble = [];

    let src = 0;
    let dst = 0;
    for (let w = 0; w < words; w++) {
      const width = Math.min(stride, symbols.length - src);
      const parity = protect ? Math.min(RS_PARITY, width) : 0;
      const count = width - parity;
      if (count < 0) break;
      const codeword = symbols.subarray(src, src + width);
      if (parity === RS_PARITY && width > RS_PARITY) {
        try {
          repaired += RS13.decode(codeword, RS_PARITY);
        } catch (err) {
          if (!(err instanceof UncorrectableError)) throw err;
          trouble.push({ codeword: codewordIndex, reason: 'uncorrectable' });
        }
      }
      // The slots come from the corrected symbols and the bits from the wire,
      // so a symbol that was repaired shows up as a slot the reader knows it
      // cannot trust rather than as an offset in everything after it.
      const { bits, trusted } = readSide(wire.subarray(src, src + width), codeword);
      const expect = checkWords(codeword, 0, count, codewordIndex);
      let mismatches = 0;
      for (let k = 0; k < bits.length; k++) {
        if (trusted[k] && bits[k] !== checkBit(expect, k)) mismatches++;
      }
      if (mismatches) trouble.push({ codeword: codewordIndex, reason: 'check', mismatches });
      data.set(codeword.subarray(0, count), dst);
      src += width;
      dst += count;
      codewordIndex++;
    }

    if (trouble.length) {
      damaged.push({ segment, trouble });
      return;
    }
    try {
      parts.push(segmentPayload(bytesFromSymbols(data.subarray(0, dst), 0), useLz4, SEGMENT_BYTES));
    } catch (err) {
      if (!(err instanceof FrameError)) throw err;
      damaged.push({ segment, trouble: [{ reason: err.message }] });
    }
  });

  let size = 0;
  for (const p of parts) size += p.length;
  const bytes = new Uint8Array(size);
  let at = 0;
  for (const p of parts) {
    bytes.set(p, at);
    at += p.length;
  }
  return { bytes, segments: bounds.length, damaged, repaired };
}

/** How many free bits a run of symbols would carry. Used by the benchmark. */
export const sideCapacity = (symbols) => countSideSlots(symbols);

export { carriesSide };
