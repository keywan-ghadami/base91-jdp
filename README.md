# base91-jdp

**basE91 on an alphabet JSON never has to escape, with typed segments and zstd
inside.**

[![Spec](https://img.shields.io/badge/spec-v0.4.0%20draft-orange)](spec/base91-jdp-v0.4.0.md)
[![Implementation](https://img.shields.io/badge/implementation-Rust%20prototype-blue)](rust/README.md)
[![License](https://img.shields.io/badge/license-MPL--2.0-green)](LICENSE)

```rust
let text = base91_jdp::encode(b"{\"user\":\"ada\",\"id\":42,\"role\":\"admin\"}");
// --EA{$user$:$ada$,$id$:42,$role$:$admin$}       41 characters for 37 bytes

let text = base91_jdp::encode_smart(&big_json, 3)?;   // 0.34 characters per byte
```

Both go into a JSON string verbatim. No escaping, no `\"`, no `\\`, nothing
that can break the document they sit in — not as a property that was tested for
but as a property of the alphabet, which contains none of the characters a JSON
string has to escape. **The encoded size is the final size.**

---

## What it is

[basE91](http://base91.sourceforge.net/) (Joachim Henke, 2005) is the densest
widely-implemented binary-to-text encoding that stays in printable ASCII. Its
91-character alphabet leaves out `\` and `'` — but not `"`, so its output has to
be escaped inside a JSON string, and the density it gained on paper it gives
back in the file.

base91-jdp makes one substitution: **`"` leaves the alphabet and `-` takes its
place.** That decides everything else. `-` lands on the alphabet's last value,
90, so the pair `--` is worth 8 280 — above anything a thirteen-bit symbol can
spell. Symbols are fixed at thirteen bits where basE91 lets them float between
thirteen and fourteen, which leaves **eighty-nine pair values no encoded stream
can contain**. Those eighty-nine are the entire signalling mechanism.

A **typed segment** says what kind of bytes it carries, and the format writes
that kind at the density it deserves:

| What the bytes are | Written as | Characters per byte |
|---|---|---|
| a run of one repeated byte | its length | 0.03 and below |
| hex, decimal digits | 4 bits each | 0.62 |
| base32, letters, hex with separators | 5 bits each | 0.77 |
| base64, base64url | 6 bits each | 0.92 |
| text the alphabet can carry | one character each | 1.00 |
| a zstd frame, or anything else | the block coder | 1.23 |

A stream that wants none of it pays nothing: no signal, no header, no padding,
no terminator.

## Where it stands

Measured by the prototype, encoding each corpus and decoding it again.

**With a compressor, against the same compressor:**

| | Base64 + zstd | Base85N + zstd | **base91-jdp** |
|---|---|---|---|
| core corpus, level 3 | 0.37304 | 0.34954 | **0.34445** |
| Silesia, level 3 | 0.41824 | 0.39205 | **0.38607** |

1.5 % smaller than the best alternative, on all 25 files of both corpora, at
every level — and it is one self-describing format rather than a pipeline of
two.

**Without a compressor on either side:**

| | Base85N | **base91-jdp** |
|---|---|---|
| core corpus | 1.00698 | **0.98354** |
| Silesia | 1.05114 | **1.03792** |

**On payloads too short for a compressor to have a window**, which is where a
field in a JSON document lives:

| | Base64 | **base91-jdp** |
|---|---|---|
| 55 samples under 200 bytes | 1.3709 | **0.9252** |

Hex digests are 50 % smaller than Base64, decimal identifiers 47 %, UUIDs 37 %,
protocol text 21 %. Compressing these samples instead costs 1.2713 — worse than
Base64 — which is the measurement that says the classes are not redundant with
compression.

## Head to head with Base85N

Both implementations built from source and run in one process under one timing
loop, so the comparison is of two encodings and not of two languages
([`rust/examples/headtohead.rs`](rust/examples/headtohead.rs), specification
Section 17.21). Base85N has no compressor, so the two configurations are
reported separately and should not be read as one.

| core corpus | size | encode | decode |
|---|---|---|---|
| **Base85N** | 1.00698 | **401 MB/s** | **1 047 MB/s** |
| base91-jdp, no compressor | **0.98354** | 68 MB/s | 381 MB/s |
| base91-jdp, zstd −5 | **0.52271** | **457 MB/s** | 351 MB/s |
| base91-jdp, zstd 9 | **0.31449** | 54 MB/s | 520 MB/s |

**Smaller in every configuration** — 2.3 % on the core corpus and 1.3 % on
Silesia with no compressor on either side, 14.2 % on field-length payloads, and
41 % to 69 % with one. **At level −5, 48 % smaller and 14 % faster at the same
time**, which is the setting a caller picks when they want speed.

**Slower in two places, and both are real.** Without a compressor this encoder
is six times slower, because the candidate scan runs over every byte no class
claims. And it decodes two to three times slower in every configuration; 91
characters give a byte-oriented decoder a harder job than 85, and that is the
price of the density rather than a bug left to fix.

**Two things Base85N does that this format does not.** Its alphabet avoids the
delimiters of several text formats, where this one is dense inside a JSON
string and nothing more: `<`, `&` and `,` are alphabet characters here, so XML,
HTML and CSV are Base85N's use and not this one's. And a compressed payload is
opaque — passthrough leaves text legible on the uncompressed path, but every
figure above that beats Base85N by more than 2 % is a stream nothing can be
read out of without decoding it.

## The repository

| | |
|---|---|
| [`spec/`](spec/README.md) | The current specification, v0.4.0. Superseded versions in [`spec/history/`](spec/history/README.md). |
| [`rust/`](rust/README.md) | The implementation: encoder, decoder, every class, parallel encoding, an optional vector path. |
| [`bench/`](bench/README.md) | The three corpora and the Base85N reference harness. The numbers are in Section 17 of the specification. |
| [`history/`](history/README.md) | The v0.3.0 JavaScript implementation and the projections a prototype replaced. Not maintained. |
| [`site/`](site/README.md) | The website generator. It has no content of its own. |

```sh
python3 bench/corpus.py --core                            # fetch the corpus
cargo test  --manifest-path rust/Cargo.toml
cargo run --release --manifest-path rust/Cargo.toml --example corpus -- bench/corpus
```

## Status, plainly

**Draft, and not deployed anywhere.** The wire format is complete and
implemented, every class round-trips, and the parallel encoder is byte-identical
to the serial one. What it has not had is a second reader: Section 20 of the
specification says which parts would most repay one, and Section 17 says which
numbers are measured and which are still arguments.

Three things are open and named as open: the donor profiles are still v0.3.0's
and were derived for a different R-Set, the candidate ranking is greedy and a
JWT shows it, and there is no C implementation, so no speed claim is made
against another codec's.

**v0.3.0 was a different format** — head-of-stream mode markers, LZ4, and
Reed-Solomon over GF(2¹³) — and the two do not interoperate. It is complete,
tested and kept in [`history/`](history/javascript-v0.3.0/README.md); the
reasoning for each thing that went is in Sections 18.1, 18.4 and 18.5 of the
current specification.

## Credit

* **basE91** — Joachim Henke, 2005. The alphabet and the pair coding are his;
  the fixed thirteen-bit symbol is the one departure, and it is what the rest
  of the format is built on.
* **[Base85N](https://base85n.ghadami.de/)** — the Dynamic Passthrough idea, the
  R-Set and donor-profile mechanism, both benchmark corpora, the website
  generator, and the habit of measuring rather than asserting.
* **Zstandard** — Yann Collet. Class 17 carries an unmodified zstd frame.

## License

Mozilla Public License 2.0. See [LICENSE](LICENSE).

---

*Parts of this repository were drafted with AI assistance and then verified
against measurements. Every number here comes from a run of the benchmarks in
this repository.*
