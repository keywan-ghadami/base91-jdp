//! Where the decoder spends its time, file by file, against Base85N's.
//!
//!     cargo run --release --features base85n --example decoderate -- bench/corpus

use std::time::Instant;

fn rate(bytes: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    bytes as f64 / 1e6 / best
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("bench/corpus".into());
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    paths.sort();
    println!("| file | classes it decodes through | jdp | Base85N |");
    println!("|---|---|---|---|");
    for p in &paths {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(p).unwrap();
        let a = base91z::encode_plain(&data);
        let b = base85n::encode(&data);
        // What the stream is made of, largest first, so a slow row can be read.
        let mut used = base91z::explain(&a).unwrap();
        used.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let carried: usize = used.iter().map(|(_, n)| n).sum();
        let mut what: Vec<String> = used.iter().take(2)
            .map(|(c, n)| format!("{c} {}%", 100 * n / data.len())).collect();
        if carried < data.len() {
            what.push(format!("block {}%", 100 * (data.len() - carried) / data.len()));
        }
        println!("| {name} | {} | {:.0} MB/s | {:.0} MB/s |", what.join(", "),
            rate(data.len(), || { std::hint::black_box(base91z::decode(&a).unwrap().len()); }),
            rate(data.len(), || { std::hint::black_box(base85n::decode(&b).unwrap().len()); }));
    }
}
