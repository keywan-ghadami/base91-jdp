// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Round trip, the guarantees the specification states as guarantees, and the
//! adversarial decode of section 15.4.

use base91_jdp::tables::{ALPHABET, PARALLEL_ALIGN, VALUE_OF};
use base91_jdp::{decode, encode, encode_with_chunk, Code};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn trip(data: &[u8]) -> String {
    let text = encode(data);
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
    let block = 2 * ((8 * data.len() + 12) / 13);
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
    // The alternation the ZMIX classes exist for: zero runs with fixed gaps.
    for g in 1..=9usize {
        for zeros in [1usize, 2, 3, 17, 90, 8400] {
            let mut data = Vec::new();
            for k in 0..40u8 {
                data.extend(std::iter::repeat(0u8).take(zeros));
                data.extend((0..g).map(|x| 1 + ((k as usize + x) % 250) as u8));
            }
            data.extend(std::iter::repeat(0u8).take(zeros));
            trip(&data);
        }
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
        trip("0123456789".repeat(20)[..len].as_bytes());
        trip("0123456789abcdef".repeat(20)[..len].as_bytes());
    }
}

#[test]
fn mixed_content_at_every_pending_bit_count() {
    // A block-mode stretch of k bytes leaves (8k mod 13) bits pending, so
    // thirteen values of k reach every state a segment can be opened in.
    for k in 0..13usize {
        let mut data: Vec<u8> = (0..k).map(|i| 0x80 | (i as u8)).collect();
        data.extend_from_slice(b"                                        ");
        data.extend(std::iter::repeat(0u8).take(50));
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
        data.extend(std::iter::repeat(0u8).take(round % 37));
        data.extend_from_slice(b"deadbeefcafebabe");
        data.extend((0..(round % 29)).map(|_| rng.random::<u8>()));
    }
    let serial = encode(&data);
    for chunk in [PARALLEL_ALIGN, 2 * PARALLEL_ALIGN, 13 * 40, 13 * 977] {
        assert_eq!(encode_with_chunk(&data, chunk), serial, "chunk {chunk}");
    }
}

#[test]
fn adversarial_decode_is_refused_not_guessed() {
    let bad: [(&str, Code); 6] = [
        // The escape, which this version cannot read.
        ("--A", Code::ExtendedClass),
        // A class above the last defined one: 8192 + 2*31 = 8254.
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

thread_local! {
    static _UNUSED: () = ();
}

lazy_static_like!(PAIR_CLASS_31, pair(8192 + 2 * 31) + "AAAA");
lazy_static_like!(PAIR_ZRUN, pair(8192 + 2 * 21));
lazy_static_like!(ZRUN_ZERO_LEN, pair(8192 + 2 * 21) + "A");

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
        assert_eq!(base91_jdp::bench::div91(v), v / 91, "div91({v})");
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
        let got = base91_jdp::simd::extract_group(&g);
        let whole = u128::from_be_bytes(g);
        for k in 0..8u32 {
            let want = ((whole >> (115 - 13 * k)) & 8191) as u32;
            assert_eq!(got[k as usize], want, "lane {k} of {g:?}");
        }
    }
}
