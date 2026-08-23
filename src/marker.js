// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What a stream says about itself, in two characters.
//
// The packer writes pair values 0..8191 and nothing else, so a stream that
// opens on a higher value is announcing something. 8280 is "--", which opens a
// passthrough segment and therefore belongs to a headerless stream that begins
// in passthrough. The eighty-eight values below it, 8192..8279, are the mode
// markers.
//
// The rule is total. A headerless stream cannot begin with a marker by
// accident, because no packer can write one -- there is no escape clause, no
// "unless the first pair happens to be", and no probability attached. That is
// the whole reason the format spends thirteen bits on a pair that could have
// held thirteen and a fraction.
//
// Every marker's second character is '-', since 91 * 90 = 8190 puts the high
// digit at 90 for everything from there up. Classic basE91 carries '"' on
// value 90 and cannot produce '-' at all, so a '-' in second place is also the
// answer to "is this ours or is it classic?" -- which is what lets classic
// basE91 stay out of band without a flag anywhere.

import { ALPHABET, SIGNAL_VALUE } from './codec.js';
import { SYMBOL_MAX } from './pack.js';

export const MARKER_MIN = SYMBOL_MAX; // 8192
export const MARKER_MAX = SIGNAL_VALUE - 1; // 8279

/**
 * The modes a marker can name.
 *
 * `compress` is LZ4 or nothing; `protect` is Reed-Solomon or nothing. The
 * check pattern in the side channel is in every framed mode, protected or not,
 * because it costs no characters -- the difference between the two is whether
 * damage can be repaired or only reported.
 *
 * Passthrough appears in none of them: it is what a headerless stream does,
 * and it cannot coexist with either of these. Compressed bytes have nothing a
 * passthrough segment could carry, and passthrough writes one character per
 * byte, which destroys the pair grid the error correction is counted on.
 */
export const MODES = Object.freeze({
  stored: { value: 8279, compress: false, protect: true },
  lz4: { value: 8278, compress: true, protect: true },
  storedChecked: { value: 8277, compress: false, protect: false },
  lz4Checked: { value: 8276, compress: true, protect: false },
});

/** Reserved: "a longer header follows". Nothing spends it yet, and that is
 *  the point -- it is what keeps eighty-eight values from being a ceiling. */
export const ESCAPE = 8266;

const BY_VALUE = new Map(Object.entries(MODES).map(([name, m]) => [m.value, { name, ...m }]));

/** The two characters of a marker value. */
export function markerChars(value) {
  if (value < MARKER_MIN || value > MARKER_MAX) {
    throw new RangeError(`${value} is not a marker value`);
  }
  return ALPHABET[value % 91] + ALPHABET[(value / 91) | 0];
}

/**
 * What the first pair of a stream says.
 *
 * @param {number} pair the first pair value, or -1 for a stream too short
 * @returns {{headerless: true} | {headerless: false, mode: object}}
 * @throws {RangeError} on a marker value no mode claims
 */
export function readMarker(pair) {
  if (pair < MARKER_MIN || pair > MARKER_MAX) return { headerless: true };
  const mode = BY_VALUE.get(pair);
  if (!mode) {
    throw new RangeError(
      pair === ESCAPE
        ? 'this stream uses an extended header, which this version cannot read'
        : `marker ${markerChars(pair)} names a mode this version does not know`,
    );
  }
  return { headerless: false, mode };
}

/** The mode record for a name, for the API and the command line. */
export function modeByName(name) {
  const mode = MODES[name];
  if (!mode) throw new RangeError(`there is no mode called ${JSON.stringify(name)}`);
  return { name, ...mode };
}
