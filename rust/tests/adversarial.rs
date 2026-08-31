// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The decoder led into the weeds on purpose.
//!
//! Specification Section 15.4 allows a decoder two outcomes on a stream it did
//! not write: the bytes, or an error. These tests write streams no encoder
//! would -- a length of zero, a run of everything, a class that does not
//! exist, a field cut in half -- and assert which error comes back, by code.
//! Asserting the *code* rather than merely "an error" is what stops a later
//! change from turning a refusal into a different refusal that happens to hide
//! a real one.
//!
//! The round-trip suite covers what an encoder can produce. This covers what
//! an attacker can.

#[path = "support/stream.rs"]
mod stream;

use base91z::tables::{
    CLASS_MAX_DEFINED, CLASS_PACKED_FIRST, CLASS_PACKED_LAST, CLASS_PT, CLASS_PT0, CLASS_RUN,
    CLASS_ZRUN, MAX_SEGMENT_BYTES, PARALLEL_ALIGN,
};
use base91z::{decode, decode_bounded, encode_plain, encode_with_chunk, Code};
use stream::Stream;

/// Every class whose first field is a length, which is every class that can be
/// told to produce nothing or to produce everything.
fn length_classes() -> Vec<u16> {
    let mut v = vec![CLASS_ZRUN, CLASS_RUN, CLASS_PT];
    v.extend(CLASS_PT0..CLASS_PACKED_FIRST);
    v.extend(CLASS_PACKED_FIRST..=CLASS_PACKED_LAST);
    v
}

/// Signal, plus the one field that comes before a length: class 0 carries its
/// mask and donor profile in a parameters pair, and every other class here
/// goes straight to the length. Getting this wrong writes the length into the
/// parameters field, which the decoder correctly refuses for the wrong reason
/// -- so it is written once, here.
fn head(s: &mut Stream, class: u16) -> &mut Stream {
    s.signal(class, 0);
    if class == CLASS_PT {
        s.pair(0);
    }
    s
}

fn refuses(text: &str, code: Code, what: &str) {
    match decode(text) {
        Ok(out) => panic!("{what}: accepted, and produced {} bytes", out.len()),
        Err(e) => assert_eq!(e.code, code, "{what}: refused, with the wrong code ({e})"),
    }
}

// ---------------------------------------------------------------- lengths --

#[test]
fn a_length_of_zero_is_refused_by_every_class_that_has_one() {
    for class in length_classes() {
        let mut s = Stream::new();
        head(&mut s, class).length(0).filler(64);
        refuses(s.as_str(), Code::InvalidLength, &format!("class {class}, length zero"));
    }
}

#[test]
fn a_length_one_above_the_class_bound_is_refused() {
    for class in length_classes() {
        let mut s = Stream::new();
        head(&mut s, class).length(MAX_SEGMENT_BYTES + 1).filler(64);
        refuses(
            s.as_str(),
            Code::InvalidLength,
            &format!("class {class}, length {}", MAX_SEGMENT_BYTES + 1),
        );
    }
}

#[test]
fn a_length_digit_above_the_radix_is_refused() {
    // Tier three is base 8280 and a digit of 8280 is the escape pair, so the
    // decoder checks each digit against 8279. `Stream::pair` clamps at the
    // escape, which is exactly the value that must not pass.
    for (p0, p1) in [(8280u16, 0u16), (0, 8280), (8280, 8280)] {
        let mut s = Stream::new();
        s.signal(CLASS_ZRUN, 0).length_tier3_raw(p0, p1).filler(16);
        refuses(
            s.as_str(),
            Code::InvalidLength,
            &format!("tier-three digits {p0}, {p1}"),
        );
    }
}

#[test]
fn the_largest_length_the_field_can_hold_is_refused_not_reserved() {
    // 8370 + 8279 + 8280 * 8279 -- about 68.5 million, against a class bound
    // of 65 536. The point is that it is refused on the field rather than by
    // running out of memory: a decoder that reserved first would be a
    // one-pair denial of service.
    let biggest = stream::TIER3_BASE + 8279 + stream::TIER3_RADIX * 8279;
    for class in length_classes() {
        let mut s = Stream::new();
        head(&mut s, class).length(biggest).filler(16);
        refuses(
            s.as_str(),
            Code::InvalidLength,
            &format!("class {class}, length {biggest}"),
        );
    }
}

// ------------------------------------------------------------------- runs --

