//! The alternative to encoding is not encoding -- but "as-is" is not free.
//!
//! A value written straight into a JSON string pays for every quotation mark,
//! backslash and control character it contains. For hex and base64 that is
//! nothing, which is why the packed bases have to beat a free alternative. For
//! anything with structure in it, it is not nothing, and that is the case
//! passthrough was built for.

use std::fs;

/// What `s` costs inside a JSON string: the escaping RFC 8259 forces.
fn embedded(s: &[u8]) -> usize {
    s.iter()
        .map(|b| match b {
            b'"' | b'\\' | 8 | 12 | b'\n' | b'\r' | b'\t' => 2,
            0x00..=0x1F => 6,
            _ => 1,
        })
        .sum()
}

fn utf8_ok(b: &[u8]) -> bool {
    std::str::from_utf8(b).is_ok()
}

fn main() {
    let mut v: Vec<_> = fs::read_dir("bench/corpus/short").unwrap().filter_map(|e| e.ok())
        .map(|e| e.path()).filter(|p| p.is_file()).collect();
    v.sort();

    println!("| sample | bytes | as-is in JSON | jdp | jdp is |");
    println!("|---|---|---|---|---|");
    let (mut n_win, mut n_lose, mut n_binary) = (0, 0, 0);
    let (mut sum_asis, mut sum_jdp, mut sum_in) = (0usize, 0usize, 0usize);
    for p in &v {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let data = fs::read(p).unwrap();
        let j = base91_jdp::encode(&data).len();
        if !utf8_ok(&data) {
            n_binary += 1;
            continue; // cannot go into a JSON string at all
        }
        let a = embedded(&data);
        sum_asis += a;
        sum_jdp += j;
        sum_in += data.len();
        if j < a { n_win += 1 } else { n_lose += 1 }
        let cat = name.split('-').nth(1).unwrap_or("?");
        if ["43-text-json-record", "48-text-log-line", "49-text-sql-statement",
            "50-text-http-request-head", "09-hex-sha-256-digest", "14-hexsep-uuid-v4",
            "25-b64u-jwt-three-segments", "20-crock-ulid-crockford"]
            .contains(&name.as_str())
        {
            println!(
                "| {} ({cat}) | {} | {a} | {j} | {} |",
                name.splitn(3, '-').nth(2).unwrap_or(&name),
                data.len(),
                if j < a {
                    format!("−{:.0} %", (1.0 - j as f64 / a as f64) * 100.0)
                } else {
                    format!("**+{:.0} %**", (j as f64 / a as f64 - 1.0) * 100.0)
                }
            );
        }
    }
    println!(
        "\nOf the {} samples that are text at all, jdp is smaller than writing them \
         straight into the document on {n_win} and larger on {n_lose}. \
         {n_binary} are binary and cannot be written straight in.",
        n_win + n_lose
    );
    println!(
        "\nOver those text samples: as-is {} characters, jdp {} — {:.1} % apart, \
         on {} input bytes.",
        sum_asis,
        sum_jdp,
        (1.0 - sum_jdp as f64 / sum_asis as f64) * 100.0,
        sum_in
    );
}
