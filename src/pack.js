// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The packing layer: bytes to characters of the JSON-safe alphabet.
//
// Two characters are one pair, worth `d0 + 91 * d1` -- the low digit first, as
// basE91 writes it. A pair has 8281 values and the packer uses 8192 of them:
// 13 bytes = 104 bits = 8 symbols of 13 bits = 16 characters, exactly. Each
// pair stands on its own, so a flipped character damages the two or three
// bytes its 13 bits touch and nothing beyond them, and one pair is one
// GF(2^13) symbol, which is what lets Reed-Solomon sit on top for the price of
// nsym/n.
//
// That leaves 8192..8280, eighty-nine values no packed stream can contain.
// They carry everything the format needs to say about itself:
//
//   8280         the separator "--": it opens and closes a passthrough segment
//                and it divides one framed segment from the next
//   8192..8279   at the head of a stream, the mode marker (src/marker.js);
//                anywhere else, a symbol from the side-channel window carrying
//                one extra bit at no cost in characters (src/frame.js)
//
// `encodeAdaptive` is basE91 as Joachim Henke wrote it, on this alphabet. It
// is not part of the format -- it is denser by 0.085 % and gives that back the
// moment a character is damaged, which is what bench/rsstudy.js measured. It
// stays as the reference the study is written against.

import { ALPHABET, Base91JdpError, ERR } from './codec.js';

const VAL = new Int16Array(256).fill(-1);
for (let v = 0; v < ALPHABET.length; v++) VAL[ALPHABET.charCodeAt(v)] = v;
const CHR = new Uint8Array(91);
for (let v = 0; v < ALPHABET.length; v++) CHR[v] = ALPHABET.charCodeAt(v);

export const SYMBOL_BITS = 13;
export const SYMBOL_MAX = 1 << SYMBOL_BITS; // 8192 values, 0..8191
export const GROUP_BYTES = 13;
export const GROUP_CHARS = 16;

/** Pair values: 91 * 91. The packer reaches 8192 of them, the format the rest. */
export const PAIR_MAX = 91 * 91; // 8281

/** `--`. Never a packed symbol, so it needs no escaping rule anywhere. */
export const SEPARATOR_VALUE = PAIR_MAX - 1; // 8280

// The side channel. There are 88 free pair values below the separator --
// 8192..8279 -- so 88 symbol values can be written as one of those instead,
// which is one bit at no cost in characters. *Which* 88 is a free choice, and
// the measurement says it matters more than anything else here.
//
// Thirteen-bit symbols are nothing like uniformly distributed, so a window
// taken as a contiguous run of the range collapses on real data. Measured over
// raw and LZ4-compressed text, source, JSON, CSV, XML, images, uniform bytes
// and zeros -- forty distributions in all:
//
//   window                worst case    mean
//   the top 88 values     0.000 %       0.5 %
//   the bottom 88         0.000 %       11 %
//   every 91st value      0.55 %        4.4 %
//   v * 8179 mod 8192     0.52 %        4.4 %
//
// The bottom window carries the most where it works -- LZ4 writes two-byte
// offsets whose high byte is zero whenever a match is near, and those make
// small symbols -- and carries nothing at all on repeated raw text. A check
// that can vanish is not a check, so the choice goes on the worst case.
//
// Every scattered window lands within noise of every other, and the multiplier
// was picked on synthetic shapes and then checked against the corpus, which
// the search never saw. 8179 is -13 modulo 8192, and thirteen is the symbol
// width: multiplying by it walks the window one step per bit-alignment class
// rather than along the grain of the data. Nothing here is load-bearing -- the
// format decodes with an empty side channel, it simply has no check pattern.
export const SIDE_COUNT = 88;
export const SIDE_MIX = 8179; // -13 mod 8192
export const SIDE_UNMIX = 4411; // its inverse: 8179 * 4411 = 1 mod 8192
export const SIDE_MAX = SYMBOL_MAX + SIDE_COUNT - 1; // 8279, one below "--"

/** Which slot a symbol owns, if any: below SIDE_COUNT means it has one. */
export const sideSlot = (v) => (v * SIDE_MIX) & (SYMBOL_MAX - 1);

/** Whether a (corrected) symbol value sits in the side-channel window. */
export const carriesSide = (v) => sideSlot(v) < SIDE_COUNT;

/** The pair value that says "this symbol, and a one bit". */
export const raiseSide = (v) => SYMBOL_MAX + sideSlot(v);

/** The inverse. Defined for every value 8192..8280, so a damaged separator
 *  lands on a symbol the field has rather than outside it. */