#[test]
fn a_run_of_the_whole_class_bound_is_produced_exactly() {
    // Fill mode at its maximum: one signal and a length field, four
    // characters in all, against 65 536 bytes out. That ratio is the reason
    // the ceiling exists, and this is the case it is measured against.
    let mut s = Stream::new();
    s.signal(CLASS_ZRUN, 0).length(MAX_SEGMENT_BYTES);
    let out = decode(s.as_str()).expect("a maximal zero run is a legal stream");
    assert_eq!(out.len(), MAX_SEGMENT_BYTES);
    assert!(out.iter().all(|&b| b == 0));

    let mut s = Stream::new();
    s.signal(CLASS_RUN, 0).length(MAX_SEGMENT_BYTES).pair(0xAB);
    let out = decode(s.as_str()).expect("a maximal byte run is a legal stream");
    assert_eq!(out.len(), MAX_SEGMENT_BYTES);
    assert!(out.iter().all(|&b| b == 0xAB));
}

#[test]
fn a_maximal_run_stops_at_the_budget_rather_than_at_its_length() {
    // Four characters in, and the caller said 1 000 bytes. The refusal has to
    // come from the ceiling, and it has to come before 65 536 bytes exist.
    for class in [CLASS_ZRUN, CLASS_RUN] {
        let mut s = Stream::new();
        s.signal(class, 0).length(MAX_SEGMENT_BYTES);
        if class == CLASS_RUN {
            s.pair(0xAB);
        }
        match decode_bounded(s.as_str(), 1000) {
            Ok(out) => panic!("class {class}: a 65 536-byte run passed a 1 000-byte budget ({} bytes)", out.len()),
            Err(e) => assert_eq!(e.code, Code::InvalidLength, "class {class}: {e}"),
        }
    }
}

#[test]
fn a_run_of_a_byte_the_class_cannot_carry_is_refused() {
    // Class 19 carries 1..=255; zero is class 18's business and 256 is not a
    // byte. Both are one pair on the wire, so both are reachable.
    for value in [0u16, 256, 8279] {
        let mut s = Stream::new();
        s.signal(CLASS_RUN, 0).length(16).pair(value);
        refuses(
            s.as_str(),
            Code::InvalidRunValue,
            &format!("run of value {value}"),
        );
    }
}

#[test]
fn a_run_that_fits_exactly_at_the_budget_is_allowed() {
    // The boundary in the other direction: a ceiling is a limit, not a margin.
    let mut s = Stream::new();
    s.signal(CLASS_ZRUN, 0).length(1000);
    let out = decode_bounded(s.as_str(), 1000).expect("1 000 bytes into a 1 000-byte budget");
    assert_eq!(out.len(), 1000);
}

// ----------------------------------------------------------------- classes --

#[test]
fn a_class_this_version_does_not_define_is_refused() {
    for class in (CLASS_MAX_DEFINED + 1)..=43 {
        let mut s = Stream::new();
        s.signal(class, 0).filler(32);
        refuses(s.as_str(), Code::UnknownClass, &format!("class {class}"));
    }
}

#[test]
fn the_escape_is_refused_rather_than_skipped() {
    let mut s = Stream::new();
    s.escape().filler(32);
    refuses(s.as_str(), Code::ExtendedClass, "the escape pair");
}

#[test]
fn passthrough_parameters_above_the_field_are_refused() {
    for params in [1024u16, 4095, 8279] {
        let mut s = Stream::new();
        s.signal(CLASS_PT, 0).pair(params).length(16).filler(16);
        refuses(
            s.as_str(),
            Code::InvalidParams,
            &format!("passthrough parameters {params}"),
        );
    }
}

// -------------------------------------------------------------- the flush --

#[test]
fn a_flush_field_wider_than_it_declares_is_refused() {
    // No bits are owed at the start of a stream, so `hi = 1` asks for an
    // eight-bit flush field. A value of 256 does not fit in eight bits and
    // must be refused rather than truncated into the output.
    let mut s = Stream::new();
    s.signal(CLASS_ZRUN, 1).pair(256).length(16);
    refuses(s.as_str(), Code::InvalidFlush, "an eight-bit field holding 256");
}

// --------------------------------------------------------------- truncation --

#[test]
fn every_prefix_of_a_hostile_stream_is_refused_or_decoded_but_never_panics() {
    // Truncation crossed with every field above. A prefix that happens to be
    // a complete stream may decode; what it may not do is panic, hang or
    // return bytes past the ceiling.
    let mut cases = Vec::new();

    for class in length_classes() {
        let mut s = Stream::new();
        head(&mut s, class).length(MAX_SEGMENT_BYTES).filler(40);
        cases.push(s);

        let mut s = Stream::new();
        s.signal(class, 1).pair(255);
        if class == CLASS_PT {
            s.pair(0);
        }
        s.length(64).filler(40);
        cases.push(s);
    }

    let mut s = Stream::new();
    s.signal(CLASS_RUN, 0).length(90).pair(255).filler(8);
    cases.push(s);

    let mut s = Stream::new();
    s.signal(CLASS_ZRUN, 0)
        .length_tier3_raw(8279, 8279)
        .filler(8);
    cases.push(s);

    for case in &cases {
        for prefix in case.prefixes() {
            if let Ok(out) = decode_bounded(prefix, 4096) {
                assert!(
                    out.len() <= 4096,
                    "a prefix produced {} bytes against a 4 096-byte budget",
                    out.len()
                );
            }
        }
    }
}

