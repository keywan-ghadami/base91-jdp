//! Is a lean frame's remaining header redundant with the segment that carries it?
//!
//!     cargo run --release --features zstd --example strip

fn lean(c: &mut zstd::bulk::Compressor<'static>) {
    use zstd::zstd_safe::{CParameter, FrameFormat};
    for p in [
        CParameter::Format(FrameFormat::Magicless),
        CParameter::ContentSizeFlag(false),
        CParameter::ChecksumFlag(false),
        CParameter::DictIdFlag(false),
    ] { c.set_parameter(p).unwrap(); }
}

fn main() {
    let cases: Vec<(String, Vec<u8>)> = vec![
        ("json x2".into(), br#"{"id":184223,"name":"Ada Lovelace"},{"id":184223,"name":"Ada Lovelace"}"#.to_vec()),
        ("zeros 300".into(), vec![0u8; 300]),
        ("json x40".into(), br#"{"id":184223,"name":"Ada Lovelace","status":"shipped"},"#.iter().cycle().take(2160).copied().collect()),
        ("128 KiB text".into(), std::iter::repeat(b"the quick brown fox jumps over the lazy dog. ").flatten().copied().take(131_072).collect()),
        ("129 KiB text".into(), std::iter::repeat(b"the quick brown fox jumps over the lazy dog. ").flatten().copied().take(132_000).collect()),
        ("1 MiB text".into(), std::iter::repeat(b"the quick brown fox jumps over the lazy dog. ").flatten().copied().take(1 << 20).collect()),
    ];

    println!("| case | level | frame | head | last | type | size | == len-5 |");
    println!("|---|---|---|---|---|---|---|---|");
    for (name, data) in &cases {
        for level in [-5i32, 3, 9, 19] {
            let mut c = zstd::bulk::Compressor::new(level).unwrap();
            lean(&mut c);
            let f = c.compress(data).unwrap();
            let (h0, h1) = (f[0], f[1]);
            let bh = f[2] as u32 | (f[3] as u32) << 8 | (f[4] as u32) << 16;
            let (last, ty, size) = (bh & 1, (bh >> 1) & 3, bh >> 3);
            println!("| {name} | {level} | {} | {h0:02x} {h1:02x} | {last} | {ty} | {size} | {} |",
                f.len(), if size as usize == f.len() - 5 { "yes" } else { "**no**" });
        }
    }

    // The reconstruction, on the cases where the shape holds.
    println!("\n### Strip five bytes, rebuild them, decompress\n");
    for (name, data) in &cases {
        for level in [-5i32, 3, 9, 19] {
            let mut c = zstd::bulk::Compressor::new(level).unwrap();
            lean(&mut c);
            let f = c.compress(data).unwrap();
            let bh = f[2] as u32 | (f[3] as u32) << 8 | (f[4] as u32) << 16;
            if f[0] != 0 || bh & 1 != 1 || (bh >> 1) & 3 != 2 || (bh >> 3) as usize != f.len() - 5 {
                println!("- {name} @ {level}: not a single compressed block, keep the frame");
                continue;
            }
            let body = &f[5..];
            // windowLog 17: a single block decompresses to at most 128 KiB, so
            // no back-reference in it can reach further.
            let n = body.len() as u32;
            let bh2 = 1u32 | 2 << 1 | n << 3;
            let mut rebuilt = vec![0x00u8, 0x38, bh2 as u8, (bh2 >> 8) as u8, (bh2 >> 16) as u8];
            rebuilt.extend_from_slice(body);
            let mut d = zstd::stream::read::Decoder::new(&rebuilt[..]).unwrap();
            d.set_parameter(zstd::zstd_safe::DParameter::Format(zstd::zstd_safe::FrameFormat::Magicless)).unwrap();
            let mut out = Vec::new();
            use std::io::Read;
            match d.read_to_end(&mut out) {
                Ok(_) if out == *data => println!("- {name} @ {level}: {} -> {} bytes, round trips", f.len(), body.len()),
                Ok(_) => println!("- {name} @ {level}: **decoded to the wrong bytes**"),
                Err(e) => println!("- {name} @ {level}: **{e}**"),
            }
        }
    }
}