export const lowerSide = (u) => ((u - SYMBOL_MAX) * SIDE_UNMIX) & (SYMBOL_MAX - 1);
/** Characters that are not pairs of this alphabet at all. Part of the one
 *  error family so that a caller has a single type to catch and a code to
 *  switch on, whichever layer refused. */
export class PackError extends Base91JdpError {
  constructor(message, code = ERR.MALFORMED_PAIRS) {
    super(code, message);
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

/** Every value as a full pair, low digit first. Values up to 8280 are fine:
 *  that is what the separator and the side channel are made of. */
export function charsFromSymbols(symbols) {
  const out = new Uint8Array(symbols.length * 2);
  for (let i = 0; i < symbols.length; i++) {
    const v = symbols[i];
    out[2 * i] = CHR[v % 91];
    out[2 * i + 1] = CHR[(v / 91) | 0];
  }
  return latin1(out);
}

/** The inverse, keeping every value the characters actually spell, 0..8280.
 *  Takes a string or the character codes of one. */
export function pairsFromChars(text) {
  if (text.length % 2 !== 0) throw new PackError('an odd number of characters');
  const at = typeof text === 'string' ? (i) => text.charCodeAt(i) : (i) => text[i];
  const pairs = new Uint16Array(text.length / 2);
  for (let i = 0; i < pairs.length; i++) {
    pairs[i] = digit(at(2 * i)) + digit(at(2 * i + 1)) * 91;
  }
  return pairs;
}

/** The inverse as symbols. A pair outside 0..8191 is clamped, not rejected:
 *  handing the damaged symbol to the error-correcting layer is the point. */
export function symbolsFromChars(text) {
  const pairs = pairsFromChars(text);
  for (let i = 0; i < pairs.length; i++) pairs[i] &= SYMBOL_MAX - 1;
  return pairs;
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
    out[o++] = CHR[v % 91];
    out[o++] = CHR[(v / 91) | 0];
  }
  if (tail === 1) {
    out[o++] = CHR[symbols[full]];
  } else if (tail === 2) {
    const v = symbols[full];
    out[o++] = CHR[v % 91];
    out[o++] = CHR[(v / 91) | 0];
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
    symbols[i] = (d0 + d1 * 91) & (SYMBOL_MAX - 1);
  }
  if (tailBits) {
    let v;
    if (tailBits <= 6) {
      v = digit(text.charCodeAt(full * 2));
    } else {
      const d0 = digit(text.charCodeAt(full * 2));
      const d1 = digit(text.charCodeAt(full * 2 + 1));
      v = d0 + d1 * 91;
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
// Side channel
// ---------------------------------------------------------------------

/**
 * Write bits into a run of symbols, in place. A symbol in the window is
 * rewritten as its reserved pair value to mean 1 and left alone to mean 0; the
 * character count does not move either way, so the bits are free.
 *
 * @param {Uint16Array} symbols packed values, 0..8191
 * @param {(slot: number) => number} bitAt called once per slot, in order
 * @returns {number} how many slots this run offered
 */
export function writeSide(symbols, bitAt) {
  let slot = 0;
  for (let i = 0; i < symbols.length; i++) {
    if (carriesSide(symbols[i])) {
      if (bitAt(slot)) symbols[i] = raiseSide(symbols[i]);
      slot++;
    }
  }
  return slot;
}

/**
 * Read the side channel back.
 *
 * The slots are found from `symbols`, the values *after* error correction, and
 * the bit values from `wire`, the values the characters actually spelled. That
 * is what keeps a damaged symbol from shifting every bit that follows it: it
 * costs one bit, at a position the reader knows is untrustworthy, because the
 * wire value is then neither of the two the corrected value allows.
 *
 * @returns {{bits: Uint8Array, trusted: Uint8Array}}
 */
export function readSide(wire, symbols) {
  const n = countSideSlots(symbols);
  const bits = new Uint8Array(n);
  const trusted = new Uint8Array(n);
  let slot = 0;
  for (let i = 0; i < symbols.length; i++) {
    if (!carriesSide(symbols[i])) continue;
    const raised = raiseSide(symbols[i]);
    bits[slot] = wire[i] === raised ? 1 : 0;
    trusted[slot] = wire[i] === symbols[i] || wire[i] === raised ? 1 : 0;
    slot++;
  }
  return { bits, trusted };
}

export function countSideSlots(symbols) {
  let n = 0;
  for (let i = 0; i < symbols.length; i++) if (carriesSide(symbols[i])) n++;
  return n;
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
