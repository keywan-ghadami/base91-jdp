//! What the per-window block-mode decision costs and what it buys, per file.
//!
//! A false positive costs size and nothing else -- block mode is the ceiling
//! of specification section 11.2 -- so the question is how much, and on which
//! files.

use std::fs;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use base91z::detect::{self, WINDOW};
use base91z::tables::tuning;

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for d in &dirs {
        let mut v: Vec<_> = fs::read_dir(d)
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        v.sort();
        for p in v {
            files.push((p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()));
        }
    }

    println!("| file | entropy | magic | windows called block | ratio, decision on | off | cost | MB/s on | off |");
    println!("|---|---|---|---|---|---|---|---|---|");
    let (mut tin, mut ton, mut toff) = (0usize, 0usize, 0usize);
    for (name, data) in &files {
        let windows = data.len().div_ceil(WINDOW);
        let called = (0..windows)
            .filter(|w| {
                let from = w * WINDOW;
                let to = (from + WINDOW).min(data.len());
                detect::is_block(&data[from..to], from == 0)
            })
            .count();

        tuning::DETECT.store(1, Relaxed);
        let on = base91z::encode_plain(data);
        let on_rate = rate(data, || {
            std::hint::black_box(base91z::encode_plain(data).len());
        });
        tuning::DETECT.store(0, Relaxed);
        let off = base91z::encode_plain(data);
        let off_rate = rate(data, || {
            std::hint::black_box(base91z::encode_plain(data).len());
        });
        tuning::reset();

        assert_eq!(base91z::decode(&on).unwrap(), *data, "{name}: decision on");
        assert_eq!(base91z::decode(&off).unwrap(), *data, "{name}: decision off");

        tin += data.len();
        ton += on.len();
        toff += off.len();
        let cost = (on.len() as f64 / off.len() as f64 - 1.0) * 100.0;
        println!(
            "| {name} | {:.2} | {} | {}/{} | {:.4} | {:.4} | {} | {:.0} | {:.0} |",
            detect::entropy(data),
            if detect::magic(data) { "yes" } else { "--" },
            called,
            windows,
            on.len() as f64 / data.len() as f64,
            off.len() as f64 / data.len() as f64,
            if cost < 0.005 { "--".to_string() } else { format!("+{cost:.2} %") },
            on_rate,
            off_rate,
        );
    }
    println!(
        "| **total** | | | | **{:.5}** | **{:.5}** | **+{:.3} %** | | |",
        ton as f64 / tin as f64,
        toff as f64 / tin as f64,
        (ton as f64 / toff as f64 - 1.0) * 100.0
    );
}

fn rate(data: &[u8], mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    data.len() as f64 / 1e6 / best
}
