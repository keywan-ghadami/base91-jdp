// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Round trip, the guarantees the specification states as guarantees, and the
//! adversarial decode of section 15.4.

use base91z::tables::{ALPHABET, PARALLEL_ALIGN, VALUE_OF};
use base91z::{decode, encode_plain, encode_with_chunk, Code};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn trip(data: &[u8]) -> String {
    let text = encode_plain(data);
    for &b in text.as_bytes() {
        assert!(VALUE_OF[b as usize] != 0xFF, "output outside the alphabet: {:?}", b as char);
    }
    // Nothing JSON has to escape: a property of the alphabet, asserted rather
    // than assumed (specification section 2.2).
    let quoted = serde_len(&text);
    assert_eq!(quoted, text.len(), "output would be escaped inside a JSON string");
    let back = decode(&text).unwrap_or_else(|e| panic!("decode failed: {e} for {} bytes", data.len()));
    assert_eq!(back, data, "round trip differs for {} bytes", data.len());
    text
}

/// Length of `s` once it sits inside a JSON string, without pulling in a JSON
/// crate: only `"` and `\` and control characters would grow it.
fn serde_len(s: &str) -> usize {
    s.bytes()
        .map(|b| match b {
            b'"' | b'\\' => 2,
            0x00..=0x1F => 6,
            _ => 1,
        })
        .sum()
}

/// The block coder is the ceiling: no candidate is committed unless it is
/// strictly shorter (specification section 11.2).
fn assert_never_worse_than_block(data: &[u8], text: &str) {
    let block = 2 * (8 * data.len()).div_ceil(13);
    assert!(
        text.len() <= block + 2,
        "{} bytes encoded to {} characters, block mode would be {}",
        data.len(),
        text.len(),
        block
    );
}

#[test]
fn every_short_length_of_random_binary() {
    let mut rng = StdRng::seed_from_u64(0x5EED);
    for len in 0..=300usize {
        let data: Vec<u8> = (0..len).map(|_| rng.random()).collect();
        let text = trip(&data);
        assert_never_worse_than_block(&data, &text);
    }
    for &len in &[1023usize, 1024, 1025, 65_535, 65_536, 65_537] {
        let data: Vec<u8> = (0..len).map(|_| rng.random()).collect();
        let text = trip(&data);
        assert_never_worse_than_block(&data, &text);
    }
}

#[test]
fn text_and_the_r_set() {
    trip(b"");
    trip(b"hello, world!");
    trip(b"{\"user\":\"ada\",\"id\":42,\"role\":\"admin\"}");
    trip("Gr\u{fc}\u{df}e aus M\u{fc}nchen".as_bytes());
    // Every subset of the R-Set, so every mask is exercised.
    let members: [&[u8]; 8] = [b" ", b"\"", b"\n", b"\\", b"\r", b"'", b"\t", b"\0"];
    for mask in 0u32..256 {
        let mut s = Vec::from(&b"the quick brown fox jumps over the lazy dog"[..]);
        for (j, m) in members.iter().enumerate() {
            if mask & (1 << j) != 0 {
                s.extend_from_slice(m);
                s.extend_from_slice(b"and back again, the same way it came, twice");
            }
        }
        trip(&s);
    }
}

#[test]
fn hyphens_are_ordinary_literals() {
    // 0.3.0 could not carry these inside a segment; 0.4.0 delimits by length.
    for body in ["-", "--", "---", "----------", "--bs-blue: #0d6efd; --bs-indigo: #6610f2;"] {
        for pad in ["", "some ordinary text before it, long enough to be a segment "] {
            let mut s = String::from(pad);
            s.push_str(body);
            s.push_str(" and some ordinary text after it, long enough to matter");
            trip(s.as_bytes());
        }
    }
}

#[test]
fn runs_and_chains() {
    for len in 0..200usize {
        trip(&vec![0u8; len]);
        trip(&vec![0xFFu8; len]);
        trip(&vec![b'a'; len]);
    }
    for &len in &[8369usize, 8370, 65_536, 65_537, 200_000] {
        trip(&vec![0u8; len]);
    }
}

#[test]
fn packed_bases() {
    let cases: [&str; 8] = [
        "0123456789012345678901234567890123456789",
        "deadbeefcafebabe0123456789abcdef",
        "DEADBEEFCAFEBABE0123456789ABCDEF",
        "550e8400-e29b-41d4-a716-446655440000",
        "abcdefghijklmnopqrstuvwxyzabcdefghij",
        "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP",
        "SGVsbG8sIHdvcmxkIQ+/SGVsbG8sIHdvcmxkIQ",
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
    ];
    for c in cases {
        for repeat in [1usize, 2, 40] {
            trip(c.repeat(repeat).as_bytes());
        }
        // Adjacent to material that is not in the class, at both ends.
        trip(format!("\u{1}\u{2}{c}\u{3}\u{4}").as_bytes());
    }
    // Every length where L*w does and does not land on a symbol boundary.
    for len in 1..80usize {
        trip(&"0123456789".repeat(20).as_bytes()[..len]);
        trip(&"0123456789abcdef".repeat(20).as_bytes()[..len]);
    }
}

