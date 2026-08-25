//! What "just use block mode for the small ones" actually costs.
//!
//! Relative and absolute, because the two say different things at this size,
//! and throughput, because that is half the argument for doing it.

use std::fs;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;
use base91z::tables::tuning;

fn main() {
    let dir = "bench/corpus/short";
    let mut v: Vec<_> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    v.sort();
    let files: Vec<(String, Vec<u8>)> = v.into_iter()
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()))
        .collect();

    let measure = |mask: usize| -> (usize, f64) {
        tuning::FAMILIES.store(mask, Relaxed);
        let out: usize = files.iter().map(|(_, d)| base91z::encode_plain(d).len()).sum();
        let total: usize = files.iter().map(|(_, d)| d.len()).sum();
        let mut best = f64::MAX;
        for _ in 0..7 {
            let t = Instant::now();
            for (_, d) in &files { std::hint::black_box(base91z::encode_plain(d).len()); }
            best = best.min(t.elapsed().as_secs_f64());
        }
        tuning::reset();
        (out, total as f64 / 1e6 / best)
    };

    let all = tuning::F_RUN | tuning::F_PACKED | tuning::F_PT;
    let (full, full_mb) = measure(all);
    let (runs_only, runs_mb) = measure(tuning::F_RUN);
    let (block, block_mb) = measure(0);
    let (no_pt, no_pt_mb) = measure(tuning::F_RUN | tuning::F_PACKED);
    let (no_packed, no_packed_mb) = measure(tuning::F_RUN | tuning::F_PT);
    let total: usize = files.iter().map(|(_, d)| d.len()).sum();

    println!("### The whole short group: {} bytes in {} samples\n", total, files.len());
    println!("| encoder | characters | per byte | more than full | MB/s |");
    println!("|---|---|---|---|---|");
    for (label, out, mb) in [
        ("every class", full, full_mb),
        ("no passthrough: runs + packed + block", no_pt, no_pt_mb),
        ("no packed bases: runs + passthrough + block", no_packed, no_packed_mb),
        ("runs and block mode only", runs_only, runs_mb),
        ("block mode alone", block, block_mb),
    ] {
        println!("| {label} | {out} | {:.4} | {:+} | {:.0} |",
            out as f64 / total as f64, out as isize - full as isize, mb);
    }

    println!("\n### Per sample, in characters\n");
    println!("| sample | bytes | every class | block only | more |");
    println!("|---|---|---|---|---|");
    let mut worst: Vec<(isize, String, usize, usize, usize)> = Vec::new();
    for (name, d) in &files {
        tuning::FAMILIES.store(all, Relaxed);
        let a = base91z::encode_plain(d).len();
        tuning::FAMILIES.store(0, Relaxed);
        let b = base91z::encode_plain(d).len();
        tuning::reset();
        worst.push((b as isize - a as isize, name.clone(), d.len(), a, b));
    }
    worst.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (delta, name, len, a, b) in worst.iter().take(8) {
        println!("| {} | {len} | {a} | {b} | **+{delta}** |",
            name.splitn(3, '-').nth(2).unwrap_or(name));
    }
    let median = worst[worst.len() / 2].0;
    println!("\nMedian over all {} samples: **+{median} characters**.", files.len());

    // What that means where these payloads actually live.
    println!("\n### In a document that carries many of them\n");
    println!("| fields | every class | block only | more |");
    println!("|---|---|---|---|");
    for n in [1usize, 10, 100, 1000] {
        let a = full as f64 / files.len() as f64 * n as f64;
        let b = block as f64 / files.len() as f64 * n as f64;
        println!("| {n} | {:.0} chars | {:.0} chars | +{:.0} ({:.0} %) |",
            a, b, b - a, (b / a - 1.0) * 100.0);
    }
}
