// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * base91-jdp -- basE91 on a JSON-safe alphabet, with dynamic passthrough,
 * optional LZ4, and optional error correction.
 *
 * The block coder is basE91 (Joachim Henke, 2005) with `"` swapped out of the
 * alphabet for `-`, which leaves the alphabet free of every character a JSON
 * string has to escape. Symbols are a fixed thirteen bits, so a pair of
 * characters is one value in 0..8191 and the eighty-nine values above that are
 * free: `--` opens a passthrough segment and divides framed segments, and
 * 8192..8279 are the mode markers of src/marker.js.
 *
 * Two characters of overhead buy compression and error correction; a stream
 * that wants neither pays nothing at all.
 *
 * See spec/ for the format.
 */

import { makeCodec, significant, ERR } from './codec.js';
import { PROFILES } from './profiles.js';
import { charsFromSymbols, pairsFromChars, GROUP_BYTES } from './pack.js';
import { markerChars, readMarker, modeByName } from './marker.js';
import {
  encodeFrame, decodeFrame, frameSegments, frameChars,
  FrameError, SEGMENT_BYTES, RS_DATA, RS_PARITY,
} from './frame.js';

export {
  ALPHABET,
  R_CHARS,
  R_NAMES,
  R_LEN,
  SYMBOL_BITS,
  SYMBOL_MAX,
  SIGNAL_VALUE,
  Base91JdpError,
  ERR,
} from './codec.js';
export { PROFILES } from './profiles.js';
export { makeCodec } from './codec.js';
export { MODES, markerChars, readMarker } from './marker.js';
export { FrameError, SEGMENT_BYTES } from './frame.js';

/** The constants of the specification. */
export const CONSTANTS = Object.freeze({
  MIN_DP_BYTES: 26,
  MIN_BINARY_RUN: 4,
  MAX_DP_BYTES: 65536,
  HEADER_CHARS: 2,
  NUM_PROFILES: PROFILES.length,
  SEGMENT_BYTES,
  RS_DATA,
  RS_PARITY,
  // Below this many symbols in a segment, four parity symbols cost more than
  // 0.2 %, and the checked modes -- which cost nothing at all -- are the
  // better trade. Above it the parity is close to free.
  RS_MIN_SYMBOLS: 2048,
});

const codec = makeCodec({
  profiles: PROFILES,
  maskMode: 'exact',
  headerChars: CONSTANTS.HEADER_CHARS,
  minDpBytes: CONSTANTS.MIN_DP_BYTES,
  minBinaryRun: CONSTANTS.MIN_BINARY_RUN,
  maxDpBytes: CONSTANTS.MAX_DP_BYTES,
});

const toBytes = (input) =>
  input instanceof Uint8Array ? input : new Uint8Array(input);

/** Whether four parity symbols per codeword are worth their place here. */
function worthProtecting(byteLength) {
  const symbols = Math.ceil(((byteLength + 1 + GROUP_BYTES) * 8) / 13);
  return symbols >= CONSTANTS.RS_MIN_SYMBOLS;
}

const PROTECT_VALUES = new Set(['auto', 'check', true, false]);

/** A framed candidate: its segments compressed, its size known, unbuilt. */
function candidate(bytes, useLz4, protect) {
  const wanted = protect === 'auto' ? worthProtecting(bytes.length) : protect === true;
  const mode = modeByName(
    useLz4 ? (wanted ? 'lz4' : 'lz4Checked') : wanted ? 'stored' : 'storedChecked',
  );
  const segments = frameSegments(bytes, mode.compress);
  return {
    mode,
    segments,
    chars: 2 + frameChars(segments, mode.protect),
    build: () => markerChars(mode.value) + charsFromSymbols(encodeFrame(segments, mode)),
  };
}

/**
 * Encode bytes.
 *
 * The default weighs the candidates and returns the shortest, so the size at
 * which a marker starts paying for itself is measured rather than declared.
 *
 * `protect` answers two questions that are easy to run together and must not
 * be: whether error correction is wanted, and whether a frame is wanted at
 * all. `true` asks for Reed-Solomon and takes the frame that comes with it.
 * `'check'` asks for a frame with the side channel's check pattern but no
 * parity -- damage reported, not repaired, at no cost in characters. `false`
 * asks for no parity and leaves the frame to the size comparison, and `'auto'`
 * turns parity on once it is close to free.
 *
 * @param {Uint8Array|ArrayLike<number>} input
 * @param {object} [options]
 * @param {'auto'|'never'|'always'} [options.compress] LZ4, if it helps
 * @param {'auto'|'check'|boolean} [options.protect] error correction and framing
 * @returns {string} characters from the alphabet only, safe inside a JSON string
 */