#[test]
fn mixed_content_at_every_pending_bit_count() {
    // A block-mode stretch of k bytes leaves (8k mod 13) bits pending, so
    // thirteen values of k reach every state a segment can be opened in.
    for k in 0..13usize {
        let mut data: Vec<u8> = (0..k).map(|i| 0x80 | (i as u8)).collect();
        data.extend_from_slice(b"                                        ");
        data.extend(std::iter::repeat_n(0u8, 50));
        data.extend_from_slice(b"0123456789012345678901234567890123456789");
        data.extend((0..k).map(|i| 0x90 | (i as u8)));
        trip(&data);
    }
}

#[test]
fn parallel_is_identical_to_serial() {
    let mut rng = StdRng::seed_from_u64(7);
    let mut data = Vec::new();
    // Something with all of it: text, runs, hex, and high-entropy stretches.
    for round in 0..400 {
        data.extend_from_slice(b"the quick brown fox jumps over the lazy dog, ");
        data.extend(std::iter::repeat_n(0u8, round % 37));
        data.extend_from_slice(b"deadbeefcafebabe");
        data.extend((0..(round % 29)).map(|_| rng.random::<u8>()));
    }
    let serial = encode_plain(&data);
    for chunk in [PARALLEL_ALIGN, 2 * PARALLEL_ALIGN, 13 * 40, 13 * 977] {
        assert_eq!(encode_with_chunk(&data, chunk), serial, "chunk {chunk}");
    }
}

#[test]
fn adversarial_decode_is_refused_not_guessed() {
    let bad: [(&str, Code); 6] = [
        // The escape, which this version cannot read.
        ("--A", Code::ExtendedClass),
        // A class above the last defined one.
        (&PAIR_CLASS_31, Code::UnknownClass),
        // A signal that ends the stream before its fields arrive.
        (&PAIR_ZRUN, Code::UnexpectedEos),
        // A character outside the alphabet.
        ("AB\u{7f}CD", Code::InvalidCharacter),
        // A lone character owing nothing.
        ("A", Code::InvalidFinalBlock),
        // A zero length on a run.
        (&ZRUN_ZERO_LEN, Code::InvalidLength),
    ];
    for (text, code) in bad {
        match decode(text) {
            Ok(v) => panic!("{text:?} decoded to {} bytes, expected {code:?}", v.len()),
            Err(e) => assert_eq!(e.code, code, "for {text:?}"),
        }
    }
}

fn pair(v: u16) -> String {
    let mut s = String::new();
    s.push(ALPHABET[(v % 91) as usize] as char);
    s.push(ALPHABET[(v / 91) as usize] as char);
    s
}

lazy_static_like!(PAIR_CLASS_31, pair(8192 + 2 * 31) + "AAAA");
lazy_static_like!(PAIR_ZRUN, pair(8192 + 2 * 18));
lazy_static_like!(ZRUN_ZERO_LEN, pair(8192 + 2 * 18) + "A");

#[macro_export]
macro_rules! lazy_static_like {
    ($name:ident, $e:expr) => {
        #[allow(non_upper_case_globals)]
        static $name: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| $e);
    };
}

#[test]
fn the_reciprocal_divides_exactly() {
    // The bulk block path replaces a division by 91 with a multiply and a
    // shift. Every value a pair can hold is checked against the real division,
    // because a derivation that is nearly right here is a codec that is nearly
    // right everywhere.
    for v in 0u32..=8280 {
        assert_eq!(base91z::bench::div91(v), v / 91, "div91({v})");
    }
}

#[cfg(feature = "simd")]
#[test]
fn simd_extract_matches_scalar() {
    // The vector path lays out thirteen bytes as eight thirteen-bit fields.
    // A layout that is nearly right produces a codec that is nearly right, so
    // every lane is checked against the u128 arithmetic on random groups.
    let mut rng = StdRng::seed_from_u64(0xB10C);
    for _ in 0..20_000 {
        let g: [u8; 16] = rng.random();
        let got = base91z::simd::extract_group(&g);
        let whole = u128::from_be_bytes(g);
        for k in 0..8u32 {
            let want = ((whole >> (115 - 13 * k)) & 8191) as u32;
            assert_eq!(got[k as usize], want, "lane {k} of {g:?}");
        }
    }
}