#[test]
fn a_stream_of_nothing_but_signals_terminates() {
    // Every class in turn, back to back, with no fields between them. Each one
    // consumes the field that follows it, which is the next signal -- so this
    // is the shape that would loop forever if a class ever consumed nothing.
    let mut s = Stream::new();
    for class in 0..=CLASS_MAX_DEFINED {
        s.signal(class, 0);
    }
    let _ = decode_bounded(s.as_str(), 1 << 20);
}

#[test]
fn a_budget_of_zero_yields_nothing_or_an_error() {
    let mut s = Stream::new();
    s.signal(CLASS_ZRUN, 0).length(MAX_SEGMENT_BYTES);
    match decode_bounded(s.as_str(), 0) {
        Ok(out) => assert!(out.is_empty(), "a zero budget produced {} bytes", out.len()),
        Err(e) => assert_eq!(e.code, Code::InvalidLength, "{e}"),
    }
}

// ------------------------------------------------------------ the bulk paths --

#[test]
fn the_bulk_paths_run_on_both_sides() {
    // Every raw pointer in this crate is in two loops: `symbols::block_bulk`,
    // which writes sixteen characters a group through a pointer rather than
    // through eight capacity checks, and `decode::bulk`, which reverses it
    // with one sixteen-byte store per thirteen bytes. Both need a stretch of
    // block-mode data to enter -- sixteen bytes on the way out, eighteen
    // characters on the way in -- and neither is reached by a stream that is
    // mostly signals.
    //
    // This test exists to be run under Miri, where the whole round-trip suite
    // is too slow to be worth a CI job: it is the smallest input that puts a
    // pointer in each of those loops. High-entropy bytes, so no typed class
    // undercuts block mode and diverts the encoder off the path under test.
    let data: Vec<u8> = (0..4096u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();

    let text = encode_plain(&data);
    assert!(text.len() > 18, "too short to reach the decoder's bulk path");
    assert_eq!(decode(&text).expect("own output"), data);

    // And the same data through the seam, which is a third writer into the
    // same buffers.
    assert_eq!(encode_with_chunk(&data, 13 * PARALLEL_ALIGN), text);
}

// ------------------------------------------------------- the chunk contract --

// Found by the `parallel` fuzz target on the empty input, within four minutes
// of its first run: `encode_with_chunk` asserts that a chunk is a whole number
// of symbol groups, and said so nowhere a caller would look. The assertion is
// right -- a chunk that ended mid-group could not be spliced -- so what changed
// is the documentation, and these are what hold both halves of it.

#[test]
#[should_panic(expected = "chunks are whole symbol groups")]
fn a_chunk_of_zero_is_refused() {
    encode_with_chunk(b"anything at all", 0);
}

#[test]
#[should_panic(expected = "chunks are whole symbol groups")]
fn a_chunk_that_is_not_a_whole_group_is_refused() {
    encode_with_chunk(b"anything at all", PARALLEL_ALIGN + 1);
}

#[test]
#[should_panic(expected = "chunks are whole symbol groups")]
fn an_empty_input_does_not_excuse_a_bad_chunk() {
    // The empty case returns early, and the check comes first on purpose: a
    // caller whose chunking is wrong should hear about it on the input that
    // happens to be empty, not on the next one.
    encode_with_chunk(b"", 1);
}

#[test]
fn an_aligned_chunk_is_accepted_at_every_size_the_seam_can_take() {
    // The other half: the contract is satisfiable, and satisfying it gives the
    // serial answer. Sizes around the group and around the data length, which
    // is where the seam falls in an interesting place.
    let data: Vec<u8> = (0..500u32).map(|i| (i * 37 % 256) as u8).collect();
    let serial = encode_plain(&data);
    for groups in [1usize, 2, 3, 7, 38, 39, 40, 100] {
        let chunk = groups * PARALLEL_ALIGN;
        assert_eq!(
            encode_with_chunk(&data, chunk),
            serial,
            "chunk of {groups} groups"
        );
    }
    assert_eq!(encode_with_chunk(b"", PARALLEL_ALIGN), encode_plain(b""));
}
