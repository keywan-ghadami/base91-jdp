// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Anything encoded comes back, and comes back through an alphabet JSON does
//! not touch.
//!
//! This is the guarantee the format exists for, so it is the one target that
//! asserts rather than merely surviving: every other property here -- the
//! alphabet, the JSON-escape count, the never-worse-than-block-mode ceiling of
//! specification Section 11.2 -- is checked on the same input while it is in
//! hand, because a fuzzer that has found an interesting input should be made
//! to say everything it can about it.

#![no_main]

use base91z::tables::VALUE_OF;
use libfuzzer_sys::fuzz_target;

// Regression guard. `#[cfg(feature = "zstd")]` in a fuzz target is read
// against the *fuzz* package's features, not against base91z's. It once named
// a feature this package did not have, so the compressed class compiled out of
// this target and was never fuzzed -- and the build succeeded, with one
// warning, which is how it went unnoticed. Fuzzing the container alone is a
// legitimate thing to want; it now has to be asked for by name.
#[cfg(not(any(feature = "zstd", feature = "container-only")))]
compile_error!(
    "the compressed class would compile out of this target unsampled: build \
     with the default features, or pass --features container-only to say that \
     leaving zstd out is deliberate"
);

/// Every character is in the alphabet, and none of them is one a JSON string
/// would have to escape. The second is the point of the format and is checked
/// by counting rather than by trusting the first.
fn assert_json_clean(text: &str) {
    for &b in text.as_bytes() {
        assert!(
            VALUE_OF[b as usize] != 0xFF,
            "character {b:?} is outside the alphabet"
        );
        assert!(
            !matches!(b, b'"' | b'\\' | 0x00..=0x1F),
            "character {b:?} would be escaped inside a JSON string"
        );
    }
}

fn trip(data: &[u8], text: &str, what: &str) {
    assert_json_clean(text);
    match base91z::decode(text) {
        Ok(back) => assert!(
            back == data,
            "{what}: {} bytes did not survive the round trip",
            data.len()
        ),
        Err(e) => panic!("{what}: own output rejected at {}: {e}", e.at),
    }
}

fuzz_target!(|data: &[u8]| {
    let plain = base91z::encode_plain(data);
    trip(data, &plain, "encode_plain");

    // Section 11.2: a candidate class is committed only where it is strictly
    // shorter than block mode, so the block coder is the ceiling and no input
    // can make the typed classes cost more than not having them.
    let block = base91z::bench::block_only(data);
    assert!(
        plain.len() <= block.len(),
        "encode_plain is {} characters against block mode's {}",
        plain.len(),
        block.len()
    );
    trip(data, &block, "block_only");

    #[cfg(feature = "zstd")]
    {
        // The default entry point, which decides per input whether to
        // compress. Infallible by contract -- there is always a valid
        // uncompressed encoding -- so a panic here is a bug in that promise.
        trip(data, &base91z::encode(data), "encode");

        // One negative level and one high one: the decision thresholds differ
        // by level, so a single level exercises a single set of branches.
        for level in [-5i32, 19] {
            if let Ok(text) = base91z::encode_at(data, level) {
                trip(data, &text, "encode_at");
            }
        }
    }
});
