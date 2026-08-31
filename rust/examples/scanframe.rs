//! What section 10.1 forbids, priced.
//!
//! "A compressor's output holds no run, no restricted alphabet and no
//! representable text worth looking for." That is true of zstd at its ordinary
//! levels and false at its negative ones, which limit the entropy coding of
//! literals and leave stretches of the source's own bytes in the frame. This
//! runs the full encoder over the frame and compares it with block-packing it.
//!
//! The pipeline this is worth comparing against -- zstd in front of a codec
//! with no compressor of its own -- is measured in binary2textbench, which
//! builds every codec from source in one process. This example measures only
//! what this format does to its own compressed payload.
//!
//!     cargo run --release --example scanframe -- bench/corpus

fn packed(n: usize) -> usize {
    let lc = if n < 90 { 1 } else if n < 8370 { 3 } else { 7 };
    2 + lc + 2 * (8 * n).div_ceil(13)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("bench/corpus".into());
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    paths.sort();
    for level in [-5i32, -1, 1, 3] {
        println!("\n### level {level}\n");
        println!("| file | block-packed | scanned | gain |");
        println!("|---|---|---|---|");
        let (mut tb, mut ts, mut tn) = (0usize, 0usize, 0usize);
        for p in &paths {
            let d = std::fs::read(p).unwrap();
            let z = zstd::bulk::compress(&d, level).unwrap();
            let b = packed(z.len());
            // The same frame bytes through the ordinary encoder, which is what
            // section 10.1 forbids an encoder from trying.
            let s = base91z::encode_plain(&z).len();
            tb += b; ts += s; tn += d.len();
            println!("| {} | {:.4} | {:.4} | {:+.2} % |",
                p.file_name().unwrap().to_string_lossy(),
                b as f64 / d.len() as f64,
                s as f64 / d.len() as f64,
                100.0 * (s as f64 / b as f64 - 1.0));
        }
        println!("| **total** | **{:.5}** | **{:.5}** | **{:+.2} %** |",
            tb as f64 / tn as f64, ts as f64 / tn as f64,
            100.0 * (ts as f64 / tb as f64 - 1.0));
    }
}
