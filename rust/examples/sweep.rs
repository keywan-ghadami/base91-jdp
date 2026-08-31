// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The three thresholds the 0.4.0 specification says must be re-swept, swept.
//!
//! `MIN_BINARY_RUN` was measured against the 0.3.0 segment structure and does
//! not transfer. The two run-break thresholds are new: without them the prefix
//! scans are greedy and swallow the runs the run classes exist to carry, and
//! with them too low, breaking out of a segment costs more than the run saves.
//!
//!     cargo run --release --example sweep -- bench/corpus

use std::fs;
use std::sync::atomic::Ordering::Relaxed;

use base91z::tables::tuning;

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let corpus: Vec<(String, Vec<u8>)> = dirs
        .iter()
        .flat_map(|d| {
            let mut v: Vec<_> = fs::read_dir(d)
                .unwrap_or_else(|e| panic!("{d}: {e}"))
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            v.sort();
            v.into_iter().map(|p| {
                (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap())
            })
        })
        .collect();
    let total_in: usize = corpus.iter().map(|(_, d)| d.len()).sum();
    let ratio = |corpus: &[(String, Vec<u8>)]| -> f64 {
        let out: usize = corpus.iter().map(|(_, d)| base91z::encode_plain(d).len()).sum();
        out as f64 / total_in as f64
    };

    println!("corpus: {} files, {total_in} bytes\n", corpus.len());

    println!("### MIN_BINARY_RUN\n");
    print!("| value |");
    let bins = [0usize, 1, 2, 3, 4, 5, 6, 8, 12, 16];
    for b in bins {
        print!(" {b} |");
    }
    println!();
    println!("|---|{}", "---|".repeat(bins.len()));
    print!("| ratio |");
    for b in bins {
        tuning::BINARY_RUN.store(b, Relaxed);
        print!(" {:.5} |", ratio(&corpus));
    }
    println!();
    tuning::reset();

    for (label, cell, values) in [
        ("MIN_RUN_IN_SEGMENT (zero runs)", &tuning::ZERO_RUN, [2usize, 4, 6, 8, 10, 13, 16, 20, 26, 40]),
        ("MIN_NONZERO_RUN_IN_SEGMENT", &tuning::NONZERO_RUN, [4usize, 8, 12, 16, 21, 26, 32, 48, 64, 128]),
    ] {
        println!("\n### {label}\n");
        print!("| value |");
        for v in values {
            print!(" {v} |");
        }
        println!();
        println!("|---|{}", "---|".repeat(values.len()));
        print!("| ratio |");
        for v in values {
            cell.store(v, Relaxed);
            print!(" {:.5} |", ratio(&corpus));
        }
        println!();
        tuning::reset();
    }

    // The thresholds do not move independently -- a longer binary run changes
    // which breaks pay -- so the sweeps above only say where to look.
    println!("\n### The grid around them\n");
    println!("| binary run | zero break | nonzero break | ratio |");
    println!("|---|---|---|---|");
    let mut best = (f64::MAX, 0, 0, 0);
    for &b in &[0usize, 1, 2, 4] {
        for &z in &[6usize, 8, 10, 13] {
            for &nz in &[8usize, 10, 12, 14, 21] {
                tuning::BINARY_RUN.store(b, Relaxed);
                tuning::ZERO_RUN.store(z, Relaxed);
                tuning::NONZERO_RUN.store(nz, Relaxed);
                let r = ratio(&corpus);
                if r < best.0 {
                    best = (r, b, z, nz);
                }
            }
        }
    }
    tuning::reset();
    println!("| **{}** | **{}** | **{}** | **{:.5}** |", best.1, best.2, best.3, best.0);
    println!("| 4 (0.3.0) | 13 | 21 | {:.5} |", {
        tuning::reset();
        ratio(&corpus)
    });
}

// Appended: the thresholds do not move independently, so the one-at-a-time
// sweeps above only say where to look. This is the small grid around them.
#[allow(dead_code)]
fn grid() {}
