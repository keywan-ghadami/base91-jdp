// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * base91-jdp -- basE91 on a JSON-safe alphabet, with Dynamic Passthrough.
 *
 * The block coder is basE91 (Joachim Henke, 2005) with `"` swapped out of the
 * alphabet for `-`, which leaves the alphabet free of every character a JSON
 * string has to escape. `-` lands on the alphabet's last value, so the pair
 * `--` becomes the one value the block coder can never emit -- and that is the
 * signal that switches Dynamic Passthrough on and off.
 *
 * See spec/base91-jdp-v0.2.0.md for the format.
 */

import { makeCodec } from './codec.js';
import { PROFILES } from './profiles.js';

export {
  ALPHABET,
  R_CHARS,
  R_NAMES,
  R_LEN,
  BLOCK_THRESHOLD,
  SIGNAL_VALUE,
  Base91JdpError,
  ERR,
} from './codec.js';
export { PROFILES } from './profiles.js';
export { makeCodec } from './codec.js';

/** The constants of spec section 6.9. */
export const CONSTANTS = Object.freeze({
  MIN_DP_BYTES: 26,
  MIN_BINARY_RUN: 4,
  MAX_DP_BYTES: 65536,
  HEADER_CHARS: 2,
  NUM_PROFILES: PROFILES.length,
});

const codec = makeCodec({
  profiles: PROFILES,
  maskMode: 'exact',
  headerChars: CONSTANTS.HEADER_CHARS,
  minDpBytes: CONSTANTS.MIN_DP_BYTES,
  minBinaryRun: CONSTANTS.MIN_BINARY_RUN,
  maxDpBytes: CONSTANTS.MAX_DP_BYTES,
});

/**
 * Encode bytes.
 *
 * @param {Uint8Array|ArrayLike<number>} bytes
 * @returns {string} characters from the alphabet only, safe inside a JSON string
 */
export const encode = (bytes) => codec.encode(bytes);

/**
 * Decode. Whitespace in the input is skipped, so wrapped output decodes as-is.
 *
 * @param {string|Uint8Array} text
 * @returns {Uint8Array}
 * @throws {Base91JdpError} on malformed input
 */
export const decode = (text) => codec.decode(text);

/** Encode a string as UTF-8. */
export const encodeText = (text) => codec.encode(new TextEncoder().encode(text));

/** Decode to a string, rejecting invalid UTF-8. */
export const decodeText = (text) =>
  new TextDecoder('utf-8', { fatal: true }).decode(codec.decode(text));
