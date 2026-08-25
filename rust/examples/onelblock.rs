//! What independent 128 KiB frames cost against 1 MiB frames: the price of a
//! format in which every compressed segment is exactly one zstd block.
//!
//!     cargo run --release --features zstd --example onelblock -- bench/corpus

use std::fs;

fn lean(c: &mut zstd::bulk::Compressor<'static>) {
    use zstd::zstd_safe::{CParameter, FrameFormat};
    for p in [
        CParameter::Format(FrameFormat::Magicless),
        CParameter::ContentSizeFlag(false),
        CParameter::ChecksumFlag(false),
        CParameter::DictIdFlag(false),
    ] { c.set_parameter(p).unwrap(); }
}

fn bytes(data: &[u8], level: i32, chunk: usize, strip: bool) -> usize {
    let mut c = zstd::bulk::Compressor::new(level).unwrap();
    lean(&mut c);
    let mut total = 0;
    for part in data.chunks(chunk) {
        let f = c.compress(part).unwrap();
        let bh = f[2] as u32 | (f[3] as u32) << 8 | (f[4] as u32) << 16;
        let single = f[0] == 0 && bh & 1 == 1 && (bh >> 1) & 3 == 2 && (bh >> 3) as usize == f.len() - 5;
        total += if strip && single { f.len() - 5 } else { f.len() };
    }
    total
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();
    for d in &dirs {
        let mut v: Vec<_> = fs::read_dir(d).unwrap().filter_map(|e| e.ok())
            .map(|e| e.path()).filter(|p| p.is_file()).collect();
        v.sort();
        for p in v { files.push((p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap())); }
    }
    for level in [-5i32, 3, 9] {
        println!("\n### level {level}\n");
        println!("| file | 1 MiB frames | 128 KiB frames | 128 KiB, stripped | vs 1 MiB |");
        println!("|---|---|---|---|---|");
        let (mut ta, mut tb, mut tc) = (0usize, 0usize, 0usize);
        for (name, data) in &files {
            let a = bytes(data, level, 1 << 20, false);
            let b = bytes(data, level, 1 << 17, false);
            let c = bytes(data, level, 1 << 17, true);
            ta += a; tb += b; tc += c;
            println!("| {name} | {a} | {b} | {c} | {:+.2} % |", 100.0 * (c as f64 / a as f64 - 1.0));
        }
        println!("| **total** | **{ta}** | **{tb}** | **{tc}** | **{:+.2} %** |",
            100.0 * (tc as f64 / ta as f64 - 1.0));
    }
}
