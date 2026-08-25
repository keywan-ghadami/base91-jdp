// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Base91z against the alternatives a caller actually chooses between.
//!
//! Base64 because it is what is deployed. Classic basE91 because this format
//! is derived from it and the difference is worth naming. Base85N because it
//! is the other modern option. And each of those in front of a compressor,
//! because that is what a caller does who needs the output small and has a
//! codec with no compressor of its own.
//!
//!     cargo run --release --features base85n --example against -- bench/corpus

use std::time::Instant;

/// Classic basE91 (Henke, 2005), for the comparison this format is named
/// after. Thirteen or fourteen bits a pair, chosen from the data, over an
/// alphabet that includes `"` and `\` -- which is exactly why it cannot go
/// into a JSON string without escaping, and why this format replaced one
/// character and fixed the symbol at thirteen bits.
const B91: &[u8; 91] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~\"";

fn base91_classic(data: &[u8]) -> String {
    let mut out = Vec::with_capacity(data.len() * 5 / 4 + 4);
    let (mut b, mut n) = (0u32, 0u32);
    for &byte in data {
        b |= (byte as u32) << n;
        n += 8;
        if n > 13 {
            let mut v = b & 8191;
            if v > 88 {
                b >>= 13;
                n -= 13;
            } else {
                v = b & 16383;
                b >>= 14;
                n -= 14;
            }
            out.push(B91[(v % 91) as usize]);
            out.push(B91[(v / 91) as usize]);
        }
    }
    if n > 0 {
        out.push(B91[(b % 91) as usize]);
        if n > 7 || b > 90 {
            out.push(B91[(b / 91) as usize]);
        }
    }
    String::from_utf8(out).unwrap()
}

fn base64_len(n: usize) -> usize {
    4 * n.div_ceil(3)
}

/// What the string costs inside a JSON document, which is where these strings
/// go. `"` and `\` have to be escaped, and classic basE91 uses `"` as its
/// ninety-first character -- so roughly one character in ninety-one of its
/// output doubles. Base64, Base85N and Base91z all have alphabets a JSON
/// string never escapes, and for them this is the identity.
fn in_json(s: &str) -> usize {
    s.len() + s.bytes().filter(|&b| b == b'"' || b == b'\\').count()
}

fn deflate(data: &[u8]) -> Vec<u8> {
    use flate2::write::DeflateEncoder;
    use std::io::Write;
    let mut e = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn rate(total: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    total as f64 / 1e6 / best
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("bench/corpus".into());
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e} -- run python3 bench/corpus.py"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    let files: Vec<Vec<u8>> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
    let total: usize = files.iter().map(Vec::len).sum();

    // Classic basE91 is here to be compared, so check it against a known
    // vector before any of its numbers are believed.
    assert_eq!(base91_classic(b"test"), "fPNKd", "classic basE91 test vector");
    assert_eq!(base91_classic(b""), "");

    let sum = |f: &dyn Fn(&[u8]) -> usize| files.iter().map(|d| f(d)).sum::<usize>();
    // Two sizes: the string itself, and what it costs once it is in the JSON
    // document it was encoded for.
    let row = |name: &str, raw: usize, json: usize, mbs: Option<f64>| {
        let speed = mbs.map_or("—".into(), |r| format!("{r:.0} MB/s"));
        let esc = if json == raw {
            "same".to_string()
        } else {
            format!("**{:.5}**", json as f64 / total as f64)
        };
        println!(
            "| {name} | {:.5} | {esc} | {speed} |",
            raw as f64 / total as f64
        );
    };

    println!("## {} files, {total} bytes\n", files.len());
    println!("### No compressor\n");
    println!("| | chars/byte | in a JSON string | encode |");
    println!("|---|---|---|---|");
    let n64 = sum(&|d| base64_len(d.len()));
    row("Base64", n64, n64, None);
    row(
        "classic basE91",
        sum(&|d| base91_classic(d).len()),
        sum(&|d| in_json(&base91_classic(d))),
        Some(rate(total, || for d in &files {
            std::hint::black_box(base91_classic(d).len());
        })),
    );
    let n85 = sum(&|d| base85n::encode(d).len());
    row("Base85N", n85, n85, Some(rate(total, || for d in &files {
        std::hint::black_box(base85n::encode(d).len());
    })));
    let nz = sum(&|d| base91z::encode_plain(d).len());
    row("**Base91z**, container only", nz, nz, Some(rate(total, || for d in &files {
        std::hint::black_box(base91z::encode_plain(d).len());
    })));

    println!("\n### With a compressor in front\n");
    println!("The three codecs above have none, so this is what a caller has to");
    println!("build. Base91z has one, and it is the same zstd.\n");
    println!("| | chars/byte | in a JSON string | encode |");
    println!("|---|---|---|---|");
    let d64 = sum(&|d| base64_len(deflate(d).len()));
    row("deflate → Base64", d64, d64, None);
    row(
        "deflate → basE91",
        sum(&|d| base91_classic(&deflate(d)).len()),
        sum(&|d| in_json(&base91_classic(&deflate(d)))),
        Some(rate(total, || for d in &files {
            std::hint::black_box(base91_classic(&deflate(d)).len());
        })),
    );
    for level in [1i32, 3, 9] {
        row(
            &format!("zstd {level} → basE91"),
            sum(&|d| base91_classic(&zstd::bulk::compress(d, level).unwrap()).len()),
            sum(&|d| in_json(&base91_classic(&zstd::bulk::compress(d, level).unwrap()))),
            Some(rate(total, || for d in &files {
                std::hint::black_box(
                    base91_classic(&zstd::bulk::compress(d, level).unwrap()).len(),
                );
            })),
        );
        let p = sum(&|d| base85n::encode(&zstd::bulk::compress(d, level).unwrap()).len());
        row(
            &format!("zstd {level} → Base85N"),
            p,
            p,
            Some(rate(total, || for d in &files {
                std::hint::black_box(
                    base85n::encode(&zstd::bulk::compress(d, level).unwrap()).len(),
                );
            })),
        );
        let q = sum(&|d| base91z::encode_at(d, level).unwrap().len());
        row(
            &format!("**Base91z**, zstd {level}"),
            q,
            q,
            Some(rate(total, || for d in &files {
                std::hint::black_box(base91z::encode_at(d, level).unwrap().len());
            })),
        );
    }
}