export function encode(input, { compress = 'auto', protect = 'auto' } = {}) {
  if (!PROTECT_VALUES.has(protect)) {
    throw new RangeError(`protect must be 'auto', 'check', true or false, not ${JSON.stringify(protect)}`);
  }
  if (!['auto', 'never', 'always'].includes(compress)) {
    throw new RangeError(`compress must be 'auto', 'never' or 'always', not ${JSON.stringify(compress)}`);
  }
  const bytes = toBytes(input);
  const forceFrame = protect === true || protect === 'check';
  const headerlessAllowed = !forceFrame && compress !== 'always';

  const framed = [];
  if (compress !== 'never') framed.push(candidate(bytes, true, protect));
  // Framing without compression only wins when a frame was asked for outright:
  // otherwise the headerless stream says the same thing in fewer characters,
  // marker and parity included.
  if (forceFrame && compress !== 'always') framed.push(candidate(bytes, false, protect));

  let best = null;
  for (const c of framed) if (!best || c.chars < best.chars) best = c;

  if (!headerlessAllowed) return best.build();

  // A headerless stream is never shorter than its input: passthrough writes one
  // character per byte and the block coder writes sixteen per thirteen. So a
  // framed candidate already under that floor has won, and the passthrough
  // scan -- which costs more than everything else here put together -- can be
  // skipped outright rather than computed and thrown away.
  if (best && best.chars < bytes.length) return best.build();

  const headerless = codec.encode(bytes);
  if (best && best.chars < headerless.length) return best.build();
  return headerless;
}

/**
 * Decode.
 *
 * Whitespace in the input is skipped, so wrapped output decodes as it stands.
 *
 * @param {string|Uint8Array} text
 * @param {object} [options]
 * @param {boolean} [options.partial] return what survived instead of throwing
 * @returns {Uint8Array}
 * @throws {Base91JdpError|FrameError|RangeError} on malformed or damaged input
 */
export function decode(text, { partial = false } = {}) {
  const src = significant(text);
  if (src.length < 2) return codec.decode(src);

  const { headerless, mode } = readMarker(pairsFromChars(src.subarray(0, 2))[0]);
  if (headerless) return codec.decode(src);

  const body = decodeFrame(pairsFromChars(src.subarray(2)), mode);
  if (body.damaged.length && !partial) {
    const { segment, trouble } = body.damaged[0];
    throw new FrameError(
      `segment ${segment} of ${body.segments} could not be recovered (${trouble[0].reason})` +
        (body.damaged.length > 1 ? ` and ${body.damaged.length - 1} more` : ''),
      ERR.DAMAGED_SEGMENT,
    );
  }
  return body.bytes;
}

/**
 * Decode, reporting what happened rather than throwing.
 *
 * A damaged stream still yields every segment that survived, which is the
 * reason segments exist. `repaired` counts the symbols error correction put
 * back; `damaged` lists the segments it could not.
 */
export function decodeDetailed(text) {
  const src = significant(text);
  if (src.length < 2) {
    return { bytes: codec.decode(src), framed: false, segments: 1, damaged: [], repaired: 0 };
  }
  const { headerless, mode } = readMarker(pairsFromChars(src.subarray(0, 2))[0]);
  if (headerless) {
    return { bytes: codec.decode(src), framed: false, segments: 1, damaged: [], repaired: 0 };
  }
  return { ...decodeFrame(pairsFromChars(src.subarray(2)), mode), framed: true, mode: mode.name };
}

/** Encode a string as UTF-8. */
export const encodeText = (text, options) =>
  encode(new TextEncoder().encode(text), options);

/** Decode to a string, rejecting invalid UTF-8. */
export const decodeText = (text, options) =>
  new TextDecoder('utf-8', { fatal: true }).decode(decode(text, options));
