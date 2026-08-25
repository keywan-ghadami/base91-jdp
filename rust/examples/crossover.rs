//! Where compression starts to win, and what "compression is mandatory"
//! therefore cannot mean.

fn main() {
    // Compressible: a JSON-shaped record repeated, which is what a caller
    // encoding many similar short payloads actually has.
    let unit = br#"{"id":184223,"name":"Ada Lovelace","status":"shipped"},"#;
    println!("| bytes | plain | zstd −5 | zstd 3 | smaller |");
    println!("|---|---|---|---|---|");
    for len in [16usize, 32, 54, 108, 216, 432, 864, 1728, 3456, 6912, 13824] {
        let data: Vec<u8> = unit.iter().cycle().take(len).copied().collect();
        let p = base91_jdp::encode(&data).len();
        let z5 = base91_jdp::encode_zstd(&data, -5).unwrap().len();
        let z3 = base91_jdp::encode_zstd(&data, 3).unwrap().len();
        let best = if z3.min(z5) < p { "zstd" } else { "**plain**" };
        println!(
            "| {len} | {:.3} | {:.3} | {:.3} | {best} |",
            p as f64 / len as f64,
            z5 as f64 / len as f64,
            z3 as f64 / len as f64
        );
    }

    println!("\n### The same for a payload with a run in it\n");
    println!("| bytes | plain | zstd −5 | smaller |");
    println!("|---|---|---|---|");
    for len in [16usize, 32, 64, 128, 256, 512, 1024] {
        // A fixed-width record, zero padded: what ZRUN and ZMIX are for.
        let mut data = Vec::new();
        while data.len() < len {
            data.extend_from_slice(b"ORD-184223");
            data.extend(std::iter::repeat(0u8).take(22));
        }
        data.truncate(len);
        let p = base91_jdp::encode(&data).len();
        let z5 = base91_jdp::encode_zstd(&data, -5).unwrap().len();
        println!(
            "| {len} | **{:.3}** | {:.3} | {} |",
            p as f64 / len as f64,
            z5 as f64 / len as f64,
            if z5 < p { "zstd" } else { "**plain**" }
        );
    }
}
