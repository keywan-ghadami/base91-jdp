// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Reed-Solomon over GF(2^m), systematic, for any m the caller asks for.
//
// Two fields are in use here and they answer different questions:
//
//   GF(2^8)   symbols are bytes. The obvious choice, and the wrong one for
//             this channel: a 13-bit character pair straddles two or three
//             byte boundaries, so one bad character becomes two or three
//             symbol errors and the code has to be three times stronger than
//             the damage warrants.
//   GF(2^13)  symbols are character pairs. One bad character is exactly one
//             symbol error, and codewords may run to 8191 symbols -- 13.3 kB
//             of payload -- so a single correction pair costs 0.024 %.
//
// The generator polynomial has roots alpha^1 .. alpha^(2t), so a codeword C
// with C(alpha^j) = 0 for those j is error-free.
//
// Codewords are shortened rather than padded: a message of k symbols produces
// k + 2t, and the decoder works in that shortened frame. A shortened codeword
// is a full one whose leading symbols are zero, and leading zeros contribute
// nothing to a polynomial evaluation, so the syndromes, Berlekamp-Massey,
// Chien search and Forney formula all apply unchanged with the shortened
// length in place of q-1.

/**
 * Build the arithmetic of GF(2^m).
 *
 * @param {number} m     field width in bits
 * @param {number} poly  primitive polynomial, with the x^m term included
 */
export function makeField(m, poly) {
  const q = 1 << m;
  const order = q - 1; // the multiplicative group, and the longest codeword
  const EXP = new Uint16Array(2 * order);
  const LOG = new Uint16Array(q);

  let x = 1;
  for (let i = 0; i < order; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    if (x & q) x ^= poly;
  }
  if (x !== 1) throw new Error(`0x${poly.toString(16)} is not primitive over GF(2^${m})`);
  for (let i = order; i < 2 * order; i++) EXP[i] = EXP[i - order];

  const mul = (a, b) => (a === 0 || b === 0 ? 0 : EXP[LOG[a] + LOG[b]]);
  const inv = (a) => EXP[order - LOG[a]];
  // alpha^e for any integer e, positive or negative
  const pow = (e) => EXP[(((e % order) + order) % order)];

  return { m, q, order, EXP, LOG, mul, inv, pow, Storage: m <= 8 ? Uint8Array : Uint16Array };
}

/** Symbols are bytes. */
export const GF8 = makeField(8, 0x11d);
/** Symbols are 13-bit character pairs -- one per two output characters. */
export const GF13 = makeField(13, 0x201b);

export class UncorrectableError extends Error {
  constructor(message) {
    super(message);
    this.name = 'UncorrectableError';
    this.code = 'UNCORRECTABLE';
  }
}

/**
 * A Reed-Solomon codec over one field.
 *
 * `encode(msg, nsym)` appends nsym parity symbols; `decode(cw, nsym)` repairs
 * the codeword in place and returns how many symbols it had to touch.
 */
