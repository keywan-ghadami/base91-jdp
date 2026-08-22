// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Reference encoders used only for comparison in the benchmark. They are
// deliberately simple: the benchmark measures output length, not speed.

import { BASE91_ALPHABET } from '../src/codec.js';

/** basE91 (Joachim Henke, 2005), the original. */
export function base91Encode(bytes) {
  const A = BASE91_ALPHABET;
  let b = 0;
  let n = 0;
  let out = '';
  for (const byte of bytes) {
    b |= byte << n;
    n += 8;
    if (n > 13) {
      let v = b & 8191;
      if (v > 88) {
        b >>= 13;
        n -= 13;
      } else {
        v = b & 16383;
        b >>= 14;
        n -= 14;
      }
      out += A[v % 91] + A[(v / 91) | 0];
    }
  }
  if (n) {
    out += A[b % 91];
    if (n > 7 || b > 90) out += A[(b / 91) | 0];
  }
  return out;
}

export function base91Decode(text) {
  const A = BASE91_ALPHABET;
  const V = new Int16Array(256).fill(-1);
  for (let i = 0; i < A.length; i++) V[A.charCodeAt(i)] = i;
  const out = [];
  let b = 0;
  let n = 0;
  let v = -1;
  for (const ch of text) {
    const d = V[ch.charCodeAt(0)];
    if (d < 0) continue;
    if (v < 0) {
      v = d;
    } else {
      v += d * 91;
      b |= v << n;
      n += (v & 8191) > 88 ? 13 : 14;
      do {
        out.push(b & 0xff);
        b >>= 8;
        n -= 8;
      } while (n > 7);
      v = -1;
    }
  }
  if (v >= 0) out.push((b | (v << n)) & 0xff);
  return Uint8Array.from(out);
}

export const base64Encode = (bytes) => Buffer.from(bytes).toString('base64');

/** Ascii85 (btoa dialect, no <~ ~> wrapper, 'z' for a zero group). */
export function ascii85Encode(bytes) {
  let out = '';
  for (let i = 0; i < bytes.length; i += 4) {
    const rest = Math.min(4, bytes.length - i);
    let v = 0;
    for (let k = 0; k < 4; k++) v = v * 256 + (k < rest ? bytes[i + k] : 0);
    if (v === 0 && rest === 4) {
      out += 'z';
      continue;
    }
    const chars = [];
    for (let k = 0; k < 5; k++) {
      chars.unshift(String.fromCharCode(33 + (v % 85)));
      v = Math.floor(v / 85);
    }
    out += chars.join('').slice(0, rest + 1);
  }
  return out;
}

/**
 * The length of `text` once it sits inside a JSON string, i.e. after
 * JSON.stringify has escaped whatever needs escaping.
 */
export const jsonEmbeddedLength = (text) => JSON.stringify(text).length - 2;
