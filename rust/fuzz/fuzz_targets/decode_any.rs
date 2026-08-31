// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The decoder against input it did not write.
//!
//! Specification Section 15.4: a decoder reads streams it did not produce, and
//! the only two outcomes allowed are the bytes or an error. Not a panic, not
//! an allocation the caller did not sanction, not a read past the end. This
//! target asserts the second of those explicitly, because the budget is the
//! part a caller depends on and the part a length field can lie about
//! (Section 16).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Arbitrary bytes rather than the alphabet: this is the path a caller
    // reaches when a field holds something that is not an encoded stream at
    // all, and the character check is part of what is under test.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // Unbounded: the default budget is the crate's own ceiling, and a length
    // field that claims more than the stream can hold must be refused rather
    // than reserved for.
    let _ = base91z::decode(text);

    // Bounded: whatever comes back is within what was sanctioned. The budget
    // is taken from the input so the fuzzer can steer it, including to zero.
    let budget = usize::from(data.first().copied().unwrap_or(0)) * 64;
    if let Ok(out) = base91z::decode_bounded(text, budget) {
        assert!(
            out.len() <= budget,
            "decode_bounded returned {} bytes against a budget of {budget}",
            out.len()
        );
    }

    // `explain` walks the same stream for a human. It reads the fields a
    // decode reads, so it can be wrong in the same ways and is fuzzed beside
    // it rather than trusted because its output is only text.
    let _ = base91z::explain(text);
});
