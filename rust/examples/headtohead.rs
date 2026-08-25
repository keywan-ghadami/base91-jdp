// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! base91-jdp against Base85N, both built from source, both in this process.
//!
//! Comparing a codec against another codec's documentation is not a
//! comparison, and comparing a Rust implementation against a Go one measures
//! the language. Base85N ships a Rust implementation with the same shape as
//! this one -- a scalar path, an optional nightly vector path, a parallel
//! encoder -- so both sides here are Rust, compiled by the same compiler at
//! the same optimisation level, timed by the same loop, on the same bytes.
//!
//!     cargo run --release --features base85n --example headtohead -- bench/corpus
//!
//! Add `+nightly --features base85n,simd` to give both sides their vector path.

use std::time::Instant;

/// One encoder run over every file, timed as a whole. Boxed because the
/// closures differ in what they capture.
type Run<'a> = Box<dyn FnMut(&[u8]) -> usize + 'a>;
/// The same for a decoder, which is handed a stream rather than bytes.
type Read<'a> = Box<dyn FnMut(&str) -> usize + 'a>;

fn pct(a: usize, b: usize) -> String {
    format!("{:+.2} %", 100.0 * (a as f64 / b as f64 - 1.0))
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let dirs = if dirs.is_empty() { vec!["bench/corpus".into()] } else { dirs };
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for d in &dirs {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .unwrap_or_else(|e| panic!("{d}: {e} -- run python3 bench/corpus.py"))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        v.sort();
        for p in v {
            files.push((
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            ));
        }
    }
    let total: usize = files.iter().map(|(_, d)| d.len()).sum();

    // Both sides decode what they encoded, on every file, before any number is
    // reported. A ratio from an encoder whose output does not come back is
    // not a measurement.
    for (name, data) in &files {
        let a = base91_jdp::encode(data);
        assert_eq!(base91_jdp::decode(&a).unwrap(), *data, "jdp {name}");
        let b = base85n::encode(data);
        assert_eq!(base85n::decode(&b).unwrap(), *data, "base85n {name}");
    }

    println!("### Size, no compressor on either side\n");
    println!("| file | bytes | Base85N | base91-jdp | |");
    println!("|---|---|---|---|---|");
    let (mut t85, mut t91) = (0usize, 0usize);
    for (name, data) in &files {
        let b = base85n::encode(data).len();
        let a = base91_jdp::encode(data).len();
        t85 += b;
        t91 += a;
        println!(
            "| {name} | {} | {:.4} | **{:.4}** | {} |",
            data.len(),
            b as f64 / data.len() as f64,
            a as f64 / data.len() as f64,
            pct(a, b)
        );
    }
    println!(
        "| **total** | **{total}** | **{:.5}** | **{:.5}** | **{}** |",
        t85 as f64 / total as f64,
        t91 as f64 / total as f64,
        pct(t91, t85)
    );

    // Throughput per file and summed, not on the concatenation. `encode_smart`
    // takes one entropy sample per input, so handing it six megabytes of JPEG
    // and JSON glued together asks it a question no caller asks. Aggregate is
    // total bytes over total time, which is what a caller encoding the corpus
    // file by file would see.
    let sum_rate = |mut f: Run| -> f64 {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            for (_, d) in &files {
                std::hint::black_box(f(d));
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        total as f64 / 1e6 / best
    };
    let enc85: Vec<String> = files.iter().map(|(_, d)| base85n::encode(d)).collect();
    let enc91: Vec<String> = files.iter().map(|(_, d)| base91_jdp::encode(d)).collect();
    let dec_rate = |mut f: Read, src: &[String]| -> f64 {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            for s in src {
                std::hint::black_box(f(s));
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        total as f64 / 1e6 / best
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());

    println!("\n### Throughput, neither side compressing\n");
    println!("This is a build of base91-jdp *without* zstd, which is the only");
    println!("configuration comparable to Base85N: Base85N has no compressor, so");
    println!("giving this one its default feature set would compare two different");
    println!("things. It is not the default build, and the section below is.\n");
    println!("| | encode | encode, {threads} threads | decode |");
    println!("|---|---|---|---|");
    println!(
        "| **Base85N** | **{:.0} MB/s** | **{:.0} MB/s** | **{:.0} MB/s** |",
        sum_rate(Box::new(|d| base85n::encode(d).len())),
        sum_rate(Box::new(move |d| base85n::encode_parallel(d, threads).len())),
        dec_rate(Box::new(|s| base85n::decode(s).unwrap().len()), &enc85)
    );
    println!(
        "| base91-jdp, no compressor | {:.0} MB/s | {:.0} MB/s | {:.0} MB/s |",
        sum_rate(Box::new(|d| base91_jdp::encode(d).len())),
        sum_rate(Box::new(move |d| base91_jdp::encode_parallel(d, threads).len())),
        dec_rate(Box::new(|s| base91_jdp::decode(s).unwrap().len()), &enc91)
    );

    println!("\n### The default build, where the entropy gate decides\n");
    println!("zstd is a default feature of this crate, so this is what a caller");
    println!("gets without asking for anything. Section 11.5 samples the input and");
    println!("either compresses it or skips the scan; Base85N's column is the same");
    println!("number three times because it has one path.\n");
    println!("| | Base85N | base91-jdp | size | encode | decode |");
    println!("|---|---|---|---|---|---|");
    for level in [-5i32, 3, 9] {
        let out: Vec<String> = files
            .iter()
            .map(|(_, d)| base91_jdp::encode_smart(d, level).unwrap())
            .collect();
        for (o, (name, d)) in out.iter().zip(&files) {
            assert_eq!(base91_jdp::decode(o).unwrap(), *d, "smart {name}");
        }
        let c: usize = out.iter().map(String::len).sum();
        println!(
            "| zstd {level} | {:.5} | **{:.5}** | {} | {:.0} MB/s | {:.0} MB/s |",
            t85 as f64 / total as f64,
            c as f64 / total as f64,
            pct(c, t85),
            sum_rate(Box::new(move |d| base91_jdp::encode_smart(d, level).unwrap().len())),
            dec_rate(Box::new(|s| base91_jdp::decode(s).unwrap().len()), &out)
        );
    }
    println!(
        "| Base85N, for reference | {:.5} | | | {:.0} MB/s | {:.0} MB/s |",
        t85 as f64 / total as f64,
        sum_rate(Box::new(|d| base85n::encode(d).len())),
        dec_rate(Box::new(|s| base85n::decode(s).unwrap().len()), &enc85)
    );
}
