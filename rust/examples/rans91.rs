// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A range-asymmetric-numeral-system coder whose output radix is 91.
//!
//! The block coder of Section 6 converts bits to characters: thirteen bits in,
//! two characters out. That is a *packer* with a fixed model -- every byte
//! value equally likely -- and it pays 0.097 % for using 8 192 of the 8 281
//! pair values. An rANS renormalising in base 91 instead of base 2 does the
//! same job with no packing loss at all, and, unlike a packer, it can carry a
//! model: give it symbol frequencies and it spends `-log91(p)` characters on a
//! symbol rather than `8/log2(91)`.
//!
//! This is the prototype that measures whether that is worth having. It is not
//! wired into the encoder and nothing in the specification refers to it.
//!
//!     cargo run --release --example rans91 -- bench/corpus/short

use std::collections::BTreeMap;

/// The output radix. This is the whole point.
const B: u64 = 91;
/// Total of the frequency table. A power of two so the divisions are shifts.
const M: u64 = 1 << 12;
/// The normalised state lives in `[L, B*L)`. `L` must be a multiple of `M`;
/// larger means less coding loss and a longer final state to write out.
const K: u64 = 16;
const L: u64 = K * M;

/// How many characters the final state costs, which is this coder's answer to
/// the block coder's flush field.
fn state_chars() -> usize {
    let mut n = 0;
    let mut hi = B * L - 1;
    while hi > 0 {
        hi /= B;
        n += 1;
    }
    n
}

/// A static model: frequency and cumulative frequency per byte, summing to `M`.
#[derive(Clone)]
struct Model {
    freq: [u32; 256],
    cum: [u32; 256],
    /// Inverse lookup, slot -> symbol, so decoding is a table read.
    slot: Vec<u8>,
}

impl Model {
    fn from_counts(counts: &[u64; 256]) -> Model {
        // Every symbol keeps at least one slot, or the coder cannot represent
        // a byte the training set never saw and the round trip is not total.
        let mut freq = [1u32; 256];
        let total: u64 = counts.iter().sum();
        let left = M - 256;
        if total > 0 {
            for s in 0..256 {
                let share = (counts[s] as u128 * left as u128 / total as u128) as u32;
                freq[s] += share;
            }
        }
        // Hand whatever rounding left over to the commonest symbol.
        let sum: u32 = freq.iter().sum();
        let top = (0..256).max_by_key(|&s| counts[s]).unwrap();
        freq[top] += M as u32 - sum;

        let mut cum = [0u32; 256];
        let mut c = 0u32;
        let mut slot = vec![0u8; M as usize];
        for s in 0..256 {
            cum[s] = c;
            for i in c..c + freq[s] {
                slot[i as usize] = s as u8;
            }
            c += freq[s];
        }
        debug_assert_eq!(c, M as u32);
        Model { freq, cum, slot }
    }

    /// Every byte equally likely, which is the model the block coder has.
    /// Not `from_counts` of nothing: that hands the rounding remainder to one
    /// symbol and the "uniform" model then costs 1.92 characters a byte.
    fn uniform() -> Model {
        Model::from_counts(&[1; 256])
    }
}

/// Encode. rANS is last-in-first-out, so the input is consumed backwards and
/// the digit stream comes out in the order a decoder wants it.
fn encode(data: &[u8], m: &Model) -> Vec<u8> {
    let mut x = L;
    let mut out: Vec<u8> = Vec::with_capacity(data.len() + 8);
    for &byte in data.iter().rev() {
        let f = m.freq[byte as usize] as u64;
        let c = m.cum[byte as usize] as u64;
        let x_max = K * B * f;
        while x >= x_max {
            out.push((x % B) as u8);
            x /= B;
        }
        x = (x / f) * M + (x % f) + c;
    }
    for _ in 0..state_chars() {
        out.push((x % B) as u8);
        x /= B;
    }
    debug_assert_eq!(x, 0);
    out.reverse();
    out
}

fn decode(digits: &[u8], n: usize, m: &Model) -> Vec<u8> {
    let sc = state_chars();
    let mut x = 0u64;
    for &d in &digits[..sc] {
        x = x * B + d as u64;
    }
    let mut i = sc;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let slot = (x % M) as u32;
        let s = m.slot[slot as usize];
        let f = m.freq[s as usize] as u64;
        let c = m.cum[s as usize] as u64;
        out.push(s);
        x = f * (x / M) + slot as u64 - c;
        while x < L {
            x = x * B + digits[i] as u64;
            i += 1;
        }
    }
    out
}

