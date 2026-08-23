// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Three ways to turn bytes into characters of the JSON-safe alphabet, so that
// bench/rsstudy.js can measure what each costs and what each loses when a bit
// flips.
//
//   adaptive   basE91 unchanged: two characters carry 13 or 14 bits, and which
//              one is decided by the value of the pair. Densest by 0.085 %,
//              and the only one where a flipped character can shift every bit
//              that follows it.
//   synchronous  13 bytes = 104 bits = 8 pairs of 13 bits = 16 characters,
//              exactly. Each pair is independent, so a flipped character
//              damages the two or three bytes its 13 bits touch and nothing
//              else. This is also the only variant with a symbol layer a
//              Reed-Solomon code can sit on: one pair is one GF(2^13) symbol.
//
// The third variant of the study, adaptive-with-realignment, is the adaptive
// coder applied to each segment separately; bench/rsstudy.js builds it from
// `encodeAdaptive` rather than needing anything here.

import { ALPHABET } from './codec.js';

const VAL = new Int16Array(256).fill(-1);
for (let v = 0; v < ALPHABET.length; v++) VAL[ALPHABET.charCodeAt(v)] = v;
const CHR = new Uint8Array(91);
for (let v = 0; v < ALPHABET.length; v++) CHR[v] = ALPHABET.charCodeAt(v);

export const SYMBOL_BITS = 13;
export const SYMBOL_MAX = 1 << SYMBOL_BITS; // 8192 values, 0..8191
export const GROUP_BYTES = 13;
export const GROUP_CHARS = 16;

export class PackError extends Error {
  constructor(message) {
    super(message);
    this.name = 'PackError';
  }
}

// Characters a trailing group of r bytes occupies, r = 0..12. Two per whole
// 13-bit symbol, plus one for a remainder of six bits or fewer and two
// otherwise. The sequence is strictly increasing, which is what lets a decoder
// recover r from the character count without a length field.
export const TAIL_CHARS = [0, 2, 3, 4, 5, 7, 8, 9, 10, 12, 13, 14, 15];
const TAIL_BYTES = new Map(TAIL_CHARS.map((c, r) => [c, r]));

// ---------------------------------------------------------------------
// Symbol layer: bytes <-> 13-bit symbols, most significant bit first
// ---------------------------------------------------------------------

/**
 * Split bytes into 13-bit symbols. A remainder of 1..12 bits becomes a final
 * short symbol, whose width is returned as `tailBits`.
 *
 * @returns {{symbols: Uint16Array, tailBits: number}}
 */
export function symbolsFromBytes(bytes) {
  const bits = bytes.length * 8;
  const full = Math.floor(bits / SYMBOL_BITS);
  const tailBits = bits - full * SYMBOL_BITS;
  const symbols = new Uint16Array(full + (tailBits ? 1 : 0));
  let acc = 0;
  let nb = 0;
  let si = 0;
  for (let i = 0; i < bytes.length; i++) {
    acc = (acc << 8) | bytes[i];
    nb += 8;
    while (nb >= SYMBOL_BITS) {
      symbols[si++] = (acc >>> (nb - SYMBOL_BITS)) & (SYMBOL_MAX - 1);
      nb -= SYMBOL_BITS;
    }
  }
  if (tailBits) symbols[si] = acc & ((1 << tailBits) - 1);
  return { symbols, tailBits };
}

/** The inverse: `tailBits` says how wide the last symbol is, 0 if none. */
export function bytesFromSymbols(symbols, tailBits) {
  const full = tailBits ? symbols.length - 1 : symbols.length;
  const bits = full * SYMBOL_BITS + tailBits;
  const out = new Uint8Array(Math.floor(bits / 8));
  let acc = 0;
  let nb = 0;
  let bi = 0;
  const emit = (width, value) => {
    acc = (acc << width) | value;
    nb += width;
    while (nb >= 8) {
      out[bi++] = (acc >>> (nb - 8)) & 0xff;
      nb -= 8;
    }
  };
  for (let i = 0; i < full; i++) emit(SYMBOL_BITS, symbols[i]);
  if (tailBits) emit(tailBits, symbols[full]);
  return out;
}

// ---------------------------------------------------------------------
// Character layer
// ---------------------------------------------------------------------

/** Every symbol as a full pair. Used when a code has appended parity. */
export function charsFromSymbols(symbols) {
  const out = new Uint8Array(symbols.length * 2);
  for (let i = 0; i < symbols.length; i++) {
    const v = symbols[i];
    out[2 * i] = CHR[(v / 91) | 0];
    out[2 * i + 1] = CHR[v % 91];
  }
  return latin1(out);
}

/** The inverse. A pair outside 0..8191 is clamped, not rejected: handing the
 *  damaged symbol to the error-correcting layer is the whole point. */
