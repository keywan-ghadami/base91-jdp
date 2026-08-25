//! If compression is mandatory, can the run classes go?
//!
//! Size and throughput, with the runs on and off, on every path a mandatory
//! compressor leaves: short payloads, which no compressor reaches, and long
//! ones, which it always does.

use std::fs;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use base91_jdp::tables::tuning;

fn load(d: &str) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(d).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    v.sort();
    v.into_iter().map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap())).collect()
}

/// Ratio and MB/s over a set of files.
fn run(files: &[(String, Vec<u8>)], level: Option<i32>) -> (f64, f64) {
    let total: usize = files.iter().map(|(_, d)| d.len()).sum();
    let enc = |d: &[u8]| match level {
        None => base91_jdp::encode(d).len(),
        Some(l) => base91_jdp::encode_smart(d, l).unwrap().len(),
    };
    let out: usize = files.iter().map(|(_, d)| enc(d)).sum();
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        for (_, d) in files {
            std::hint::black_box(enc(d));
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    (out as f64 / total as f64, total as f64 / 1e6 / best)
}

fn main() {
    let short = load("bench/corpus/short");
    let core = load("bench/corpus");
    let all = tuning::F_RUN | tuning::F_PACKED | tuning::F_PT;

    println!("| path | classes | chars/byte | MB/s |");
    println!("|---|---|---|---|");
    for (label, files, level) in [
        ("short, no compressor reaches it", &short, None),
        ("short, compression mandatory", &short, Some(-5)),
        ("core, compression mandatory", &core, Some(-5)),
        ("core, compression mandatory, level 3", &core, Some(3)),
        ("core, no compressor available", &core, None),
    ] {
        for (what, mask) in [
            ("with runs", all),
            ("without any run class", tuning::F_PACKED | tuning::F_PT),
        ] {
            tuning::FAMILIES.store(mask, Relaxed);
            let (r, mb) = run(files, level);
            println!("| {label} | {what} | {r:.4} | {mb:.0} |");
        }
        tuning::reset();
        println!("| | | | |");
    }
}
