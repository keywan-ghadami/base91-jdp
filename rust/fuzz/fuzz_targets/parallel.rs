// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The parallel encoder is the serial one, character for character.
//!
//! Splitting a stream is only sound if the split cannot be observed in the
//! output, and the chunk boundary is where that would fail: a segment that
//! wants to continue across it, a flush that lands differently, an accumulator
//! carried where it should not be. The test suite checks this at a handful of
//! sizes; a fuzzer can put the boundary anywhere, including at one byte and
//! inside every class the encoder might have chosen.

#![no_main]

use base91z::tables::PARALLEL_ALIGN;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The first two bytes steer the chunk size and the rest is the payload, so
    // the fuzzer controls where the boundary falls rather than only what is on
    // either side of it.
    //
    // Aligned to `PARALLEL_ALIGN`, because a chunk that is not a whole number
    // of symbol groups is a documented panic rather than a bug to find. The
    // first version of this target did not align it, found that panic on the
    // empty input within four minutes, and was right to: the precondition was
    // real and undocumented. `tests/adversarial.rs` now holds it as a case,
    // and this target stays on the side of the contract where a panic would
    // mean something.
    let (head, payload) = data.split_at(data.len().min(2));
    let groups = match head {
        [a, b] => usize::from(u16::from_le_bytes([*a, *b])).max(1),
        _ => 1,
    };
    let chunk = groups * PARALLEL_ALIGN;

    let serial = base91z::encode_plain(payload);
    let split = base91z::encode_with_chunk(payload, chunk);
    assert!(
        serial == split,
        "chunk {chunk} over {} bytes: parallel and serial disagree",
        payload.len()
    );

    // And it still decodes: identical output that neither path can read back
    // would satisfy the assertion above and nothing else.
    match base91z::decode(&split) {
        Ok(back) => assert!(back == payload, "chunk {chunk}: round trip differs"),
        Err(e) => panic!("chunk {chunk}: own output rejected at {}: {e}", e.at),
    }
});