export function symbolsFromChars(text) {
  if (text.length % 2 !== 0) throw new PackError('an odd number of characters');
  const symbols = new Uint16Array(text.length / 2);
  for (let i = 0; i < symbols.length; i++) {
    const d0 = digit(text.charCodeAt(2 * i));
    const d1 = digit(text.charCodeAt(2 * i + 1));
    symbols[i] = (d0 * 91 + d1) & (SYMBOL_MAX - 1);
  }
  return symbols;
}

// ---------------------------------------------------------------------
// Byte-synchronous coder, self-delimiting
// ---------------------------------------------------------------------

/** 13 bytes to 16 characters, with the short-tail rule of TAIL_CHARS. */
export function encodeSynchronous(bytes) {
  const { symbols, tailBits } = symbolsFromBytes(bytes);
  const full = tailBits ? symbols.length - 1 : symbols.length;
  const tail = tailBits ? (tailBits <= 6 ? 1 : 2) : 0;
  const out = new Uint8Array(full * 2 + tail);
  let o = 0;
  for (let i = 0; i < full; i++) {
    const v = symbols[i];
    out[o++] = CHR[(v / 91) | 0];
    out[o++] = CHR[v % 91];
  }
  if (tail === 1) {
    out[o++] = CHR[symbols[full]];
  } else if (tail === 2) {
    const v = symbols[full];
    out[o++] = CHR[(v / 91) | 0];
    out[o++] = CHR[v % 91];
  }
  return latin1(out);
}

export function decodeSynchronous(text) {
  const rem = text.length % GROUP_CHARS;
  if (!TAIL_BYTES.has(rem)) {
    throw new PackError(`${text.length} characters cannot end a stream`);
  }
  const tailBytes = TAIL_BYTES.get(rem);
  const byteLength = Math.floor(text.length / GROUP_CHARS) * GROUP_BYTES + tailBytes;
  const bits = byteLength * 8;
  const full = Math.floor(bits / SYMBOL_BITS);
  const tailBits = bits - full * SYMBOL_BITS;

  const symbols = new Uint16Array(full + (tailBits ? 1 : 0));
  for (let i = 0; i < full; i++) {
    const d0 = digit(text.charCodeAt(2 * i));
    const d1 = digit(text.charCodeAt(2 * i + 1));
    symbols[i] = (d0 * 91 + d1) & (SYMBOL_MAX - 1);
  }
  if (tailBits) {
    let v;
    if (tailBits <= 6) {
      v = digit(text.charCodeAt(full * 2));
    } else {
      const d0 = digit(text.charCodeAt(full * 2));
      const d1 = digit(text.charCodeAt(full * 2 + 1));
      v = d0 * 91 + d1;
    }
    symbols[full] = v & ((1 << tailBits) - 1);
  }
  return bytesFromSymbols(symbols, tailBits);
}

// ---------------------------------------------------------------------
// Adaptive coder: basE91 on the same alphabet
// ---------------------------------------------------------------------

const ADAPTIVE_THRESHOLD = 88;

export function encodeAdaptive(bytes) {
  const out = new Uint8Array(Math.ceil(bytes.length * 1.3) + 8);
  let o = 0;
  let b = 0;
  let n = 0;
  for (let i = 0; i < bytes.length; i++) {
    b |= bytes[i] << n;
    n += 8;
    if (n > 13) {
      let v = b & 8191;
      if (v > ADAPTIVE_THRESHOLD) {
        b >>= 13;
        n -= 13;
      } else {
        v = b & 16383;
        b >>= 14;
        n -= 14;
      }
      out[o++] = CHR[v % 91];
      out[o++] = CHR[(v / 91) | 0];
    }
  }
  if (n) {
    out[o++] = CHR[b % 91];
    if (n > 7 || b > 90) out[o++] = CHR[(b / 91) | 0];
  }
  return latin1(out.subarray(0, o));
}

export function decodeAdaptive(text) {
  const out = new Uint8Array(text.length + 8);
  let o = 0;
  let b = 0;
  let n = 0;
  let v = -1;
  for (let i = 0; i < text.length; i++) {
    const d = digit(text.charCodeAt(i));
    if (v < 0) {
      v = d;
      continue;
    }
    v += d * 91;
    b |= v << n;
    n += (v & 8191) > ADAPTIVE_THRESHOLD ? 13 : 14;
    do {
      out[o++] = b & 0xff;
      b >>= 8;
      n -= 8;
    } while (n > 7);
    v = -1;
  }
  if (v >= 0) out[o++] = (b | (v << n)) & 0xff;
  return out.subarray(0, o);
}

// ---------------------------------------------------------------------

/** The alphabet value of a character code, or -1. Never throws: a damaged
 *  stream is exactly the case an error-correcting layer wants to see. */
export const charValue = (code) => (code < 256 ? VAL[code] : -1);

function digit(code) {
  const v = charValue(code);
  if (v < 0) {
    throw new PackError(`${JSON.stringify(String.fromCharCode(code))} is not in the alphabet`);
  }
  return v;
}

function latin1(bytes) {
  let s = '';
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    s += String.fromCharCode.apply(null, bytes.subarray(i, i + CHUNK));
  }
  return s;
}
