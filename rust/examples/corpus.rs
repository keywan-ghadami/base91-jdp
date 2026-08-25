// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What the prototype actually encodes to, and how fast.
//!
//! The specification's section 17.4 is a projection, computed from the
//! format's arithmetic rather than from an implementation. This is the
//! implementation, so this is where the projection is either confirmed or
//! withdrawn.
//!
//!     cargo run --release --example corpus -- bench/corpus [bench/corpus/silesia]

use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dirs = if args.is_empty() { vec!["bench/corpus".to_string()] } else { args };

    println!("| file | bytes | chars/byte | serial | parallel x4 | spliced + rejoined |");
    println!("|---|---|---|---|---|---|");
    let (mut tin, mut tout) = (0usize, 0usize);

    for dir in &dirs {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.path())
            .collect();
        names.sort();
        for path in names {
            let data = fs::read(&path).unwrap();
            if data.is_empty() {
                continue;
            }
            let text = base91_jdp::encode(&data);
            // Round trip, so a size in this table came from output that reads
            // back. A benchmark that does not decode measures nothing.
            let back = base91_jdp::decode(&text).expect("decode");
            assert_eq!(back, data, "{} does not round trip", path.display());

            let serial = throughput(&data, || {
                std::hint::black_box(base91_jdp::encode(&data).len());
            });
            let (par_text, stats) = base91_jdp::encode_parallel_stats(&data, 4);
            assert_eq!(par_text, text, "{}: parallel differs", path.display());
            let parallel = throughput(&data, || {
                std::hint::black_box(base91_jdp::encode_parallel(&data, 4).len());
            });
            let total = stats.spliced + stats.repaired;
            let rate = if total == 0 {
                "n/a".to_string()
            } else {
                format!(
                    "{:.0} % + {:.0} %",
                    100.0 * stats.spliced as f64 / total as f64,
                    100.0 * stats.rejoined as f64 / total as f64
                )
            };
            tin += data.len();
            tout += text.len();
            println!(
                "| {} | {} | {:.4} | {:.0} MB/s | {:.0} MB/s | {} |",
                name(&path),
                data.len(),
                text.len() as f64 / data.len() as f64,
                serial,
                parallel,
                rate
            );
        }
    }
    println!("| **total** | {tin} | **{:.5}** | | | |", tout as f64 / tin as f64);
}

fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// MB/s of input, best of a few rounds: the machine is shared and the spread
/// is wide, so the minimum time is the honest number.
fn throughput(data: &[u8], mut f: impl FnMut()) -> f64 {
    let rounds = if data.len() > 8 << 20 { 2 } else { 5 };
    let mut best = f64::MAX;
    for _ in 0..rounds {
        let t = Instant::now();
        f();
        let s = t.elapsed().as_secs_f64();
        if s < best {
            best = s;
        }
    }
    data.len() as f64 / 1e6 / best
}
