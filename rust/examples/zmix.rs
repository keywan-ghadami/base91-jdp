//! What the eight chained-gap classes are worth, on the corpus that has the
//! pattern they exist for.

use std::fs;
use std::sync::atomic::Ordering::Relaxed;
use base91_jdp::tables::tuning;

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut v: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    v.sort();
    let files: Vec<(String, Vec<u8>)> = v.into_iter()
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()))
        .collect();

    println!("| file | with ZMIX | without | cost |");
    println!("|---|---|---|---|");
    let (mut tin, mut ta, mut tb) = (0usize, 0usize, 0usize);
    for (name, d) in &files {
        tuning::reset();
        let with = base91_jdp::encode(d).len();
        tuning::FAMILIES.store(tuning::F_RUN | tuning::F_PACKED | tuning::F_PT, Relaxed);
        let without = base91_jdp::encode(d).len();
        tuning::reset();
        tin += d.len(); ta += with; tb += without;
        let cost = (without as f64 / with as f64 - 1.0) * 100.0;
        println!("| {name} | {:.4} | {:.4} | {} |",
            with as f64 / d.len() as f64, without as f64 / d.len() as f64,
            if cost < 0.005 { "--".into() } else { format!("+{cost:.2} %") });
    }
    println!("| **total** | **{:.5}** | {:.5} | **+{:.2} %** |",
        ta as f64 / tin as f64, tb as f64 / tin as f64,
        (tb as f64 / ta as f64 - 1.0) * 100.0);
}