/// The default entry point compresses where compression pays and does not
/// where it does not, and both come back. The point of the rename: a caller
/// who writes `encode` gets the format, not a subset of it.
#[test]
#[cfg(feature = "zstd")]
fn the_default_encode_uses_the_whole_format() {
    use base91z::encode;

    // A field: too short for a window, carried by the classes.
    let field = b"{\"user\":\"ada\",\"id\":42,\"role\":\"admin\"}";
    let t = encode(field);
    assert_eq!(decode(&t).unwrap(), field);
    assert_eq!(t, encode_plain(field), "a field should not reach for a frame");

    // A document: compressed, and much smaller than the container alone.
    let doc: Vec<u8> = std::iter::repeat_n(&field[..], 400).flatten().copied().collect();
    let t = encode(&doc);
    assert_eq!(decode(&t).unwrap(), doc);
    assert!(
        t.len() * 4 < encode_plain(&doc).len(),
        "{} against {}",
        t.len(),
        encode_plain(&doc).len()
    );

    // Never worse than the container, whatever the input.
    for n in [0usize, 1, 13, 200, 5000] {
        let d: Vec<u8> = (0..n).map(|i| (i * 7919 % 251) as u8).collect();
        assert!(encode(&d).len() <= encode_plain(&d).len(), "{n} bytes");
        assert_eq!(decode(&encode(&d)).unwrap(), d);
    }
}

#[cfg(feature = "zstd")]
mod compressed {
    use base91z::{decode, decode_bounded, encode_auto, encode_zstd, Code};
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    fn trip(data: &[u8], level: i32) {
        let text = encode_zstd(data, level).unwrap();
        assert_eq!(decode(&text).unwrap(), data, "{} bytes at level {level}", data.len());
        let auto = encode_auto(data, level).unwrap();
        assert_eq!(decode(&auto).unwrap(), data, "auto, {} bytes", data.len());
        // Section 11.2: the compressed candidate is taken only when it wins.
        assert!(auto.len() <= text.len());
        assert!(auto.len() <= base91z::encode_plain(data).len());
    }

    #[test]
    fn frames_round_trip_at_every_level() {
        let mut rng = StdRng::seed_from_u64(11);
        let text: Vec<u8> = std::iter::repeat_n(b"the quick brown fox jumps over the lazy dog. ", 500)
            .flatten()
            .copied()
            .collect();
        let noise: Vec<u8> = (0..40_000).map(|_| rng.random()).collect();
        for level in [-5, 1, 3, 9, 19] {
            for data in [&text, &noise] {
                trip(data, level);
            }
            for len in [0usize, 1, 13, 100, 5000] {
                trip(&text[..len.min(text.len())], level);
            }
        }
    }

    /// Section 10.2: a payload that fits in one block loses its frame header
    /// and its block header, and a payload that does not keeps them. Both
    /// forms have to come back, and the encoder has to choose between them by
    /// the payload rather than by luck -- so this pins which class each size
    /// lands in as well as that it round trips.
    #[test]
    fn a_single_block_payload_is_stripped_and_a_larger_one_is_not() {
        let unit = b"the quick brown fox jumps over the lazy dog. ";
        let classes = |text: &str| -> Vec<String> {
            base91z::explain(text)
                .unwrap()
                .into_iter()
                .map(|(c, _)| c.to_string())
                .collect()
        };
        for len in [200usize, 1000, 1 << 16, (1 << 17) - 1, 1 << 17, (1 << 17) + 1, 1 << 20] {
            let data: Vec<u8> = unit.iter().cycle().take(len).copied().collect();
            let text = encode_zstd(&data, 3).unwrap();
            assert_eq!(decode(&text).unwrap(), data, "{len} bytes");
            // Up to a block's worth of input, zstd emits one compressed block
            // and the encoder strips it. Past that it cannot, and the frame
            // stays whole.
            let want = if len <= 1 << 17 { "ZBLK" } else { "ZSTD" };
            assert!(classes(&text).iter().all(|c| c == want), "{len} bytes: {:?}", classes(&text));
        }

        // The other reason the frame stays: a payload zstd declines to
        // compress comes back as a raw block, which the strip does not apply
        // to. Fifty bytes of prose is such a payload.
        let short = &unit[..37];
        let text = encode_zstd(short, 3).unwrap();
        assert_eq!(decode(&text).unwrap(), short);
        assert_eq!(classes(&text), ["ZSTD"], "an uncompressed block was stripped");
    }

