// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Does zstd make the classes unnecessary?
//!
//! Three questions, in order. What zstd does on payloads too short for it to
//! have a window; what each family of classes is worth when it is taken away;
//! and which classes carry anything at all.

use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::Ordering::Relaxed;

use base91z::tables::tuning;

fn load(dir: &str) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    v.sort();
    v.into_iter()
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()))
        .collect()
}

fn ratio(files: &[(String, Vec<u8>)]) -> f64 {
    let (mut i, mut o) = (0usize, 0usize);
    for (_, d) in files {
        i += d.len();
        o += base91z::encode_plain(d).len();
    }
    o as f64 / i as f64
}

fn main() {
    let short = load("bench/corpus/short");
    let core = load("bench/corpus");

    // --- 1. zstd where it has no window -------------------------------
    println!("### zstd on payloads under 200 bytes\n");
    println!("| sample | bytes | jdp | zstd −5 | zstd 3 | zstd 19 |");
    println!("|---|---|---|---|---|---|");
    let (mut tin, mut tj) = (0usize, 0usize);
    let mut tz = [0usize; 3];
    let mut zwins = 0;
    for (name, data) in &short {
        let j = base91z::encode_plain(data).len();
        let z: Vec<usize> = [-5, 3, 19]
            .iter()
            .map(|l| base91z::encode_zstd(data, *l).unwrap().len())
            .collect();
        if z[1] < j {
            zwins += 1;
        }
        tin += data.len();
        tj += j;
        for k in 0..3 {
            tz[k] += z[k];
        }
        if ["03-dec-card-number-16-digits", "09-sha-256-digest", "14-uuid-v4",
            "31-text-first-last-name", "43-text-json-record", "53-binary-zero-run-32-bytes"]
            .contains(&name.as_str())
        {
            println!(
                "| {} | {} | **{:.3}** | {:.3} | {:.3} | {:.3} |",
                name.splitn(3, '-').nth(2).unwrap_or(name),
                data.len(),
                j as f64 / data.len() as f64,
                z[0] as f64 / data.len() as f64,
                z[1] as f64 / data.len() as f64,
                z[2] as f64 / data.len() as f64
            );
        }
    }
    println!(
        "| **all 55** | {tin} | **{:.4}** | {:.4} | {:.4} | {:.4} |",
        tj as f64 / tin as f64,
        tz[0] as f64 / tin as f64,
        tz[1] as f64 / tin as f64,
        tz[2] as f64 / tin as f64
    );
    println!("\nzstd is smaller on {zwins} of the 55.\n");

    // --- 2. taking a family away --------------------------------------
    println!("### What each family is worth\n");
    println!("| classes enabled | short corpus | core corpus |");
    println!("|---|---|---|");
    for (label, mask) in [
        ("all", tuning::F_RUN | tuning::F_PACKED | tuning::F_PT),
        ("no runs", tuning::F_PACKED | tuning::F_PT),
        ("no packed bases", tuning::F_RUN | tuning::F_PT),
        ("no passthrough", tuning::F_RUN | tuning::F_PACKED),
        ("passthrough only", tuning::F_PT),
        ("none: block coder alone", 0),
    ] {
        tuning::FAMILIES.store(mask, Relaxed);
        println!("| {label} | {:.4} | {:.4} |", ratio(&short), ratio(&core));
    }
    tuning::reset();

    // --- 2b. how many packed classes are worth their table ------------
    println!("\n### Thinning the packed bases\n");
    println!("| kept | classes | short corpus | core corpus |");
    println!("|---|---|---|---|");
    // Bit per class from DEC: DEC HEXL HEXU HEXL_D HEXU_D ALPHA_L ALPHA_U
    // B32 B32H CROCK B64 B64U ALNUM
    for (label, names, mask) in [
        ("all thirteen", "", 0x1FFFusize),
        ("no w = 5", "DEC HEXL HEXU B64 B64U ALNUM", 0b1_1100_0000_0111),
        ("hex, digits, base64", "DEC HEXL HEXU B64 B64U", 0b0_1100_0000_0111),
        ("hex and digits", "DEC HEXL HEXU", 0b0_0000_0000_0111),
        ("hex only", "HEXL HEXU", 0b0_0000_0000_0110),
        ("none", "", 0),
        // DEC is a subset of HEXL and both are w = 4; ALPHA_U of B32 at w = 5;
        // ALNUM of B64 at w = 6. A class whose alphabet is contained in
        // another of the same width can never produce a shorter segment.
        ("without the three subsumed", "-DEC -ALPHA_U -ALNUM", 0x1FFF & !(1 | 64 | 4096)),
    ] {
        tuning::PACKED_MASK.store(mask, Relaxed);
        println!("| {label} | {names} | {:.4} | {:.4} |", ratio(&short), ratio(&core));
    }
    tuning::reset();

    // --- 3. which classes carry anything ------------------------------
    println!("\n### Bytes carried, by class\n");
    for (label, files) in [("short", &short), ("core", &core)] {
        let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
        let mut total = 0usize;
        for (_, d) in files {
            total += d.len();
            for (c, n) in base91z::explain(&base91z::encode_plain(d)).unwrap() {
                *tally.entry(c).or_default() += n;
            }
        }
        let mut v: Vec<_> = tally.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let carried: usize = v.iter().map(|(_, n)| n).sum();
        let named: Vec<String> = v
            .iter()
            .map(|(c, n)| format!("{c} {:.1} %", 100.0 * *n as f64 / total as f64))
            .collect();
        println!(
            "**{label}**: {} — block mode {:.1} %\n",
            named.join(", "),
            100.0 * (total - carried) as f64 / total as f64
        );
    }
}
