// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What compression costs and what it is worth, per level.
//!
//! The container encodes at gigabytes per second (section 17.13). zstd does
//! not, at any level anyone would use for size. This measures both sides of
//! that so the throughput of the whole is attributed where it belongs.
//!
//!     cargo run --release --example compress -- bench/corpus/countries.json

use std::fs;
use std::time::Instant;

fn rate(bytes: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    bytes as f64 / 1e6 / best
}

fn main() {
    for path in std::env::args().skip(1) {
        let data = fs::read(&path).unwrap();
        println!("\n### {} — {} bytes\n", path, data.len());
        println!("| level | chars/byte | whole encode | zstd alone | container alone | decode |");
        println!("|---|---|---|---|---|---|");

        // The container on its own, for the row every other row is measured
        // against: the same bytes, already compressed, through block mode.
        let frame = zstd::bulk::compress(&data, 3).unwrap();

        // No compressor at all: the question a caller actually asks first.
        let plain = base91z::encode_plain(&data);
        assert_eq!(base91z::decode(&plain).unwrap(), data);
        let plain_enc = rate(data.len(), || {
            std::hint::black_box(base91z::encode_plain(&data).len());
        });
        let plain_dec = rate(data.len(), || {
            std::hint::black_box(base91z::decode(&plain).unwrap().len());
        });
        println!(
            "| none | {:.4} | {:.0} MB/s | -- | {:.0} MB/s | {:.0} MB/s |",
            plain.len() as f64 / data.len() as f64,
            plain_enc,
            3300.0,
            plain_dec
        );

        for level in [-5, -1, 1, 3, 9, 15, 19] {
            let text = base91z::encode_zstd(&data, level).unwrap();
            assert_eq!(base91z::decode(&text).unwrap(), data);
            let whole = rate(data.len(), || {
                std::hint::black_box(base91z::encode_zstd(&data, level).unwrap().len());
            });
            let zonly = rate(data.len(), || {
                std::hint::black_box(zstd::bulk::compress(&data, level).unwrap().len());
            });
            let conly = rate(frame.len(), || {
                std::hint::black_box(base91z::bench::block_only(&frame).len());
            });
            let dec = rate(data.len(), || {
                std::hint::black_box(base91z::decode(&text).unwrap().len());
            });
            println!(
                "| {level} | {:.4} | {:.0} MB/s | {:.0} MB/s | {:.0} MB/s | {:.0} MB/s |",
                text.len() as f64 / data.len() as f64,
                whole,
                zonly,
                conly,
                dec
            );
        }

        // What the whole encoder does when it has to earn the right to
        // compress: both candidates built, the shorter kept (section 11.2).
        let auto = base91z::encode_auto(&data, 3).unwrap();
        let auto_rate = rate(data.len(), || {
            std::hint::black_box(base91z::encode_auto(&data, 3).unwrap().len());
        });
        println!(
            "| auto, level 3 | {:.4} | {:.0} MB/s | | | |",
            auto.len() as f64 / data.len() as f64,
            auto_rate
        );
    }
}
