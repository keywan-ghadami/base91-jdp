//! The short group under every compression setting, as one table.
//!
//!     cargo run --release --features zstd --example shortnow

fn main() {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir("bench/corpus/short")
        .expect("run python3 bench/corpus.py --short")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    files.sort();
    let data: Vec<Vec<u8>> = files.iter().map(|p| std::fs::read(p).unwrap()).collect();
    let total: usize = data.iter().map(Vec::len).sum();

    let plain: usize = data.iter().map(|d| base91z::encode_plain(d).len()).sum();
    println!("| setting | chars/byte |");
    println!("|---|---|");
    println!("| base64 | {:.4} |", data.iter().map(|d| 4 * d.len().div_ceil(3)).sum::<usize>() as f64 / total as f64);
    println!("| plain, no compressor | **{:.4}** |", plain as f64 / total as f64);
    for level in [-5i32, 3, 19] {
        let z: usize = data.iter().map(|d| base91z::encode_zstd(d, level).unwrap().len()).sum();
        let a: usize = data.iter().map(|d| base91z::encode_auto(d, level).unwrap().len()).sum();
        println!("| zstd {level} in a segment | {:.4} |", z as f64 / total as f64);
        println!("| compressing where it wins, level {level} | **{:.4}** |", a as f64 / total as f64);
    }
}
