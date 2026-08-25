// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What the segment and the frame say twice, and what each repetition costs.
//!
//! A zstd frame arriving inside a typed segment repeats most of its own
//! header. The signal already named the class, so the magic number says it
//! again. The length field already gave the size, so the frame header's
//! content-size field and the block header's size field say it again. The
//! format makes no integrity claim, so the checksum is answering a question
//! nobody asked. This walks down the ladder and prices each rung, in frame
//! bytes and then in characters, because a byte saved is 1.2308 characters
//! saved.
//!
//!     cargo run --release --features zstd --example merge

use zstd::zstd_safe::{CParameter, FrameFormat};

fn frame(data: &[u8], level: i32, params: &[CParameter]) -> Vec<u8> {
    let mut c = zstd::bulk::Compressor::new(level).unwrap();
    for p in params {
        c.set_parameter(*p).unwrap();
    }
    c.compress(data).unwrap()
}

/// The lean frame with its five implied header bytes taken off, where the
/// shape allows it. Mirrors `compress::strip`.
fn stripped(f: &[u8]) -> usize {
    let h = u32::from(f[2]) | u32::from(f[3]) << 8 | u32::from(f[4]) << 16;
    let single = f[0] == 0 && h & 1 == 1 && (h >> 1) & 3 == 2 && (h >> 3) as usize == f.len() - 5;
    if single { f.len() - 5 } else { f.len() }
}

/// Characters a payload of `n` bytes occupies, packed at eight bits in.
fn chars(n: usize) -> usize {
    2 * (8 * n).div_ceil(13)
}
fn length_chars(n: usize) -> usize {
    if n < 90 {
        1
    } else if n < 8370 {
        3
    } else {
        7
    }
}
/// Signal, length field, payload.
fn segment(n: usize) -> usize {
    2 + length_chars(n) + chars(n)
}

fn main() {
    let mut short: Vec<Vec<u8>> = std::fs::read_dir("bench/corpus/short")
        .expect("run python3 bench/corpus.py --short")
        .filter_map(|e| e.ok())
        .map(|e| std::fs::read(e.path()).unwrap())
        .collect();
    short.sort();
    let json = std::fs::read("bench/corpus/countries.json").unwrap();
    let css = std::fs::read("bench/corpus/bootstrap.css").unwrap();

    let stock: &[CParameter] = &[];
    let lean: &[CParameter] = &[
        CParameter::Format(FrameFormat::Magicless),
        CParameter::ContentSizeFlag(false),
        CParameter::ChecksumFlag(false),
        CParameter::DictIdFlag(false),
    ];

    for level in [-5, 3] {
        println!("\n### level {level}\n");
        println!("| payload | input | stock frame | lean frame | stripped block |");
        println!("|---|---|---|---|---|");
        let mut rows: Vec<(String, usize, usize, usize, usize)> = Vec::new();
        rows.push((
            format!("{} short samples", short.len()),
            short.iter().map(Vec::len).sum(),
            short.iter().map(|d| frame(d, level, stock).len()).sum(),
            short.iter().map(|d| frame(d, level, lean).len()).sum(),
            short.iter().map(|d| stripped(&frame(d, level, lean))).sum(),
        ));
        for (name, d) in [("bootstrap.css", &css), ("countries.json", &json)] {
            let f = frame(d, level, lean);
            rows.push((
                name.into(),
                d.len(),
                frame(d, level, stock).len(),
                f.len(),
                stripped(&f),
            ));
        }
        for (name, inp, a, b, c) in &rows {
            println!("| {name} | {inp} B | {a} B | {b} B | {c} B |");
        }

        println!("\nIn characters, counting the segment around each payload:\n");
        println!("| payload | stock | lean | stripped | saved |");
        println!("|---|---|---|---|---|");
        for (name, _, a, b, c) in &rows {
            // Several short samples means several segments, so the count is
            // per payload and summed by the caller; here each row is one.
            let (x, y, z) = (segment(*a), segment(*b), segment(*c));
            println!(
                "| {name} | {x} | {y} | {z} | **{:.2} %** |",
                (x - z) as f64 * 100.0 / x as f64
            );
        }
    }

    // Per-segment is the number that matters for the short group, where each
    // sample is its own segment rather than one lump.
    println!("\n### The short group, segment by segment, level 3\n");
    let (mut a, mut b, mut c) = (0usize, 0usize, 0usize);
    for d in &short {
        let f = frame(d, 3, lean);
        a += segment(frame(d, 3, stock).len());
        b += segment(f.len());
        c += segment(stripped(&f));
    }
    println!("- stock frames: {a} characters");
    println!("- lean frames: {b} characters");
    println!("- stripped blocks: {c} characters");
    println!(
        "- saved: **{:.2} %** of what the compressed form used to cost",
        (a - c) as f64 * 100.0 / a as f64
    );
}