/// The same coder with `N` states running side by side.
///
/// One rANS state is a serial dependency: every symbol's renormalisation waits
/// on the previous symbol's state. Interleaved lanes break that chain, which
/// is how every fast rANS implementation reaches its speed, and it is the
/// difference between a prototype's number and the coder's real ceiling.
/// Symbol `i` belongs to lane `i % N`; the lanes share one digit stream, and
/// because the encoder runs backwards it pushes digits in exactly the order
/// the decoder consumes them.
const LANES: usize = 8;

fn encode_n(data: &[u8], m: &Model) -> Vec<u8> {
    let mut x = [L; LANES];
    let mut out: Vec<u8> = Vec::with_capacity(data.len() + 8 * LANES);
    for i in (0..data.len()).rev() {
        let byte = data[i];
        let lane = i % LANES;
        let f = m.freq[byte as usize] as u64;
        let c = m.cum[byte as usize] as u64;
        let x_max = K * B * f;
        let s = &mut x[lane];
        while *s >= x_max {
            out.push((*s % B) as u8);
            *s /= B;
        }
        *s = (*s / f) * M + (*s % f) + c;
    }
    for s in x.iter_mut() {
        for _ in 0..state_chars() {
            out.push((*s % B) as u8);
            *s /= B;
        }
    }
    out.reverse();
    out
}

