// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The decoder against streams that are wrong rather than invalid.
//!
//! `decode_any` gives the decoder arbitrary bytes, and almost all of them stop
//! at the character check in the first few characters -- which is a path worth
//! testing and not the interesting one. Here every input byte is folded onto
//! the alphabet first, so the fuzzer spends its budget past that check, on the
//! part that reads signals, classes, lengths and padding. That is where a
//! decoder gets hurt: a length that overruns, an index into a table that is
//! not there, padding that says one thing and a stream that says another.
//!
//! Nothing is asserted about the *result* -- an arbitrary alphabet string is
//! not a stream anybody encoded, so bytes and an error are equally correct.
//! What is asserted is that one of the two happens, and, for the bounded call,
//! that the ceiling holds.

#![no_main]

use base91z::tables::ALPHABET;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fold rather than reject: every input maps to a stream, so a mutation is
    // never wasted and the corpus stays as small as libFuzzer wants it. 91
    // does not divide 256, so this is not uniform -- it does not need to be,
    // and the pair values that carry signals stay reachable from every byte.
    let text: String = data
        .iter()
        .map(|&b| ALPHABET[usize::from(b) % ALPHABET.len()] as char)
        .collect();

    let _ = base91z::decode(&text);
    let _ = base91z::explain(&text);

    let budget = data.len().saturating_mul(8);
    if let Ok(out) = base91z::decode_bounded(&text, budget) {
        assert!(
            out.len() <= budget,
            "decode_bounded returned {} bytes against a budget of {budget}",
            out.len()
        );
    }
});
