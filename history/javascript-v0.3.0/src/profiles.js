// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The donor profiles of spec section 4.2.
 *
 * A profile is an ordered ranking of eight alphabet characters, not an
 * alphabet: a segment whose mask has k bits set spends only the first k of
 * them, so only those k stop being usable as literals inside it.
 *
 * Derived by tools/deriveprofiles.js on the 2.37 MB training corpus of
 * tools/traincorpus.py, which shares no file and no upstream project with the
 * benchmark corpus. The search minimises encoded size over the training text;
 * the candidate pool is the rarest punctuation, with letters and digits
 * excluded on principle (see the tool for why, and RESULTS.md for what
 * happens when they are not).
 *
 * A fifth profile was worth 0.013 % on the training corpus and 0.001 % on the
 * hold-out; four is where the curve flattens.
 */
export const PROFILES = ['$~^%#@><', '@&!~%<$^', '%@#<~>$^', '*$?&^|~%'];
