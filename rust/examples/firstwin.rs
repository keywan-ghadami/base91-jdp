// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The shortest payload on which compressing beats not compressing.
//!
//! Not against block mode -- against the whole plain encoder, runs and packed
//! bases and passthrough included, which is what a caller actually gives up.
//! Byte by byte, so the answer is a length and not a decade.
//!
//!     cargo run --release --features zstd --example firstwin

/// A payload shape: give it a length, it gives you that many bytes.
type Shape<'a> = &'a dyn Fn(usize) -> Vec<u8>;

fn cycle(unit: &[u8], len: usize) -> Vec<u8> {
    unit.iter().cycle().take(len).copied().collect()
}

/// What a compressed segment cost before Section 17.20: a stock zstd frame,
/// magic number and content size and checksum and all, inside the same
/// signal and length field.
fn before(data: &[u8], level: i32) -> usize {
    let n = zstd::bulk::Compressor::new(level).unwrap().compress(data).unwrap().len();
    let length_chars = if n < 90 { 1 } else if n < 8370 { 3 } else { 7 };
    2 + length_chars + 2 * (8 * n).div_ceil(13)
}

/// The first length at which compression wins, and the first from which it
/// wins at every longer length up to `max`. Reported for the encoder as it
/// stands and as it stood before the frame headers came off.
struct Cross {
    first: Option<usize>,
    stable: Option<usize>,
    first_before: Option<usize>,
}

fn crossover(make: Shape, level: i32, max: usize) -> Cross {
    let (mut first, mut first_before) = (None, None);
    let mut stable = None;
    let mut run_from = None;
    for len in 1..=max {
        let data = make(len);
        let plain = base91z::encode_plain(&data).len();
        if before(&data, level) < plain {
            first_before.get_or_insert(len);
        }
        if base91z::encode_zstd(&data, level).unwrap().len() < plain {
            first.get_or_insert(len);
            run_from.get_or_insert(len);
        } else {
            run_from = None;
        }
        stable = run_from;
    }
    Cross { first, stable, first_before }
}

fn main() {
    let json = b"{\"id\":184223,\"name\":\"Ada Lovelace\",\"status\":\"shipped\"},";
    let record: Shape = &|len| {
        let mut v = Vec::new();
        while v.len() < len {
            v.extend_from_slice(b"ORD-184223");
            v.extend(std::iter::repeat_n(0u8, 22));
        }
        v.truncate(len);
        v
    };
    let prose = b"the quick brown fox jumps over the lazy dog. pack my box with five dozen liquor jugs. how vexingly quick daft zebras jump. ";
    let log = b"2026-08-10T07:12:44Z INFO order.service order=184223 user=ada status=shipped duration_ms=41\n";
    let hex = b"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    let noise: Vec<u8> = {
        // A deterministic LCG: bytes with nothing in them to find.
        let mut s = 0x2545_F491_4F6C_DD1Du64;
        (0..4096)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 24) as u8
            })
            .collect()
    };

    let f_json = |n: usize| cycle(json, n);
    let f_prose = |n: usize| cycle(prose, n);
    let f_log = |n: usize| cycle(log, n);
    let f_hex = |n: usize| cycle(hex, n);
    let f_noise = |n: usize| noise[..n].to_vec();
    let cases: Vec<(&str, Shape)> = vec![
        ("repeated JSON record", &f_json),
        ("zero-padded record", record),
        ("prose", &f_prose),
        ("repeated log line", &f_log),
        ("repeated hex digest", &f_hex),
        ("high-entropy binary", &f_noise),
    ];

    let show = |n: Option<usize>| n.map_or("never".to_string(), |n| n.to_string());
    println!("| payload | level | before | now | wins from |");
    println!("|---|---|---|---|---|");
    for (name, make) in &cases {
        for level in [-5i32, 3, 19] {
            let c = crossover(*make, level, 4096);
            println!(
                "| {name} | {level} | {} | **{}** | {} |",
                show(c.first_before),
                show(c.first),
                show(c.stable)
            );
        }
    }
}