fn decode_n(digits: &[u8], n: usize, m: &Model) -> Vec<u8> {
    let sc = state_chars();
    let mut x = [0u64; LANES];
    let mut i = 0;
    // The encoder flushed lane 0 first and then reversed, so lane LANES-1
    // comes off the front.
    for lane in (0..LANES).rev() {
        for _ in 0..sc {
            x[lane] = x[lane] * B + digits[i] as u64;
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let s = &mut x[k % LANES];
        let slot = (*s % M) as u32;
        let sym = m.slot[slot as usize];
        let f = m.freq[sym as usize] as u64;
        let c = m.cum[sym as usize] as u64;
        out.push(sym);
        *s = f * (*s / M) + slot as u64 - c;
        while *s < L {
            *s = *s * B + digits[i] as u64;
            i += 1;
        }
    }
    out
}

fn counts_of<'a>(samples: impl Iterator<Item = &'a [u8]>) -> [u64; 256] {
    let mut c = [0u64; 256];
    for s in samples {
        for &b in s {
            c[b as usize] += 1;
        }
    }
    c
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("bench/corpus/short".into());
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    let files: Vec<(String, Vec<u8>)> = paths
        .iter()
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            )
        })
        .collect();
    let total: usize = files.iter().map(|(_, d)| d.len()).sum();

    println!("radix {B}, M = {M}, L = {L}, final state {} characters\n", state_chars());

    // Correctness first: the coder is worthless if it does not round trip.
    let uniform = Model::uniform();
    for (name, data) in &files {
        let d = encode(data, &uniform);
        assert_eq!(&decode(&d, data.len(), &uniform), data, "{name}");
    }
    println!("round trip: {} files, all exact\n", files.len());

    // A uniform model over incompressible bytes has a known answer, so if the
    // coder disagrees with it the coder is wrong and nothing below means
    // anything. Check that before reporting a single ratio.
    {
        let mut s = 0x2545_F491_4F6C_DD1Du64;
        let noise: Vec<u8> = (0..200_000)
            .map(|_| { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 24) as u8 })
            .collect();
        let d = encode(&noise, &uniform);
        assert_eq!(decode(&d, noise.len(), &uniform), noise, "noise");
        let floor = noise.len() as f64 * 8.0 / 91f64.log2();
        println!(
            "sanity: {} bytes of noise -> {} characters, floor {:.1}, {:+.1}\n",
            noise.len(), d.len(), floor, d.len() as f64 - floor
        );
    }

    println!("### Per file, uniform model against its own floor\n");
    println!("| file | bytes | rANS | floor | delta |");
    println!("|---|---|---|---|---|");
    for (name, data) in &files {
        let d = encode(data, &uniform).len();
        let floor = data.len() as f64 * 8.0 / 91f64.log2();
        println!("| {name} | {} | {d} | {floor:.0} | {:+.0} |", data.len(), d as f64 - floor);
    }
    println!();

    println!("| coder | chars/byte | chars |");
    println!("|---|---|---|");
    println!("| block coder, as shipped | {:.6} | {} |", 16.0 / 13.0,
        files.iter().map(|(_, d)| 2 * (8 * d.len()).div_ceil(13)).sum::<usize>());
    let u: usize = files.iter().map(|(_, d)| encode(d, &uniform).len()).sum();
    println!("| rANS, uniform model | {:.6} | {u} |", u as f64 / total as f64);
    let floor = total as f64 * 8.0 / 91f64.log2();
    println!("| theoretical floor, 8 bits in 91 symbols | {:.6} | {:.0} |",
        8.0 / 91f64.log2(), floor);
    println!("| rANS above the floor | | {:+.1} characters over {} files |",
        u as f64 - floor, files.len());

    // Trained, leave-one-out, so no sample is scored against itself.
    let mut trained = 0usize;
    for (i, (name, data)) in files.iter().enumerate() {
        let c = counts_of(files.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, (_, d))| d.as_slice()));
        let m = Model::from_counts(&c);
        let d = encode(data, &m);
        assert_eq!(&decode(&d, data.len(), &m), data, "{name}");
        trained += d.len();
    }
    println!("| rANS, trained order-0, leave-one-out | {:.4} |", trained as f64 / total as f64);

    let jdp: usize = files.iter().map(|(_, d)| base91z::encode_plain(d).len()).sum();
    println!("| **Base91z as it stands** | **{:.4}** | {jdp} |", jdp as f64 / total as f64);

    // Throughput, which is where a division per symbol has to answer for
    // itself against a table read per pair.
    let rate = |bytes: usize, mut f: Box<dyn FnMut()>| -> f64 {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            f();
            best = best.min(t.elapsed().as_secs_f64());
        }
        bytes as f64 / 1e6 / best
    };
    let big: Vec<u8> = files.iter().flat_map(|(_, d)| d.iter().copied()).collect();
    let n = big.len();
    let m2 = uniform.clone();
    let digits = encode(&big, &uniform);
    println!("\n### Throughput on {} MB of the corpus\n", n / 1_000_000);
    println!("| stage | MB/s |");
    println!("|---|---|");
    {
        let b = &big;
        println!("| block coder alone (`block_only`) | {:.0} |",
            rate(n, Box::new(|| { std::hint::black_box(base91z::bench::block_only(b).len()); })));
    }
    {
        let b = &big;
        println!("| the whole encoder, scan included | {:.0} |",
            rate(n, Box::new(|| { std::hint::black_box(base91z::encode_plain(b).len()); })));
    }
    {
        let (b, m) = (&big, &uniform);
        println!("| rANS encode, uniform model | {:.0} |",
            rate(n, Box::new(|| { std::hint::black_box(encode(b, m).len()); })));
    }
    {
        let (d, m) = (&digits, &m2);
        println!("| rANS decode, uniform model | {:.0} |",
            rate(n, Box::new(|| { std::hint::black_box(decode(d, n, m).len()); })));
    }
    let wide = encode_n(&big, &uniform);
    assert_eq!(decode_n(&wide, n, &uniform), big, "interleaved round trip");
    {
        let (b, m) = (&big, &uniform);
        println!("| rANS encode, {LANES} lanes | {:.0} |",
            rate(n, Box::new(|| { std::hint::black_box(encode_n(b, m).len()); })));
    }
    {
        let (d, m) = (&wide, &m2);
        println!("| rANS decode, {LANES} lanes | {:.0} |",
            rate(n, Box::new(|| { std::hint::black_box(decode_n(d, n, m).len()); })));
    }
    println!(
        "\n{LANES} lanes cost {} characters of extra state and give {:+.3} % in size.",
        wide.len() as i64 - digits.len() as i64,
        100.0 * (wide.len() as f64 / digits.len() as f64 - 1.0)
    );

    // Where each one wins, since an aggregate hides it. Only the short group
    // is named `NN-category-what`; on any other directory the key is noise.
    if !files.iter().all(|(n, _)| n.split('-').nth(1).is_some_and(|c| !c.is_empty())) {
        return;
    }
    println!("\n### By category\n");
    println!("| kind | bytes | jdp | rANS uniform | rANS trained |");
    println!("|---|---|---|---|---|");
    let mut by: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for (i, (name, data)) in files.iter().enumerate() {
        let c = counts_of(files.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, (_, d))| d.as_slice()));
        let m = Model::from_counts(&c);
        let cat = name.split('-').nth(1).unwrap_or("?").to_string();
        let e = by.entry(cat).or_default();
        e.0 += data.len();
        e.1 += base91z::encode_plain(data).len();
        e.2 += encode(data, &uniform).len();
        e.3 += encode(data, &m).len();
    }
    for (cat, (n, j, u, t)) in &by {
        println!(
            "| {cat} | {n} | **{:.4}** | {:.4} | {:.4} |",
            *j as f64 / *n as f64,
            *u as f64 / *n as f64,
            *t as f64 / *n as f64
        );
    }
}
