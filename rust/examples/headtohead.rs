// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Base91z against Base85N, each in the configuration it ships with.
//!
//! Both codecs are built from source and run in this process under the same
//! timing loop, so what is compared is two encodings and not two languages.
//!
//! Compression is part of this format (Section 10) and is not part of
//! Base85N's. This benchmark therefore compares each codec as it comes: this
//! one with its compressor, that one without. There is no configuration here
//! that turns this format's compressor off, because a caller does not have
//! one. What the third table adds is the other honest question -- what a
//! Base85N caller would have to build to get a stream this small, which is
//! zstd in front of it, and how the two compare then.
//!
//!     cargo run --release --features base85n --example headtohead -- bench/corpus

use std::time::Instant;

/// One pass over every file, timed as a whole; the best of three. Rates are
/// always over the original bytes, so a column can be read down the table.
fn rate(total: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    total as f64 / 1e6 / best
}

fn pct(a: usize, b: usize) -> String {
    format!("{:+.2} %", 100.0 * (a as f64 / b as f64 - 1.0))
}

/// zstd, then Base85N: what a caller does who wants a Base85N stream small.
/// Given every advantage -- a stock frame over the whole file in one piece,
/// where this format chunks at a mebibyte and pays 0.2 % for it (Section 17.9).
fn pipeline(data: &[u8], level: i32) -> String {
    base85n::encode(&zstd::bulk::compress(data, level).unwrap())
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let dirs = if dirs.is_empty() { vec!["bench/corpus".into()] } else { dirs };
    let mut files: Vec<Vec<u8>> = Vec::new();
    for d in &dirs {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .unwrap_or_else(|e| panic!("{d}: {e} -- run python3 bench/corpus.py"))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        v.sort();
        for p in v {
            files.push(std::fs::read(&p).unwrap());
        }
    }
    let total: usize = files.iter().map(Vec::len).sum();

    // Nothing is reported that did not come back.
    for d in &files {
        let a = base91z::encode_at(d, 1).unwrap();
        assert_eq!(base91z::decode(&a).unwrap(), *d);
        let b = base85n::encode(d);
        assert_eq!(base85n::decode(&b).unwrap(), *d);
    }

    let size = |out: &[String]| out.iter().map(String::len).sum::<usize>();
    let jdp = |level: i32| -> Vec<String> {
        files.iter().map(|d| base91z::encode_at(d, level).unwrap()).collect()
    };

    println!("## {} files, {total} bytes\n", files.len());

    let b85: Vec<String> = files.iter().map(|d| base85n::encode(d)).collect();
    let n85 = size(&b85);
    let e85 = rate(total, || for d in &files { std::hint::black_box(base85n::encode(d).len()); });
    let d85 = rate(total, || for s in &b85 { std::hint::black_box(base85n::decode(s).unwrap().len()); });

    println!("### Each codec as it ships\n");
    println!("| | size | encode | decode |");
    println!("|---|---|---|---|");
    println!("| Base85N 0.5.1 | {:.5} | {e85:.0} MB/s | {d85:.0} MB/s |", n85 as f64 / total as f64);
    for (level, note) in [(1i32, "the recommendation"), (-5, "for encode throughput")] {
        let out = jdp(level);
        let n = size(&out);
        println!(
            "| **Base91z, zstd {level}** ({note}) | **{:.5}** | {:.0} MB/s | {:.0} MB/s |",
            n as f64 / total as f64,
            rate(total, || for d in &files {
                std::hint::black_box(base91z::encode_at(d, level).unwrap().len());
            }),
            rate(total, || for s in &out {
                std::hint::black_box(base91z::decode(s).unwrap().len());
            })
        );
    }

    println!("\n### Every level, and what a Base85N caller would have to build\n");
    println!("| level | Base91z | encode | decode | zstd → Base85N | |");
    println!("|---|---|---|---|---|---|");
    for level in [-5i32, -3, -1, 1, 2, 3, 5, 9] {
        let out = jdp(level);
        let n = size(&out);
        let p: usize = files.iter().map(|d| pipeline(d, level).len()).sum();
        println!(
            "| {level} | **{:.5}** | {:.0} MB/s | {:.0} MB/s | {:.5} | {} |",
            n as f64 / total as f64,
            rate(total, || for d in &files {
                std::hint::black_box(base91z::encode_at(d, level).unwrap().len());
            }),
            rate(total, || for s in &out {
                std::hint::black_box(base91z::decode(s).unwrap().len());
            }),
            p as f64 / total as f64,
            pct(n, p)
        );
    }
    println!(
        "\nNegative levels are the one place the pipeline wins, and Section 10.1\n\
         says why: they limit the entropy coding of literals, so stretches of\n\
         the source survive into the frame where Base85N's passthrough reaches\n\
         them and this format's block mode does not look. From level 1 up there\n\
         is nothing left to find and this format is ahead everywhere."
    );

    let plain: usize = files.iter().map(|d| base91z::encode_plain(d).len()).sum();
    println!(
        "\n### The containers alone, for the record\n\n\
         Neither side compressing -- a build of this crate that cannot link\n\
         zstd. It is not a configuration a caller chooses, and it is not a term\n\
         in the comparison above; it says what the container is worth.\n\n\
         | | size | encode | decode |\n|---|---|---|---|\n\
         | Base85N | {:.5} | {e85:.0} MB/s | {d85:.0} MB/s |\n\
         | Base91z, no compressor | **{:.5}** ({}) | {:.0} MB/s | {:.0} MB/s |",
        n85 as f64 / total as f64,
        plain as f64 / total as f64,
        pct(plain, n85),
        rate(total, || for d in &files { std::hint::black_box(base91z::encode_plain(d).len()); }),
        {
            let out: Vec<String> = files.iter().map(|d| base91z::encode_plain(d)).collect();
            rate(total, || for s in &out { std::hint::black_box(base91z::decode(s).unwrap().len()); })
        }
    );
}
