# Benchmarks

Everything measured about the current format lives in **Section 17 of the
[specification](../spec/base91z-v0.4.0.md)**, next to the constants it
justifies, and is produced by the examples in
[`rust/`](../rust/README.md). This directory holds what those examples read.

```sh
bench/fetch.sh                     # core and short, what CI measures
bench/fetch.sh all                 # adds silesia: 202 MiB
```

The corpus itself lives in
[binary2textbench](https://github.com/keywan-ghadami/binary2textbench), which
measures this codec against Base64, classic basE91, Ascii85, Base85N and
Base94Max on the same bytes. It used to live here, in `bench/corpus.py` and
`bench/wire_samples.py`; Base85N carried a second copy of the same thing, and
two corpus generators that are supposed to agree are a bug waiting to happen.
`fetch.sh` fills `bench/corpus/` exactly as before, so every example in
[`rust/`](../rust/README.md) reads it unchanged.

## The three groups

**core** — thirteen real files, 6.52 MB, one per input class: three binary
container formats, an uncompressed source tar, a JSON dataset pretty-printed
and minified, JavaScript, CSS and Python source, the CommonMark specification,
a Markdown changelog, a JPEG and a PNG. It is Base85N's corpus unchanged, which
is what makes the comparison against it a comparison.

**silesia** — the twelve files and 202 MiB that compression work has been
reported against since 2003. It is here because thirteen files picked by a
codec's own author are a weak basis for a claim about real data, and Silesia
was assembled by somebody else before this encoding existed.

**short** — fifty-five field-level samples under 200 bytes, authored in
[`corpus/wire_samples.py`](https://github.com/keywan-ghadami/binary2textbench/blob/main/corpus/wire_samples.py) from
invented values. It is the only group
that reaches the packed bases of Section 9 at all, and the only one where three
characters of segment overhead are visible. Its samples are chosen to exercise
the classes, which makes it the wrong instrument for estimating how often each
occurs in real traffic.

Nothing is vendored: every downloaded sample is pulled from a pinned archive
and checked against a recorded SHA-256, so a rerun either reproduces the same
bytes or fails loudly.

## Measuring against the other codecs

Nothing here does that any more. Sizes and throughput against Base64, classic
basE91, Ascii85, Base85N and Base94Max are measured in
[binary2textbench](https://github.com/keywan-ghadami/binary2textbench), which
builds every codec from source and runs them in one process, so what is
compared is six encodings and not six languages — and which puts the JSON
escaping inside the clock, where an alphabet containing `"` or `\\` pays for
itself twice.

It used to be measured here twice over: `bench/base85n/` ran the upstream Go
implementation for sizes, and `rust/examples/headtohead.rs`, `against.rs` and
`decoderate.rs` linked Base85N's Rust crate for throughput. That link was an
optional path dependency on a checkout *outside* this repository, and Cargo
reads a path dependency whether or not its feature is enabled — so `cargo
test`, `cargo clippy` and every example needed a second repository on disk
before they would even resolve. The comparison is better done in one place that
has all the codecs; the dependency went with it.

`.github/workflows/bench.yml` calls that repository's composite action on every
pull request, and the numbers it produced are still in Sections 17.21 and 17.22
of the specification.

## Running the measurements

```sh
cd rust
cargo run --release --example corpus  -- ../bench/corpus     # size and speed
cargo run --release --example short   -- ../bench/corpus/short
cargo run --release --example compress -- ../bench/corpus/countries.json
cargo run --release --example ablate                          # what each class is worth
cargo run --release --example sweep   -- ../bench/corpus     # the constants
```

Measurements of the superseded v0.3.0 are in
[`history/javascript-v0.3.0/results/`](../history/javascript-v0.3.0/results/RESULTS.md).
They are correct about that version and say nothing about this one.
