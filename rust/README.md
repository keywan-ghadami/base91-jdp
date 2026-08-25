# base91-jdp, Rust prototype

An encoder and decoder for **specification v0.4.0**, written to find out
whether the format encodes at the density the specification projected, and
whether the parallel and vector paths it was shaped for actually pay.

```rust
let data = b"{\"user\":\"ada\",\"id\":42}";
let text = base91_jdp::encode(data);
assert_eq!(base91_jdp::decode(&text).unwrap(), data);
```

```sh
cargo test                                    # round trip, canonicity, adversarial decode
cargo run --release --example corpus -- ../bench/corpus
cargo run --release --example sweep  -- ../bench/corpus
cargo +nightly test --features simd           # the same, with the vector paths on
```

## What it implements, and what it does not

Passthrough with the eight-member R-Set and the donor profiles, the packed
bases, the run classes and the chained-gap classes, the block coder under all
of them, and a decoder for every one. **Class 20, the zstd segment, is not
implemented** and is rejected on decode with `Code::Unsupported`: it needs a
compression library, and none of the questions this prototype exists to answer
are about zstd.

## What it found

Three things the specification did not say before this crate encoded a byte,
and one it said wrongly.

**The prefix scans were swallowing the runs.** The first run over the benchmark
corpus produced 1.03809 characters per byte where the specification projected
1.00464. Passthrough carries a zero byte at one character each — NUL is an
R-Set member — so the greedy prefix scan of section 11.1 was absorbing exactly
the runs that `ZRUN` carries at eighty-nine for three characters. `tar` came out
at 1.0018 against a projected 0.7823. The **run break** is that finding, and it
is now normative in section 11.1.

**`MIN_BINARY_RUN` should not exist.** 0.3.0 set it to 4, measured against a
structure this version does not have. Swept here, zero is the best value: with
typed segments there is nothing for it to prevent, and forcing four bytes
through block mode after every segment costs 0.3 %.

Together those two are three percent of the corpus, and they take the encoder
from behind Base85N to ahead of it with no compressor on either side:

| | Base85N | jdp 0.3.0 | 0.4.0 projected | **0.4.0, this crate** |
|---|---|---|---|---|
| core corpus, 6.52 MB | 1.00698 | 1.09650 | 1.00464 | **0.97944** |

**Parallel encoding works, and the obvious join does not.** Cutting the input
at multiples of thirteen bytes lets each worker start with an empty accumulator
(section 14.5). Splicing a worker's output only where that assumption held
outright fires on a fifth to a half of chunks, and repairing a whole chunk
otherwise makes the parallel encoder slower than the serial one. Bounding the
repair at the first segment boundary both paths reach gives 2 to 3.5× on four
threads, with output byte-identical to a serial encode — asserted by the tests
at four chunk sizes, down to a single symbol group.

**The `simd` feature does not pay yet**, and the flag stays off by default for
that reason. Vectorising the run scan is neutral. Four arrangements of a vector
fast-forward for the passthrough prefix scan all measured slower than the
scalar loop, because the bytes that stop such a skip — the R-Set members and
the donors — are precisely the frequent characters of the text the scan runs
on. `src/simd.rs` records each arrangement, its number, and what would be worth
trying next.

## A feature flag for more speed

`--features simd` needs **nightly**, because `std::simd` is unstable. It is the
only thing in this crate that is nightly, and the crate is stable-only and
portable without it. Enabling it may never change output: the vector paths
answer questions the scalar code would answer the same way, a conservative
answer costs a scalar step, and the whole test suite runs under both.

```sh
cargo +nightly build --release --features simd
```

## Status

This is a prototype for a draft. The wire format may still change, nothing here
is published to crates.io, and the specification's section 20 says what review
would be most useful. Bugs found here are bugs in the format until shown
otherwise — that is what it is for.
