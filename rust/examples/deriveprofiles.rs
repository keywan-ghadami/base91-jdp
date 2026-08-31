// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Derives the donor profile table of specification Section 8.2.
//!
//! Section 17.5 requires it: the table shipped with 0.4.0 was fitted against
//! the 0.3.0 R-Set, which held `-` and not NUL. The membership changed, `k` can
//! now reach eight, and the eighth rank had never been fitted at all.
//!
//! A profile is an ordered ranking of eight alphabet characters. A segment
//! whose mask has `k` bits set spends the first `k` of them as stand-ins, so
//! only those `k` become unusable as literals inside it -- which is why the
//! table is searched by encoded size and not by character frequency: what
//! matters is how often a donor's own character turns up in the text it would
//! have to stand in for.
//!
//! The search is greedy in two nested ways, as the 0.3.0 derivation was.
//! Profiles are added one at a time, each chosen to help most given the ones
//! already in the table; within a profile, positions are filled left to right,
//! the rest falling back to the rarity order. A table of fewer than
//! `NUM_PROFILES` is evaluated by repeating its last entry: a duplicate profile
//! is never the better of the two, so the encoder behaves as if it were absent.
//!
//!     python3 tools/traincorpus.py
//!     cargo run --release --example deriveprofiles

use std::fs;

use base91z::tables::{tuning, ALPHABET, NUM_PROFILES};

const POOL_SIZE: usize = 20;
const RANKS: usize = 8;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("bench/train".into());
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e} -- run python3 tools/traincorpus.py"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "train"))
        .collect();
    paths.sort();
    let files: Vec<Vec<u8>> = paths.iter().map(|p| fs::read(p).unwrap()).collect();
    assert!(!files.is_empty(), "no training data -- run python3 tools/traincorpus.py");
    let total: usize = files.iter().map(|f| f.len()).sum();
    eprintln!("training on {} files, {total} bytes", files.len());

    // Candidate pool: the rarest alphabet characters in the training text.
    // Letters and digits are excluded on principle rather than by frequency --
    // a rare capital is rare when all text is counted together and common in
    // the one file that uses it, so a letter donor breaks segments in bursts.
    // `-` is excluded because Section 8.2 forbids it as a donor.
    let mut counts = [0usize; 256];
    for f in &files {
        for &b in f {
            counts[b as usize] += 1;
        }
    }
    let mut pool: Vec<u8> = ALPHABET
        .iter()
        .copied()
        .filter(|&c| c != b'-' && !c.is_ascii_alphanumeric())
        .collect();
    pool.sort_by_key(|&c| (counts[c as usize], c));
    pool.truncate(POOL_SIZE);
    eprintln!(
        "pool: {}",
        pool.iter().map(|&c| c as char).collect::<String>()
    );

    let rarity_fill = |prefix: &[u8]| -> [u8; RANKS] {
        let mut out = [0u8; RANKS];
        let mut n = 0;
        for &c in prefix.iter().chain(pool.iter()) {
            if n == RANKS {
                break;
            }
            if !out[..n].contains(&c) {
                out[n] = c;
                n += 1;
            }
        }
        assert_eq!(n, RANKS, "pool too small to fill a profile");
        out
    };

    // A table of `built` real profiles, padded to NUM_PROFILES with repeats.
    let cost = |table: &[[u8; RANKS]]| -> usize {
        let mut full = [[0u8; RANKS]; NUM_PROFILES];
        for (i, slot) in full.iter_mut().enumerate() {
            *slot = table[i.min(table.len() - 1)];
        }
        tuning::set_profiles(full);
        files.iter().map(|f| base91z::encode_plain(f).len()).sum()
    };

    let mut table: Vec<[u8; RANKS]> = Vec::new();
    let mut curve: Vec<usize> = Vec::new();
    for p in 0..NUM_PROFILES {
        let mut chosen: Vec<u8> = Vec::new();
        for _ in 0..RANKS {
            let mut best = None;
            let mut best_cost = usize::MAX;
            for &c in &pool {
                if chosen.contains(&c) {
                    continue;
                }
                let mut trial = chosen.clone();
                trial.push(c);
                let mut cand = table.clone();
                cand.push(rarity_fill(&trial));
                let n = cost(&cand);
                if n < best_cost {
                    best_cost = n;
                    best = Some(c);
                }
            }
            chosen.push(best.expect("pool exhausted"));
        }
        table.push(rarity_fill(&chosen));
        let n = cost(&table);
        curve.push(n);
        eprintln!(
            "profile {p}  {}   {n}  ({:.5})",
            table[p].iter().map(|&c| c as char).collect::<String>(),
            n as f64 / total as f64
        );
    }

    println!("\n// derived by examples/deriveprofiles.rs");
    println!("pub const PROFILES: [[u8; 8]; {NUM_PROFILES}] = [");
    for prof in &table {
        let cs: Vec<String> = prof.iter().map(|&c| format!("b'{}'", c as char)).collect();
        println!("    [{}],", cs.join(", "));
    }
    println!("];");

    println!("\n| profiles | 1 | 2 | 3 | 4 |");
    println!("|---|---|---|---|---|");
    print!("| gain |");
    for i in 0..curve.len() {
        if i == 0 {
            print!(" — |");
        } else {
            print!(
                " {:.3} % |",
                (curve[i - 1] as f64 - curve[i] as f64) / total as f64 * 100.0
            );
        }
    }
    println!();
}
