// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What the compression level costs, and what it costs in determinism.
//!
//! The level is a parameter of the encoding and not of the payload, so the
//! same bytes encode to a different string at each one -- all of them valid,
//! all of them decoding back to the same bytes. This prints that, because the
//! README says it and a claim in this repository should have a run behind it.
//!
//!     cargo run --release --example levels -- bench/corpus/commonmark-spec.txt

use std::collections::BTreeSet;

const LEVELS: [i32; 5] = [-5, 1, 3, 9, 19];

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: levels <file> [bytes]");
        std::process::exit(2);
    });
    let take: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(40_000);

    let all = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let data = &all[..take.min(all.len())];

    println!("{} bytes of {path}\n", data.len());

    // Twice at the same level, first: within one level and one build this is
    // deterministic, and specification Section 11.3 is why.
    assert_eq!(
        base91z::encode(data),
        base91z::encode(data),
        "the same call twice disagreed with itself"
    );

    let mut texts = BTreeSet::new();
    for level in LEVELS {
        let text = base91z::encode_at(data, level).expect("compressor");
        assert_eq!(base91z::decode(&text).expect("own output"), data);
        println!(
            "  level {level:>3}   {:>8} characters   {:.5} per byte{}",
            text.len(),
            text.len() as f64 / data.len() as f64,
            if level == base91z::DEFAULT_LEVEL { "   <- the default" } else { "" }
        );
        texts.insert(text);
    }

    println!(
        "\n{} levels, {} distinct strings, all decoding to the same {} bytes.",
        LEVELS.len(),
        texts.len(),
        data.len()
    );
    println!(
        "encode_plain, which has no level: {} characters, and stable within a version.",
        base91z::encode_plain(data).len()
    );
    println!("\nSign the payload, not the text. `decode(encode(x)) == x` is the promise;");
    println!("\"the same x gives the same text\" is not, across levels or builds.");
}
