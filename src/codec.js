// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// base91-jdp: basE91 with a JSON-safe alphabet and Dynamic Passthrough.
//
// This file is the parameterised core. `src/index.js` binds it to the frozen
// constants of the specification; `bench/sweep.js` and `tools/deriveprofiles.js`
// bind it to variants, which is how those constants were chosen.

// ---------------------------------------------------------------------
// Alphabet (spec section 4)
// ---------------------------------------------------------------------

// basE91 (Joachim Henke, 2005) with '"' replaced by '-'.  The alphabet then
// contains none of JSON's string-syntax characters (`"`, `\`) and none of its
// control characters, so encoded output drops into a JSON string verbatim.
// '-' lands on value 90, the last, which puts the pair "--" at 8280 -- above
// everything thirteen bits can spell, and so a signal that needs no escaping.
export const ALPHABET =
  'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789' +
  '!#$%&()*+,./:;<=>?@[]^_`{|}~-';

export const BASE91_ALPHABET =
  'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789' +
  '!#$%&()*+,./:;<=>?@[]^_`{|}~"';

export const SIGNAL_VALUE = 8280; // 90 + 90 * 91, the pair "--"
export const HYPHEN = 0x2d;

// A pair holds 8281 values and the block coder writes 8192 of them: thirteen
// bits, always. That is what leaves 8192..8280 free -- the separator above and
// the mode markers of src/marker.js -- and what makes one pair one GF(2^13)
// symbol, which is the whole basis of the error correction. A pair value in
// 8192..8279 inside a stream is therefore not a symbol at all; it is a stream
// that has been damaged or was never ours.
export const SYMBOL_BITS = 13;
export const SYMBOL_MAX = 1 << SYMBOL_BITS;

/**
 * An alphabet with `ch` moved to value 90, so that the reserved pair becomes
 * `ch` twice. Only used by the benchmark, to measure what the choice of
 * signal character costs; the format fixes it at `-`.
 */
export function alphabetWithSignal(ch) {
  const i = ALPHABET.indexOf(ch);
  if (i < 0) throw new Error(`${ch} is not in the alphabet`);
  const a = [...ALPHABET];
  [a[i], a[90]] = [a[90], a[i]];
  return a.join('');
}


// The R-Set: the eight bytes a passthrough segment substitutes.  Seven of them
// real text is full of and the alphabet does not contain; the eighth is '-'
// itself, which is in the alphabet but cannot be written literally, since two
// of them in a row would end the segment.  Substituting it instead means a
// payload never contains '-' at all, so the exit signal needs no escaping
// rule and text full of '--' costs one donor rather than a mode switch each
// time.  The index j is normative: it fixes the bit positions of mask.
export const R_CHARS = [0x20, 0x22, 0x0a, 0x5c, 0x0d, 0x27, 0x09, 0x2d];
export const R_NAMES = ['space', '"', 'LF', '\\', 'CR', "'", 'TAB', '-'];
export const R_LEN = R_CHARS.length;

export const ERR = {
  INVALID_CHARACTER: 'INVALID_CHARACTER',
  UNEXPECTED_EOS: 'UNEXPECTED_EOS',
  UNDEFINED_SIGNAL: 'UNDEFINED_SIGNAL',
  INVALID_FLUSH: 'INVALID_FLUSH',
  INVALID_FINAL_BLOCK: 'INVALID_FINAL_BLOCK',
  // A pair in 8192..8279: no block coder can write one, so the stream is
  // damaged, or it is a framed stream being read as a headerless one.
  RESERVED_PAIR: 'RESERVED_PAIR',
};

export class Base91JdpError extends Error {
  constructor(code, message) {
    super(message || code);
    this.name = 'Base91JdpError';
    this.code = code;
  }
}

const fail = (code, msg) => {
  throw new Base91JdpError(code, msg);
};

// ---------------------------------------------------------------------
// Codec construction
// ---------------------------------------------------------------------

// Only a fallback for makeCodec() calls that name no profiles; the shipped
// codec passes the derived table of src/profiles.js.
export const DEFAULT_PROFILES = [['$', '~', '^', '%', '#', '@', '>', '<']];

