# Benchmarks

Everything measured about the current format lives in **Section 17 of the
[specification](../spec/base91-jdp-v0.4.0.md)**, next to the constants it
justifies, and is produced by the examples in
[`rust/`](../rust/README.md). This directory holds what those examples read.

```sh
python3 bench/corpus.py            # fetch every group (about 210 MB)
python3 bench/corpus.py --core     # the thirteen core files only
python3 bench/corpus.py --short    # the authored field samples only
```

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

`base85n/` runs the upstream Go implementation, v0.5.1, so that the comparisons
come from an execution rather than from its documentation. It needs Go on the
path; without it those columns are left out.

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
