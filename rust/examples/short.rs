// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The short group: field-level payloads under 200 bytes.
//!
//! This is the only benchmark that exercises the packed bases of
//! specification section 9 at all, and the only one where three characters of
//! segment overhead are visible. It reports which class each sample actually
//! landed in, because a ratio alone cannot say whether a class ever fired.
//!
//!     cargo run --release --example short -- bench/corpus/short

use std::collections::BTreeMap;
use std::fs;

fn base64_len(n: usize) -> usize {
    4 * n.div_ceil(3)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("bench/corpus/short".into());
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e} -- run python3 bench/corpus.py --short"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();

    println!("| sample | bytes | base64 | jdp | class | chars |");
    println!("|---|---|---|---|---|---|");

    let mut by_cat: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    let (mut tin, mut t64, mut tj) = (0usize, 0usize, 0usize);

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let data = fs::read(path).unwrap();
        let text = base91z::encode_plain(&data);
        assert_eq!(base91z::decode(&text).unwrap(), data, "{name}");

        // Which classes carried it, largest first.
        let used = base91z::explain(&text).unwrap();
        let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
        for (c, n) in &used {
            *tally.entry(c).or_default() += n;
        }
        let mut order: Vec<_> = tally.into_iter().collect();
        order.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let carried: usize = order.iter().map(|(_, n)| n).sum();
        let mut classes: Vec<String> = order.iter().take(2).map(|(c, _)| c.to_string()).collect();
        if carried < data.len() {
            classes.push("block".into());
        }

        let cat = name.split('-').nth(1).unwrap_or("?").to_string();
        let e = by_cat.entry(cat).or_default();
        e.0 += data.len();
        e.1 += base64_len(data.len());
        e.2 += text.len();
        tin += data.len();
        t64 += base64_len(data.len());
        tj += text.len();

        println!(
            "| {} | {} | {:.3} | **{:.3}** | {} | {} |",
            name.splitn(3, '-').nth(2).unwrap_or(&name),
            data.len(),
            base64_len(data.len()) as f64 / data.len() as f64,
            text.len() as f64 / data.len() as f64,
            if classes.is_empty() { "block".into() } else { classes.join(" + ") },
            text.len(),
        );
    }

    println!("\n### By what the sample is\n");
    println!("| kind | bytes | base64 | jdp | against base64 |");
    println!("|---|---|---|---|---|");
    for (cat, (b, s64, sj)) in &by_cat {
        println!(
            "| {cat} | {b} | {:.4} | **{:.4}** | {:+.1} % |",
            *s64 as f64 / *b as f64,
            *sj as f64 / *b as f64,
            (*sj as f64 / *s64 as f64 - 1.0) * 100.0
        );
    }
    println!(
        "| **all** | {tin} | {:.4} | **{:.4}** | **{:+.1} %** |",
        t64 as f64 / tin as f64,
        tj as f64 / tin as f64,
        (tj as f64 / t64 as f64 - 1.0) * 100.0
    );
}