export function makeRS(field) {
  const { order, mul, inv, pow, Storage } = field;
  const generators = new Map();

  /** Generator polynomial for `nsym` parity symbols, highest degree first. */
  function generator(nsym) {
    let g = [1];
    for (let i = 0; i < nsym; i++) {
      const next = new Array(g.length + 1).fill(0);
      for (let j = 0; j < g.length; j++) {
        next[j] ^= g[j];
        next[j + 1] ^= mul(g[j], pow(i + 1));
      }
      g = next;
    }
    return g;
  }

  const genFor = (nsym) => {
    let g = generators.get(nsym);
    if (!g) generators.set(nsym, (g = generator(nsym)));
    return g;
  };

  function encode(msg, nsym) {
    if (msg.length + nsym > order) throw new RangeError('codeword longer than the field allows');
    const g = genFor(nsym);
    const out = new Storage(msg.length + nsym);
    out.set(msg);
    for (let i = 0; i < msg.length; i++) {
      const coef = out[i];
      if (coef !== 0) {
        for (let j = 1; j <= nsym; j++) out[i + j] ^= mul(g[j], coef);
      }
    }
    out.set(msg);
    return out;
  }

  /** S_j = C(alpha^j) for j = 1..nsym; null when every one of them is zero. */
  function syndromes(cw, nsym) {
    const s = new Storage(nsym);
    let any = false;
    for (let j = 0; j < nsym; j++) {
      const a = pow(j + 1);
      let v = 0;
      for (let i = 0; i < cw.length; i++) v = mul(v, a) ^ cw[i];
      s[j] = v;
      if (v !== 0) any = true;
    }
    return any ? s : null;
  }

  /** Berlekamp-Massey: the error locator polynomial, lowest degree first. */
  function errorLocator(s, nsym) {
    let lambda = [1];
    let prev = [1];
    let shift = 1;
    let b = 1;
    for (let n = 0; n < nsym; n++) {
      let delta = s[n];
      for (let i = 1; i < lambda.length; i++) delta ^= mul(lambda[i], s[n - i]);
      if (delta === 0) {
        shift++;
        continue;
      }
      const scale = mul(delta, inv(b));
      const next = lambda.slice();
      for (let i = 0; i < prev.length; i++) {
        const at = i + shift;
        while (next.length <= at) next.push(0);
        next[at] ^= mul(prev[i], scale);
      }
      if (2 * (lambda.length - 1) <= n) {
        prev = lambda;
        b = delta;
        shift = 1;
      } else {
        shift++;
      }
      lambda = next;
    }
    return lambda;
  }

  /** Chien search: the positions, left to right, at which Lambda vanishes. */
  function errorPositions(lambda, length) {
    const out = [];
    for (let i = 0; i < length; i++) {
      // symbol i has locator alpha^(length-1-i); it is in error exactly when
      // Lambda vanishes at that locator's inverse
      const xInv = pow(-(length - 1 - i));
      let v = 0;
      let p = 1;
      for (let d = 0; d < lambda.length; d++) {
        v ^= mul(lambda[d], p);
        p = mul(p, xInv);
      }
      if (v === 0) out.push(i);
    }
    return out;
  }

  /** Forney: the magnitude of the error at each located position. */
  function applyCorrections(cw, s, lambda, positions, nsym) {
    // Omega(x) = S(x) * Lambda(x) mod x^nsym
    const omega = new Storage(nsym);
    for (let i = 0; i < nsym; i++) {
      let v = 0;
      for (let j = 0; j <= i && j < lambda.length; j++) v ^= mul(lambda[j], s[i - j]);
      omega[i] = v;
    }
    for (const pos of positions) {
      const xInv = pow(-(cw.length - 1 - pos));
      let num = 0;
      let p = 1;
      for (let d = 0; d < nsym; d++) {
        num ^= mul(omega[d], p);
        p = mul(p, xInv);
      }
      // the formal derivative of Lambda keeps only its odd-degree terms
      let den = 0;
      p = 1;
      const xInv2 = mul(xInv, xInv);
      for (let d = 1; d < lambda.length; d += 2) {
        den ^= mul(lambda[d], p);
        p = mul(p, xInv2);
      }
      if (den === 0) return false;
      cw[pos] ^= mul(num, inv(den));
    }
    return true;
  }

  /**
   * Repair a codeword in place, returning the number of symbols corrected.
   *
   * Up to `nsym / 2` symbol errors anywhere in the codeword are recoverable.
   * Beyond that the decoder either says so, or -- rarely -- lands on a
   * different valid codeword and returns quietly wrong data. That is a
   * property of the code, not of this implementation, and it is why the
   * format that uses this has to decide separately whether it wants a
   * checksum.
   */
  function decode(cw, nsym) {
    const s = syndromes(cw, nsym);
    if (s === null) return 0;
    const lambda = errorLocator(s, nsym);
    const count = lambda.length - 1;
    if (count > nsym / 2) throw new UncorrectableError(`${count} errors exceed the code`);
    const positions = errorPositions(lambda, cw.length);
    if (positions.length !== count) {
      throw new UncorrectableError('the error locator has no consistent root set');
    }
    if (!applyCorrections(cw, s, lambda, positions, nsym)) {
      throw new UncorrectableError('a located error has no magnitude');
    }
    if (syndromes(cw, nsym) !== null) {
      throw new UncorrectableError('the codeword is still wrong after correction');
    }
    return count;
  }

  return { field, encode, decode, maxCodeword: order };
}

export const RS8 = makeRS(GF8);
export const RS13 = makeRS(GF13);

// Backwards-compatible byte-level entry points.
export const MAX_CODEWORD = GF8.order;
export const encode = (msg, nsym) => RS8.encode(msg, nsym);
export const decode = (cw, nsym) => RS8.decode(cw, nsym);