    /// A stripped payload is a bare block, so the decoder writes the five
    /// bytes of header itself. Corrupt the block and it must refuse rather
    /// than hand back whatever the reconstruction happened to produce.
    #[test]
    fn a_damaged_block_is_refused() {
        let data: Vec<u8> = b"the quick brown fox jumps over the lazy dog. "
            .iter()
            .cycle()
            .take(4000)
            .copied()
            .collect();
        let text = encode_zstd(&data, 3).unwrap();
        let good: Vec<char> = text.chars().collect();
        let mut refused = 0;
        // Every character of the payload in turn, moved one place in the
        // alphabet. The length field and the signal are the first characters
        // and are not the subject here, so start past them.
        for i in 6..good.len() {
            let mut bad: Vec<char> = good.clone();
            bad[i] = if bad[i] == 'A' { 'B' } else { 'A' };
            let s: String = bad.into_iter().collect();
            match decode(&s) {
                Ok(v) => assert_ne!(v, data, "a changed character decoded to the same bytes"),
                Err(_) => refused += 1,
            }
        }
        assert!(refused > 0, "no single-character change was refused");
    }

    #[test]
    fn a_frame_is_carried_across_segments() {
        // Longer than one frame's payload, so several ZSTD segments follow one
        // another and the decoder has to resume block mode between them.
        let big: Vec<u8> = (0..3_000_000u32).map(|i| (i / 977) as u8).collect();
        let text = encode_zstd(&big, 3).unwrap();
        assert_eq!(decode(&text).unwrap(), big);
    }

    /// The decompression context is kept per thread rather than built per
    /// frame, so a frame that fails part way through must not leave anything
    /// behind for the next one. Alternate a refused frame with a good one and
    /// insist the good one is unaffected -- including the case that abandons a
    /// stream mid-way rather than at its first byte, which is the ceiling.
    #[test]
    fn a_refused_frame_does_not_poison_the_next_decode() {
        let data: Vec<u8> = b"the quick brown fox jumps over the lazy dog. "
            .iter()
            .cycle()
            .take(9000)
            .copied()
            .collect();
        let good = encode_zstd(&data, 3).unwrap();
        let bomb = encode_zstd(&vec![0u8; 1 << 20], 19).unwrap();

        let mut truncated = good.clone();
        truncated.truncate(good.len() - 8);

        for round in 0..8 {
            assert_eq!(decode(&good).unwrap(), data, "round {round}, before");
            // Abandoned at the ceiling, with the stream part way through.
            assert!(decode_bounded(&bomb, 4096).is_err(), "round {round}, bomb");
            assert_eq!(decode(&good).unwrap(), data, "round {round}, after the bomb");
            // Abandoned because the frame ran out of input.
            assert!(decode(&truncated).is_err(), "round {round}, truncated");
            assert_eq!(decode(&good).unwrap(), data, "round {round}, after the truncation");
        }
    }

    #[test]
    fn expansion_is_bounded_by_the_caller() {
        // A megabyte of zeros is a few hundred characters. A decoder that
        // allocates on the length field alone has already lost.
        let bomb = encode_zstd(&vec![0u8; 1 << 20], 19).unwrap();
        assert!(bomb.len() < 400, "{} characters", bomb.len());
        assert_eq!(decode(&bomb).unwrap().len(), 1 << 20);
        // The segment declares what it expands to, so the ceiling is reached
        // by reading a field rather than by decompressing a megabyte and
        // throwing it away. The code pins that: `MalformedFrame` here would
        // mean the frame was expanded first.
        match decode_bounded(&bomb, 4096) {
            Ok(v) => panic!("decoded {} bytes past the ceiling", v.len()),
            Err(e) => assert_eq!(e.code, Code::InvalidLength, "{e}"),
        }
    }

    /// The declared length is a claim, and a decoder that allocates against a
    /// claim has to check it. Every single-character change to the field is
    /// refused -- not by the decompressor, which is perfectly happy, but by
    /// the comparison against what actually came out.
    #[test]
    fn a_wrong_declared_length_is_refused() {
        let data: Vec<u8> = b"the quick brown fox jumps over the lazy dog. "
            .iter()
            .cycle()
            .take(4000)
            .copied()
            .collect();
        let text = encode_zstd(&data, 3).unwrap();
        assert_eq!(decode(&text).unwrap(), data);

        // Segment layout: two characters of signal, the payload length -- one
        // character, the frame being well under ninety bytes -- and then the
        // plain length, which at 4 000 is the three-character tier: a marker
        // and a pair. Asserted rather than assumed, so that a layout change
        // fails here loudly instead of quietly testing nothing.
        let chars: Vec<char> = text.chars().collect();
        assert_eq!(chars[3], '-', "not the three-character length tier: {text}");

        let mut caught = 0;
        for i in [4usize, 5] {
            for c in ['A', 'B', 'z', '0'] {
                if chars[i] == c {
                    continue;
                }
                let mut bad = chars.clone();
                bad[i] = c;
                let s: String = bad.into_iter().collect();
                let e = decode(&s).expect_err("a wrong declared length decoded");
                if e.what == "not the declared length" {
                    caught += 1;
                }
            }
        }
        assert!(caught > 0, "no mutation reached the length check");
    }
}
