// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The decoder driven by field values a fuzzer chooses.
//!
//! `decode_alphabet` folds random bytes onto the alphabet, which gets past the
//! character check but reaches a *signal* only by accident -- the pair values
//! that carry one are 88 of 8 281, so most of that budget is spent decoding
//! well-formed block data that says nothing interesting. This target writes
//! the fields instead: a signal for a class the fuzzer picks, a length from a
//! table that is nothing but boundaries, a run value, a parameters pair. It
//! reaches in one mutation what the blind target reaches by luck.
//!
//! `tests/adversarial.rs` fixes the cases that are worth naming -- zero
//! length, the class bound and one past it, a maximal run against a small
//! budget. This is the same emitter with the field values left open, so what
//! it finds can be written down there.

#![no_main]

#[path = "../../tests/support/stream.rs"]
mod stream;

use base91z::tables::{CLASS_MAX_DEFINED, MAX_BLOCK_BYTES, MAX_FRAME_BYTES, MAX_SEGMENT_BYTES};
use libfuzzer_sys::fuzz_target;
use stream::{Stream, TIER3_BASE, TIER3_RADIX};

/// Lengths worth writing: every tier boundary, every class bound, and one
/// either side of each. A fuzzer will not find `MAX_SEGMENT_BYTES + 1` by
/// mutating a four-byte integer in any useful time; it will find index 11.
const LENGTHS: &[usize] = &[
    0,
    1,
    89,                     // the last tier-one length
    90,                     // the first tier-two length
    TIER3_BASE - 1,         // the last tier-two length
    TIER3_BASE,             // the first tier-three length
    MAX_SEGMENT_BYTES - 1,
    MAX_SEGMENT_BYTES,      // the bound for runs, packed classes, passthrough
    MAX_SEGMENT_BYTES + 1,
    MAX_BLOCK_BYTES,        // the bound for class 20
    MAX_BLOCK_BYTES + 1,
    MAX_FRAME_BYTES,        // the bound for class 17
    MAX_FRAME_BYTES + 1,
    TIER3_BASE + 8279 + TIER3_RADIX * 8279, // the largest the field can hold
];

/// A cursor over the fuzzer's bytes. Running out is not an error: it ends the
/// stream, which is itself a case worth reaching -- every field can be
/// truncated, and truncation is where a decoder reads what is not there.
struct Script<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Script<'a> {
    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        b
    }

    fn pair_value(&mut self) -> u16 {
        u16::from(self.byte()) | (u16::from(self.byte()) << 8)
    }

    fn done(&self) -> bool {
        self.at >= self.data.len()
    }
}

fuzz_target!(|data: &[u8]| {
    let mut script = Script { data, at: 0 };
    let mut s = Stream::new();

    // Bounded so a single input cannot build a stream large enough to make the
    // run time about allocation rather than about the decoder.
    for _ in 0..64 {
        if script.done() {
            break;
        }
        match script.byte() % 7 {
            // A signal, for a class that may or may not exist. Reaching past
            // CLASS_MAX_DEFINED is the point: an undefined class must be
            // refused, and the escape has its own refusal.
            0 => {
                let class = u16::from(script.byte()) % (CLASS_MAX_DEFINED + 8);
                let hi = u16::from(script.byte() & 1);
                s.signal(class, hi);
            }
            // A length, from the boundary table.
            1 => {
                let l = LENGTHS[usize::from(script.byte()) % LENGTHS.len()];
                s.length(l);
            }
            // A tier-three length with both digits given directly, so a digit
            // above the radix is reachable.
            2 => {
                let p0 = script.pair_value();
                let p1 = script.pair_value();
                s.length_tier3_raw(p0, p1);
            }
            // A bare pair: a run value, a parameters field, a flush field, or
            // block data, depending on where the stream is.
            3 => {
                let v = script.pair_value();
                s.pair(v);
            }
            4 => {
                let v = u16::from(script.byte());
                s.ch(v);
            }
            5 => {
                s.filler(usize::from(script.byte()));
            }
            _ => {
                s.escape();
            }
        }
    }

    let text = s.as_str();

    // Two outcomes are allowed and no third one is: bytes, or an error.
    let _ = base91z::decode(text);
    let _ = base91z::explain(text);

    // And under a ceiling, what comes back is within it. A length field is the
    // one part of a stream that asks the decoder to allocate, so this is the
    // assertion the whole target exists for.
    let budget = usize::from(data.first().copied().unwrap_or(0)) * 512;
    if let Ok(out) = base91z::decode_bounded(text, budget) {
        assert!(
            out.len() <= budget,
            "decode_bounded returned {} bytes against a budget of {budget}",
            out.len()
        );
    }
});
