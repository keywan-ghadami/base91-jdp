//! On long data, compression wins on both axes -- so what is the class
//! machinery still doing there?

use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::Ordering::Relaxed;
use base91_jdp::tables::tuning;

fn load(d: &str) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(d).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    v.sort();
    v.into_iter().map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap())).collect()
}

fn main() {
    let core = load("bench/corpus");
    let short = load("bench/corpus/short");

    // What classes carry, once smart compression is deciding.
    println!("### Class usage on the core corpus, with smart compression on\n");
    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    let mut total = 0usize;
    for (_, d) in &core {
        total += d.len();
        let text = base91_jdp::encode_smart(d, -5).unwrap();
        for (c, n) in base91_jdp::explain(&text).unwrap() {
            *tally.entry(c).or_default() += n;
        }
    }
    let mut v: Vec<_> = tally.into_iter().filter(|(_, n)| *n > 0).collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (c, n) in &v {
        println!("- {c}: {:.2} % of input bytes", 100.0 * *n as f64 / total as f64);
    }
    let carried: usize = v.iter().map(|(_, n)| n).sum();
    println!("- block mode: {:.2} %\n", 100.0 * (total - carried) as f64 / total as f64);

    // What each family is worth, with and without a compressor available.
    println!("### With a compressor, and without\n");
    println!("| classes enabled | core, no compressor | core, smart −5 | short, no compressor |");
    println!("|---|---|---|---|");
    let ratio = |f: &Vec<(String, Vec<u8>)>, comp: Option<i32>| -> f64 {
        let (mut i, mut o) = (0usize, 0usize);
        for (_, d) in f {
            i += d.len();
            o += match comp {
                None => base91_jdp::encode(d).len(),
                Some(l) => base91_jdp::encode_smart(d, l).unwrap().len(),
            };
        }
        o as f64 / i as f64
    };
    for (label, mask) in [
        ("all", tuning::F_RUN | tuning::F_PACKED | tuning::F_PT),
        ("no runs at all", tuning::F_PACKED | tuning::F_PT),
        ("no packed bases", tuning::F_RUN | tuning::F_PT),
        ("no passthrough", tuning::F_RUN | tuning::F_PACKED),
        ("block coder alone", 0),
    ] {
        tuning::FAMILIES.store(mask, Relaxed);
        println!("| {label} | {:.4} | {:.5} | {:.4} |",
            ratio(&core, None), ratio(&core, Some(-5)), ratio(&short, None));
    }
    tuning::reset();
}
