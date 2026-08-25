// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Does the frame buffer between zstd and the packer cost anything?
//!
//! Three paths for the same work, in one binary so the comparison is fair:
//! zstd's own convenience call, which allocates `compress_bound(len)` per
//! frame and hands the Vec over; the same compression into a buffer this
//! thread keeps; and the packer reading out of each.

use std::time::Instant;

fn rate(bytes: usize, rounds: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..7 {
        let t = Instant::now();
        for _ in 0..rounds {
            f();
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    (bytes * rounds) as f64 / 1e6 / best
}

fn main() {
    let big = std::fs::read("bench/corpus/countries.json").unwrap();
    let small: Vec<Vec<u8>> = (0..200u32)
        .map(|i| format!("{{\"id\":{i},\"name\":\"Ada Lovelace\",\"status\":\"shipped\"}}").into_bytes())
        .collect();
    let small_bytes: usize = small.iter().map(|d| d.len()).sum();

    for level in [-5, 3] {
        println!("\n### level {level}\n");
        println!("| path | one 1.4 MB payload | 200 payloads of ~50 bytes |");
        println!("|---|---|---|");

        // zstd's own call: a fresh compressor and a fresh buffer every time,
        // which is what a caller writes before reading the crate's source.
        let a_big = rate(big.len(), 1, || {
            let mut c = zstd::bulk::Compressor::new(level).unwrap();
            std::hint::black_box(c.compress(&big).unwrap().len());
        });
        let a_small = rate(small_bytes, 1, || {
            let mut c = zstd::bulk::Compressor::new(level).unwrap();
            for d in &small {
                std::hint::black_box(c.compress(d).unwrap().len());
            }
        });
        println!("| fresh compressor, fresh buffer | {a_big:.0} MB/s | {a_small:.0} MB/s |");

        // A kept compressor, still allocating a buffer per frame.
        let mut kept = zstd::bulk::Compressor::new(level).unwrap();
        let b_big = rate(big.len(), 1, || {
            std::hint::black_box(kept.compress(&big).unwrap().len());
        });
        let b_small = rate(small_bytes, 1, || {
            for d in &small {
                std::hint::black_box(kept.compress(d).unwrap().len());
            }
        });
        println!("| kept compressor, fresh buffer | {b_big:.0} MB/s | {b_small:.0} MB/s |");

        // A kept compressor and a kept buffer: what the crate does now.
        let mut buf: Vec<u8> = Vec::new();
        let c_big = rate(big.len(), 1, || {
            buf.clear();
            buf.reserve(zstd::zstd_safe::compress_bound(big.len()));
            std::hint::black_box(kept.compress_to_buffer(&big, &mut buf).unwrap());
        });
        let c_small = rate(small_bytes, 1, || {
            for d in &small {
                buf.clear();
                buf.reserve(zstd::zstd_safe::compress_bound(d.len()));
                std::hint::black_box(kept.compress_to_buffer(d, &mut buf).unwrap());
            }
        });
        println!("| kept compressor, kept buffer | {c_big:.0} MB/s | {c_small:.0} MB/s |");

        // And the whole encoder, which is the third row plus the packer.
        let d_big = rate(big.len(), 1, || {
            std::hint::black_box(base91_jdp::encode_zstd(&big, level).unwrap().len());
        });
        let d_small = rate(small_bytes, 1, || {
            for d in &small {
                std::hint::black_box(base91_jdp::encode_zstd(d, level).unwrap().len());
            }
        });
        println!("| **the encoder: that plus packing** | **{d_big:.0} MB/s** | **{d_small:.0} MB/s** |");
    }
}
