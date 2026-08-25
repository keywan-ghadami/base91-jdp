//! Is a packed base ever the right answer?
//!
//! The claim to test: a value is either already text that JSON accepts -- a
//! hex digest, a base64 token -- in which case you write it into the document
//! and encode nothing; or it is binary, in which case block mode is what it
//! wants. If that holds, the packed bases only ever see input that should not
//! have been handed to an encoder at all.

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn b64(b: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let mut v = 0u32;
        for i in 0..3 { v = (v << 8) | *c.get(i).unwrap_or(&0) as u32; }
        for i in 0..4 {
            s.push(if i <= c.len() { A[((v >> (18 - 6 * i)) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

fn main() {
    let cases: [(&str, Vec<u8>); 4] = [
        ("SHA-256 digest", (0..32u8).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect()),
        ("UUID", (0..16u8).map(|i| i.wrapping_mul(53).wrapping_add(7)).collect()),
        ("AES-256 key", (0..32u8).map(|i| i.wrapping_mul(97).wrapping_add(3)).collect()),
        ("64-byte token", (0..64u8).map(|i| i.wrapping_mul(29).wrapping_add(5)).collect()),
    ];

    println!("### The same value, four ways into a JSON string\n");
    println!("| value | raw bytes | hex, written as-is | base64, as-is | **jdp on the raw bytes** | jdp on the hex text |");
    println!("|---|---|---|---|---|---|");
    for (name, raw) in &cases {
        let h = hex(raw);
        let b = b64(raw);
        let jdp_raw = base91z::encode_plain(raw);
        let jdp_hex = base91z::encode_plain(h.as_bytes());
        println!(
            "| {name} | {} B | {} | {} | **{}** | {} |",
            raw.len(),
            h.len(),
            b.len(),
            jdp_raw.len(),
            jdp_hex.len()
        );
    }

    println!("\n### What carried the raw bytes\n");
    for (name, raw) in &cases {
        let t = base91z::encode_plain(raw);
        let used = base91z::explain(&t).unwrap();
        let carried: usize = used.iter().map(|(_, n)| n).sum();
        let names: Vec<String> = used.iter().map(|(c, n)| format!("{c} {n} B")).collect();
        println!(
            "- {name}: {} block mode {} B",
            if names.is_empty() { String::new() } else { names.join(", ") + ", " },
            raw.len() - carried
        );
    }

    println!("\n### And a value that is natively text\n");
    println!("| value | as-is in JSON | jdp | jdp is |");
    println!("|---|---|---|---|");
    for (name, text) in [
        ("JWT", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxODQyMjMifQ.3Yv1kQ8Zr7pNc2LxWmA4hTgKdF9sBvE0uJqRnXoYiPs"),
        ("UUID as text", "b0f1c2d3-4e5a-4b6c-8d9e-0f1a2b3c4d5e"),
        ("hex digest as text", "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"),
        ("ULID", "01ARZ3NDEKTSV4RRFFQ69G5FAV"),
    ] {
        let j = base91z::encode_plain(text.as_bytes()).len();
        println!(
            "| {name} | {} | {j} | {} |",
            text.len(),
            if j < text.len() {
                format!("{:.0} % smaller", (1.0 - j as f64 / text.len() as f64) * 100.0)
            } else {
                format!("**{:.0} % larger**", (j as f64 / text.len() as f64 - 1.0) * 100.0)
            }
        );
    }
}
