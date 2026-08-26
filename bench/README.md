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
[`wire_samples.py`](wire_samples.py) from invented values. It is the only group
that reaches the packed bases of Section 9 at all, and the only one where three
characters of segment overhead are visible. Its samples are chosen to exercise
the classes, which makes it the wrong instrument for estimating how often each
occurs in real traffic.

Nothing is vendored: every downloaded sample is pulled from a pinned archive
and checked against a recorded SHA-256, so a rerun either reproduces the same
bytes or fails loudly.

## The reference implementation of Base85N

`base85n/` runs the upstream Go implementation, v0.5.1, so that the size
comparisons come from an execution rather than from its documentation. It needs
Go on the path; without it those columns are left out.

For **throughput**, Go against Rust would measure the languages. Base85N also
ships a Rust implementation with the same shape as this one — a scalar path, an
optional nightly vector path, a parallel encoder — and
[`rust/examples/headtohead.rs`](../rust/README.md) links it directly, so both
sides of every number in specification Section 17.21 are compiled by the same
compiler at the same optimisation level and timed by the same loop in one
process. It needs a Base85N checkout beside this repository:

```sh
git clone https://github.com/keywan-ghadami/base85n ../keywan-ghadami/base85n
cargo run --release --features base85n --manifest-path rust/Cargo.toml \
    --example headtohead -- bench/corpus
```

The `base85n` feature is off by default and no part of the library uses it.

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
