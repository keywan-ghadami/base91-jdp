//! Compress or not: what the choice costs, and whether one histogram can make
//! it as well as building both candidates can.

use std::fs;
use std::time::Instant;

fn rate(bytes: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 { let t = Instant::now(); f(); best = best.min(t.elapsed().as_secs_f64()); }
    bytes as f64 / 1e6 / best
}

fn main() {
    let level: i32 = std::env::args().nth(1).unwrap().parse().unwrap();
    let dirs: Vec<String> = std::env::args().skip(2).collect();
    let mut files = Vec::new();
    for d in &dirs {
        let mut v: Vec<_> = fs::read_dir(d).unwrap().filter_map(|e| e.ok())
            .map(|e| e.path()).filter(|p| p.is_file()).collect();
        v.sort();
        for p in v { files.push((p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap())); }
    }
    println!("### level {level}\n");
    println!("| file | entropy | none | zstd | auto | smart | agrees | none MB/s | smart MB/s | auto MB/s |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    let (mut tin, mut tn, mut ta, mut ts) = (0usize, 0usize, 0usize, 0usize);
    for (name, data) in &files {
        let none = base91_jdp::encode(data);
        let z = base91_jdp::encode_zstd(data, level).unwrap();
        let auto = base91_jdp::encode_auto(data, level).unwrap();
        let smart = base91_jdp::encode_smart(data, level).unwrap();
        assert_eq!(base91_jdp::decode(&smart).unwrap(), *data);
        let rn = rate(data.len(), || { std::hint::black_box(base91_jdp::encode(data).len()); });
        let rs = rate(data.len(), || { std::hint::black_box(base91_jdp::encode_smart(data, level).unwrap().len()); });
        let ra = rate(data.len(), || { std::hint::black_box(base91_jdp::encode_auto(data, level).unwrap().len()); });
        tin += data.len(); tn += none.len(); ta += auto.len(); ts += smart.len();
        println!("| {name} | {:.2} | {:.4} | {:.4} | {:.4} | **{:.4}** | {} | {:.0} | {:.0} | {:.0} |",
            base91_jdp::detect::entropy(data),
            none.len() as f64 / data.len() as f64,
            z.len() as f64 / data.len() as f64,
            auto.len() as f64 / data.len() as f64,
            smart.len() as f64 / data.len() as f64,
            if smart.len() == auto.len() { "yes" } else { "**no**" },
            rn, rs, ra);
    }
    println!("| **total** | | {:.5} | | {:.5} | **{:.5}** | | | | |",
        tn as f64 / tin as f64, ta as f64 / tin as f64, ts as f64 / tin as f64);
}