/**
 * Build a codec from a configuration.
 *
 * @param {object} cfg
 * @param {string[][]} cfg.profiles     donor rankings, one per R-Set member
 * @param {number[]}   cfg.rChars       the substituted byte values, in mask-bit order
 * @param {string}     cfg.maskMode     'exact' | 'prefix' | 'none'
 * @param {number}     cfg.headerChars  header width, in characters
 * @param {number}     cfg.minDpBytes   shortest passthrough segment
 * @param {number}     cfg.minBinaryRun bytes of block mode before DP may resume
 * @param {number}     cfg.maxDpBytes   longest passthrough segment
 */
export function makeCodec(cfg = {}) {
  const alphabet = cfg.alphabet ?? ALPHABET;
  const rChars = cfg.rChars ?? R_CHARS;
  const rLen = rChars.length;
  if (alphabet.length !== 91 || new Set(alphabet).size !== 91) {
    throw new Error('the alphabet must be 91 distinct characters');
  }
  // The pair the block coder can never produce is value 90 twice, so the
  // character sitting on value 90 is the one the mode signal is made of.
  const SIG = alphabet.charCodeAt(90);
  const profiles = (cfg.profiles ?? DEFAULT_PROFILES).map((p) =>
    typeof p === 'string' ? [...p] : p,
  );
  const maskMode = cfg.maskMode ?? 'exact';
  if (!['exact', 'prefix', 'none'].includes(maskMode)) {
    throw new Error(`unknown maskMode ${maskMode}`);
  }
  const exactMask = maskMode === 'exact';
  const headerChars = cfg.headerChars ?? 2;
  const minDpBytes = cfg.minDpBytes ?? 24;
  const minBinaryRun = cfg.minBinaryRun ?? 1;
  const maxDpBytes = cfg.maxDpBytes ?? 4096;
  const numProfiles = profiles.length;

  // --- derived tables ------------------------------------------------
  const VAL = new Int16Array(256).fill(-1); // byte -> alphabet value
  for (let v = 0; v < alphabet.length; v++) VAL[alphabet.charCodeAt(v)] = v;
  const CHR = new Uint8Array(91); // alphabet value -> byte
  for (let v = 0; v < alphabet.length; v++) CHR[v] = alphabet.charCodeAt(v);

  const RIDX = new Int8Array(256).fill(-1); // byte -> R-Set index
  rChars.forEach((c, j) => (RIDX[c] = j));

  // ranks[c * numProfiles + p] is the position byte c holds in profile p, or
  // rLen when it is absent from that profile.  A profile stays viable while
  // no literal it has seen ranks below k, so "absent" and "ranked below no
  // possible k" have to be the same number.
  const RANK = new Uint8Array(256 * numProfiles).fill(rLen);
  profiles.forEach((prof, p) => {
    if (prof.length !== rLen) throw new Error(`profile must have ${rLen} donors`);
    if (new Set(prof).size !== rLen) throw new Error('donors must be distinct');
    prof.forEach((ch, r) => {
      const c = ch.charCodeAt(0);
      if (VAL[c] < 0) throw new Error(`donor ${ch} is not in the alphabet`);
      if (c === SIG)
        throw new Error(`${ch} carries the mode signal and may not be a donor`);
      RANK[c * numProfiles + p] = r;
    });
  });

  const headerCapacity = 91 ** headerChars;
  // What the header's mask field can say: every subset (exact), how many of
  // the frequency-ordered R-Set characters are covered (prefix), or nothing,
  // in which case every segment spends every donor (none).
  const maxMaskStates =
    maskMode === 'exact' ? 1 << rLen : maskMode === 'prefix' ? rLen + 1 : 1;
  if (2 * maxMaskStates * numProfiles > headerCapacity) {
    throw new Error(
      `header of ${headerChars} char(s) cannot carry ${numProfiles} profiles` +
        (exactMask ? ' and a mask' : ''),
    );
  }
  const HEADER_LIMIT = 2 * maxMaskStates * numProfiles;

  const maskToState = (mask) =>
    maskMode === 'exact' ? mask : maskMode === 'prefix' ? 32 - Math.clz32(mask) : 0;
  const stateToMask = (state) =>
    maskMode === 'exact' ? state : maskMode === 'prefix' ? (1 << state) - 1 : (1 << rLen) - 1;

  const packHeader = (hi, mask, profile) =>
    hi + 2 * (maskToState(mask) + maxMaskStates * profile);
  const unpackHeader = (h) => {
    const hi = h & 1;
    const rest = (h - hi) / 2;
    const state = rest % maxMaskStates;
    return { hi, mask: stateToMask(state), profile: (rest - state) / maxMaskStates };
  };

  // donorTable[j] -> byte, for the active bits of mask (spec section 4.3)
  function donorsFor(mask, profile) {
    const prof = profiles[profile];
    const out = new Int16Array(rLen).fill(-1);
    let rank = 0;
    for (let j = 0; j < rLen; j++) {
      if (mask & (1 << j)) out[j] = prof[rank++].charCodeAt(0);
    }
    return out;
  }

  // ---------------------------------------------------------------------
  // Passthrough prefix scan (spec section 6.2)
  // ---------------------------------------------------------------------
  const minDonor = new Uint8Array(numProfiles);
  const tentative = new Uint8Array(numProfiles);

  function dpScan(input, pos, end) {
    minDonor.fill(rLen);
    let mask = 0;
    let k = maskMode === 'none' ? rLen : 0;
    let profile = 0;
    let i = 0;
    let prev = -1;
    let stop = 'eos';
    const limit = Math.min(end - pos, maxDpBytes);

    while (i < limit) {
      const c = input[pos + i];
      // "--" is the mode signal; it can never appear inside a segment. When
      // the signal character is itself an R-Set member it is substituted
      // away instead, and this can never fire.
      if (c === SIG && prev === SIG && RIDX[c] < 0) {
        stop = 'signal';
        break;
      }

      const j = RIDX[c];
      let newK = k;
      let newMask = mask;
      if (j >= 0) {
        if (mask & (1 << j)) {
          prev = c;
          i++;
          continue;
        }
        // 'prefix' can only widen the mask to a prefix of the R-Set, so a
        // rarer R-Set character drags every commoner one in with it.
        newMask = maskMode === 'prefix' ? (1 << (j + 1)) - 1 : mask | (1 << j);
        if (maskMode !== 'none') newK = popcount(newMask);
        tentative.set(minDonor);
      } else {
        if (VAL[c] < 0) {
          stop = 'unrepresentable';
          break; // no alphabet character stands for this byte
        }
        let changed = false;
        const base = c * numProfiles;
        for (let p = 0; p < numProfiles; p++) {
          const r = RANK[base + p];
          const m = r < minDonor[p] ? r : minDonor[p];
          tentative[p] = m;
          if (m !== minDonor[p]) changed = true;
        }
        if (!changed) {
          prev = c;
          i++;
          continue;
        }
      }

      let np = -1;
      for (let p = 0; p < numProfiles; p++) {
        if (tentative[p] >= newK) {
          np = p;
          break;
        }
      }
      if (np < 0) {
        stop = 'donor';
        break; // no profile can lend that many donors
      }

      if (j >= 0) mask = newMask;
      minDonor.set(tentative);
      k = newK;
      profile = np;
      prev = c;
      i++;
    }

    // A segment may not end on '-': the exit signal would glue onto it and
    // the decoder would cut the segment one character early.
    if (i > 0 && pos + i < end && input[pos + i - 1] === SIG && RIDX[SIG] < 0) i--;

    if (i === limit && limit < end - pos) stop = 'cap';
    if (maskMode === 'none') mask = (1 << rLen) - 1;
    return { L: i, mask, profile, stop };
  }

  // ---------------------------------------------------------------------
  // Encoder (spec section 6)
  // ---------------------------------------------------------------------

  function encode(input, stats) {
    const bytes =
      input instanceof Uint8Array ? input : new Uint8Array(input);
    const end = bytes.length;
    let out = new Uint8Array(Math.ceil(end * 1.3) + 64);
    let o = 0;
    const need = (extra) => {
      if (o + extra > out.length) {
        const bigger = new Uint8Array(Math.max(out.length * 2, o + extra + 64));
        bigger.set(out.subarray(0, o));
        out = bigger;
      }
    };

    // The block coder's accumulator, most significant bit first: bytes go in
    // at the bottom, thirteen-bit symbols come off the top. `n` is how many
    // bits are held, never more than twelve between symbols.
    let b = 0;
    let n = 0;
    let pos = 0;
    let binaryRun = Infinity; // no exit signal has been paid at the start
    let inDp = false;

    // The bits owed on bytes already read. One character carries up to six of
    // them, two carry up to twelve, which is the most there can ever be.
    const flushPending = () => {
      if (n === 0) return;
      const w = b & ((1 << n) - 1);
      out[o++] = CHR[w % 91];
      if (n > 6) out[o++] = CHR[(w / 91) | 0];
      b = 0;
      n = 0;
    };

    while (pos < end) {
      const scan =
        binaryRun >= minBinaryRun ? dpScan(bytes, pos, end) : { L: 0 };

      if (scan.L >= minDpBytes) {
        if (stats) {
          stats.dpSegments++;
          stats.dpBytes += scan.L;
          stats.stops[scan.stop] = (stats.stops[scan.stop] ?? 0) + 1;
        }
        need(6 + headerChars + scan.L);
        // exit block mode: signal, header, then the pending bits
        out[o++] = SIG;
        out[o++] = SIG;
        // The decoder can work out the pending count modulo eight from the
        // symbols it has read; the header's low bit says which of the two
        // candidates below thirteen it was.
        const hi = n >= 8 ? 1 : 0;
        let h = packHeader(hi, scan.mask, scan.profile);
        for (let t = 0; t < headerChars; t++) {
          out[o++] = CHR[h % 91];
          h = (h / 91) | 0;
        }
        flushPending();

        const donor = donorsFor(scan.mask, scan.profile);
        for (let i = 0; i < scan.L; i++) {
          const c = bytes[pos + i];
          const j = RIDX[c];
          out[o++] = j >= 0 ? donor[j] : c;
        }
        pos += scan.L;
        inDp = true;
        if (pos < end) {
          out[o++] = SIG;
          out[o++] = SIG;
          inDp = false;
        }
        binaryRun = 0;
      } else {
        // block mode, one byte at a time (spec section 6.3)
        if (stats) stats.blockBytes++;
        b = (b << 8) | bytes[pos++];
        n += 8;
        if (n >= SYMBOL_BITS) {
          n -= SYMBOL_BITS;
          const v = (b >>> n) & (SYMBOL_MAX - 1);
          b &= (1 << n) - 1;
          need(2);
          out[o++] = CHR[v % 91];
          out[o++] = CHR[(v / 91) | 0];
        }
        binaryRun++;
      }
    }

    if (!inDp) {
      need(2);
      flushPending();
    }
    return latin1(out.subarray(0, o));
  }

  // ---------------------------------------------------------------------
  // Decoder (spec section 7)
  // ---------------------------------------------------------------------

  function decode(text) {
    // Whitespace is never significant -- the alphabet contains none of it --
    // so it comes out first. What is left is counted, and the count is what
    // tells the final group how wide it is.
    const src = significant(text);
    const len = src.length;
    let out = new Uint8Array(len + 16);
    let o = 0;
    const emit = (byte) => {
      if (o >= out.length) {
        const bigger = new Uint8Array(out.length * 2);
        bigger.set(out);
        out = bigger;
      }
      out[o++] = byte;
    };

    // The mirror of the encoder's accumulator: symbols go in at the bottom,
    // bytes come off the top, and `n` is what is left over, always under eight.
    let b = 0;
    let n = 0;
    let i = 0;
    let inDp = false;
    let donor = null;
    let fromDonor = null;

    const emitBits = (width, value) => {
      b = (b << width) | value;
      n += width;
      while (n >= 8) {
        n -= 8;
        emit((b >>> n) & 0xff);
      }
      b &= (1 << n) - 1;
    };

    const next = () => (i < len ? src[i++] : -1);
    const peek = () => (i < len ? src[i] : -1);
    const digit = (c) => {
      if (c < 0) fail(ERR.UNEXPECTED_EOS, 'input ends inside a group');
      const v = VAL[c];
      if (v < 0)
        fail(
          ERR.INVALID_CHARACTER,
          `${JSON.stringify(String.fromCharCode(c))} is not in the alphabet`,
        );
      return v;
    };

    for (;;) {
      if (inDp) {
        const c = next();
        if (c < 0) break;
        if (c === SIG && peek() === SIG) {
          next();
          inDp = false;
          continue;
        }
        const j = fromDonor[c];
        if (j >= 0) {
          emit(rChars[j]);
        } else {
          digit(c); // validate: every other character stands for itself
          emit(c);
        }
        continue;
      }

      const left = len - i;
      // What the writer still owed on the byte in hand, modulo eight. It held
      // either that many bits or eight more -- never anything else, because a
      // thirteenth bit would have become a symbol.
      const owed = (8 - n) % 8;

      if (left === 0) {
        if (n !== 0) fail(ERR.INVALID_FINAL_BLOCK, 'the stream ends inside a byte');
        break;
      }

      // The final group. One character carries up to six bits and two carry
      // seven to twelve, so the character count picks between the two
      // candidates -- except after three held bits, where the only candidate
      // needs one character and two characters can only be a whole symbol.
      if (left === 1 || (left === 2 && n !== 3)) {
        const q = left === 1 ? owed : owed >= 7 ? owed : owed + 8;
        const wide = q > 6;
        if (q === 0 || q > 12 || (wide ? 2 : 1) !== left) {
          fail(ERR.INVALID_FINAL_BLOCK, `${left} trailing character(s) cannot owe ${q} bits`);
        }
        let w = digit(next());
        if (wide) w += digit(next()) * 91;
        if (w >= 1 << q)
          fail(ERR.INVALID_FINAL_BLOCK, 'the final group carries more bits than are owed');
        emitBits(q, w);
        break;
      }

      const v = digit(next()) + digit(next()) * 91;

      if (v !== SIGNAL_VALUE) {
        if (v >= SYMBOL_MAX) {
          fail(ERR.RESERVED_PAIR, `the pair value ${v} is reserved and cannot be a symbol`);
        }
        emitBits(SYMBOL_BITS, v);
        continue;
      }

      // --- mode signal ------------------------------------------------
      let h = 0;
      for (let t = 0, w = 1; t < headerChars; t++, w *= 91) h += digit(next()) * w;
      if (h >= HEADER_LIMIT)
        fail(ERR.UNDEFINED_SIGNAL, `header value ${h} is not defined`);
      const { hi, mask, profile } = unpackHeader(h);

      // The writer's pending bit count is congruent to -n mod 8; the header's
      // low bit says which of the two candidates below thirteen it was.
      const nEnc = owed + 8 * hi;
      if (nEnc > 12)
        fail(ERR.INVALID_FLUSH, `pending bit count ${nEnc} is out of range`);
      if (nEnc > 0) {
        let w = digit(next());
        if (nEnc > 6) w += digit(next()) * 91;
        if (w >= 1 << nEnc)
          fail(ERR.INVALID_FLUSH, 'pending bits carry more than they may');
        // n + nEnc is a multiple of eight by the construction of nEnc, so this
        // always empties the accumulator exactly.
        emitBits(nEnc, w);
      }
      // n is zero here, and there is no check for it: n + nEnc is a multiple
      // of eight by construction, so a flush that did not close a byte is not
      // a stream this decoder can be handed.

      donor = donorsFor(mask, profile);
      fromDonor = new Int8Array(256).fill(-1);
      for (let j = 0; j < rLen; j++) if (donor[j] >= 0) fromDonor[donor[j]] = j;
      inDp = true;
    }

    return out.subarray(0, o);
  }

  const newStats = () => ({ dpSegments: 0, dpBytes: 0, blockBytes: 0, stops: {} });

  return {
    encode,
    encodeStats(input) {
      const stats = newStats();
      stats.chars = encode(input, stats).length;
      stats.bytes = input.length;
      return stats;
    },
    decode,
    config: {
      profiles,
      maskMode,
      headerChars,
      minDpBytes,
      minBinaryRun,
      maxDpBytes,
      numProfiles,
    },
    dpScan,
    donorsFor,
  };
}

// ---------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------

function popcount(x) {
  x -= (x >> 1) & 0x55555555;
  x = (x & 0x33333333) + ((x >> 2) & 0x33333333);
  return (((x + (x >> 4)) & 0x0f0f0f0f) * 0x01010101) >> 24;
}

function latin1(bytes) {
  let s = '';
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    s += String.fromCharCode.apply(null, bytes.subarray(i, i + CHUNK));
  }
  return s;
}

/**
 * The characters of an encoded stream, with whitespace dropped. Wrapped output
 * decodes as it stands, and what is left is a count the final group can be
 * measured against.
 */
function significant(text) {
  const isString = typeof text === 'string';
  const n = text.length;
  const out = new Uint8Array(n);
  let o = 0;
  for (let i = 0; i < n; i++) {
    const c = isString ? text.charCodeAt(i) : text[i];
    if (c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0d) continue;
    if (c > 0xff)
      fail(ERR.INVALID_CHARACTER, `U+${c.toString(16)} is not in the alphabet`);
    out[o++] = c;
  }
  return out.subarray(0, o);
}
