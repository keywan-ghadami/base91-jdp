//! Where the time goes on data no class can carry -- which is what a
//! compressed payload is -- and what the per-window block-mode decision of
//! `detect` is worth. Turning a candidate family off changes the encoding, so
//! those rows are a profile, not alternatives.

use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use base91z::tables::tuning;

fn rate(label: &str, data: &[u8], mut f: impl FnMut()) {
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!("| {label} | {:.0} MB/s |", data.len() as f64 / 1e6 / best);
}

fn main() {
    let mut x: u64 = 0x243F6A8885A308D3;
    let data: Vec<u8> = (0..4_000_000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect();
    println!("entropy of the sample: {:.3} bits/byte\n", base91z::detect::entropy(&data));

    println!("| what | rate |");
    println!("|---|---|");
    rate("whole encoder", &data, || {
        std::hint::black_box(base91z::encode_plain(&data).len());
    });
    tuning::DETECT.store(0, Relaxed);
    rate("without the window decision", &data, || {
        std::hint::black_box(base91z::encode_plain(&data).len());
    });
    for (label, mask) in [
        ("... and without passthrough", tuning::F_RUN | tuning::F_PACKED),
        ("... and without packed bases", tuning::F_RUN | tuning::F_PT),
        ("... and no scan at all", 0),
    ] {
        tuning::FAMILIES.store(mask, Relaxed);
        rate(label, &data, || {
            std::hint::black_box(base91z::encode_plain(&data).len());
        });
    }
    tuning::reset();
    rate("block coder alone", &data, || {
        std::hint::black_box(base91z::bench::block_only(&data).len());
    });
}
