//! Compression over the whole corpus: what it wins, and where the decision to
//! use it costs more than the compression does.

use std::fs;
use std::time::Instant;

fn rate(bytes: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..2 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    bytes as f64 / 1e6 / best
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();
    for d in &dirs {
        let mut v: Vec<_> = fs::read_dir(d).unwrap().filter_map(|e| e.ok())
            .map(|e| e.path()).filter(|p| p.is_file()).collect();
        v.sort();
        for p in v {
            files.push((p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()));
        }
    }
    println!("| file | plain | zstd 3 | kept | encode, zstd | encode, auto |");
    println!("|---|---|---|---|---|---|");
    let (mut tin, mut tp, mut tz, mut ta) = (0usize, 0usize, 0usize, 0usize);
    for (name, data) in &files {
        let plain = base91_jdp::encode(data);
        let z = base91_jdp::encode_zstd(data, 3).unwrap();
        let auto = base91_jdp::encode_auto(data, 3).unwrap();
        assert_eq!(base91_jdp::decode(&auto).unwrap(), *data);
        let zr = rate(data.len(), || { std::hint::black_box(base91_jdp::encode_zstd(data, 3).unwrap().len()); });
        let ar = rate(data.len(), || { std::hint::black_box(base91_jdp::encode_auto(data, 3).unwrap().len()); });
        tin += data.len(); tp += plain.len(); tz += z.len(); ta += auto.len();
        println!("| {name} | {:.4} | {:.4} | {} | {:.0} MB/s | {:.0} MB/s |",
            plain.len() as f64 / data.len() as f64,
            z.len() as f64 / data.len() as f64,
            if z.len() < plain.len() { "zstd" } else { "**plain**" },
            zr, ar);
    }
    println!("| **total** | **{:.5}** | **{:.5}** | **{:.5}** | | |",
        tp as f64 / tin as f64, tz as f64 / tin as f64, ta as f64 / tin as f64);
}
