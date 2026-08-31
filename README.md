# Base91z

**basE91 on an alphabet JSON never has to escape, with typed segments and zstd
inside.**

[![Spec](https://img.shields.io/badge/spec-v0.4.0%20final-green)](spec/base91z-v0.4.0.md)
[![Implementation](https://img.shields.io/badge/implementation-Rust%20prototype-blue)](rust/README.md)
[![License](https://img.shields.io/badge/license-MPL--2.0-green)](LICENSE)

```rust
let text = base91z::encode(b"{\"user\":\"ada\",\"id\":42,\"role\":\"admin\"}");
// C-CAl{$user$:$ada$,$id$:42,$role$:$admin$}      42 characters for 37 bytes

let text = base91z::encode(&big_json);             // 0.37 characters per byte
```

One entry point. It compresses where compression pays and carries the payload
with a typed class where it does not, and the same `decode` reads either.

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

Base91z makes one substitution: **`"` leaves the alphabet and `-` takes its
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

## Which one you want

| | |
|---|---|
| **[Base85N](https://base85n.ghadami.de/)** | Enterprise-grade where compatibility is the requirement. Its 85-character alphabet is safe in JSON, XML, HTML *and* CSV; its passthrough leaves text legible in the encoded stream; it is the fastest of these to encode and to decode. Take it when the output has to survive several formats, be read by a person, or go through tooling you do not control. |
| **Base91z** | Take it when **size is the requirement** and the destination is JSON. Compression is inside the format rather than a stage in front of it, and it is smaller than every pipeline below at the same compressor setting. |
| **Base64** | Take it when you need no argument. Everything reads it. It costs a third of the payload again, forever. |

Base91z is not a competitor to Base85N for XML, HTML or CSV: `<`, `&` and `,`
are Base91z alphabet characters. It is denser because it spends six characters
Base85N deliberately does not.

## Against the alternatives

Thirteen files, 6.52 MB. Every codec is built from source and run in one
process under one timing loop, so what is compared is encodings and not
languages — measured by
[binary2textbench](https://github.com/keywan-ghadami/binary2textbench), which
does this for six encodings and puts the JSON escaping inside the clock.

Two things to read the tables with. **The second size column is what the string
costs inside a JSON document**, which is where these strings go. And **speed is
given against Base64 rather than in MB/s**: these runs happen on shared cloud
machines where an absolute figure says as much about the neighbour as about the
codec, so each codec is timed bracketed between two Base64 readings and the
quotient is reported. It is a *time* ratio — 0.9 is ten per cent faster than
Base64, 1.7 is seventy per cent slower — and the spread beside it is the
interquartile range across rounds. **A gap smaller than the spread is not a
result.**

### No compressor in front of anybody

Base91z's compressor is part of the format rather than a stage a caller bolts
on, so this is the row it ships with; the others have none.

| | chars/byte | in a JSON string | encode | decode |
|---|---|---|---|---|
| Base64 | 1.33333 | same | 1.000 | 1.000 |
| classic basE91 | 1.21517 | 1.23996 | 1.704 ± 0.045 | 3.119 ± 0.092 |
| Base94Max | 1.20228 | 1.23639 | 1.801 ± 0.044 | 4.145 ± 0.082 |
| Ascii85 | 1.18812 | 1.21326 | **0.875 ± 0.030** | 3.016 ± 0.074 |
| Base85N | 1.00698 | same | 1.064 ± 0.027 | **0.991 ± 0.017** |
| **Base91z** | **0.37432** | same | 1.039 ± 0.031 | 1.635 ± 0.030 |

Take the compressor away and the container alone encodes the same corpus to
**0.98354** — still under Base85N's 1.00698, and reproducible here with
`cargo run --release --example corpus -- bench/corpus`.

### With the same compressor in front of everybody

zstd at level 1, which is what `encode` uses by default. The other five get it
as a stage in front; Base91z is told to make its own decision at that level.

| | chars/byte | in a JSON string | encode | decode |
|---|---|---|---|---|
| Base64 | 0.40536 | same | 1.000 | 1.000 |
| Ascii85 | 0.38002 | 0.38901 | 0.988 ± 0.010 | 1.580 ± 0.100 |
| Base85N | 0.37992 | same | 0.914 ± 0.004 | **0.931 ± 0.004** |
| **Base91z** | 0.37432 | **0.37432** | **0.859 ± 0.137** | 1.391 ± 0.134 |
| classic basE91 | **0.37383** | 0.37706 | 1.140 ± 0.023 | 1.415 ± 0.052 |
| Base94Max | **0.37184** | 0.37891 | 1.185 ± 0.026 | 1.763 ± 0.111 |

**Smallest in the JSON column**, which is the column that decides the file that
ships. Classic basE91 and Base94Max are smaller as raw strings — their symbols
are wider than Base91z's fixed thirteen bits — and give it back the moment the
string enters the document it was made for. On encode Base91z and Base85N are
**tied**: the 0.055 between them is inside Base91z's own 0.137 of spread, and
b2tb's rule is that a gap inside the spread is not an ordering.

At level −5 the picture is the same shape: Base91z 0.52273 against Base85N's
0.50479, and encode 0.786 ± 0.099 against 0.977 ± 0.013. At levels 9 and 19 the
compressor is nearly all of the work for everybody and every codec's figure
collapses towards 1.0, with a spread that swamps the differences — the honest
reading there is that any throughput claim about a compressing encoder is a
claim about zstd. The one exception is an already-compressed payload, where
Base91z declines to compress at all and the image category comes out at 0.24 of
Base64's time at level 9 and 0.017 at level 19.

**Decoding is where Base91z is behind**: 1.391 against Base85N's 0.931 at level
1, and 1.635 against 0.991 with no compressor. Ninety-one characters are a
harder job for a byte-oriented decoder than eighty-five.

On payloads too short for a compressor to have a window, which is where a field
in a JSON document lives, no compressor is used at all and the typed classes
carry it: over 55 field samples under 200 bytes, **0.9252 against Base64's
1.3709** — hex digests 50 % smaller than Base64, decimal identifiers 47 %,
UUIDs 37 %.

## The neighbourhood

| | |
|---|---|
| **[basE91](http://base91.sourceforge.net/)** (Henke, 2005) | What this is built on. Denser than Base64 and widely implemented, but its alphabet contains `"`, so its output has to be escaped inside a JSON string — the 1.21517 above becomes 1.23996. Reading and writing it is 30 lines either way, and [binary2textbench](https://github.com/keywan-ghadami/binary2textbench/blob/main/runner-rust/src/codecs.rs) contains a complete encoder if you want one to read. |
| **[Base94](https://vorakl.com/articles/base94/)** | The whole printable-ASCII alphabet, and therefore the densest ASCII encoding possible at 1.221 characters a byte. It buys that by treating the file as one big number, which is quadratic: usable for tens of kilobytes, not for a megabyte. It also contains `"`, `\`, `<`, `&` and `,`, so it is safe in nothing. |
| **[Base122](https://blog.kevinalbs.com/base122)** (Albertson) | Not ASCII: it encodes into UTF-8 and is about 14 % smaller than Base64 *as bytes*. Its author does not recommend it for web pages, and control characters in it do not survive copy-paste. Where a length check, a regular expression or a terminal is in the path, a non-ASCII string is a different kind of risk from a large one. |
| **[Ascii85](https://en.wikipedia.org/wiki/Ascii85)** | 1.25 characters a byte and long deployed in PostScript and PDF. Its alphabet contains `"` and `\`. |

## The repository

| | |
|---|---|
| [`spec/`](spec/README.md) | The current specification, v0.4.0. Superseded versions in [`spec/history/`](spec/history/README.md). |
| [`rust/`](rust/README.md) | The implementation: encoder, decoder, every class, parallel encoding, an optional vector path. |
| [`bench/`](bench/README.md) | How the three corpora are fetched. The numbers are in Section 17 of the specification. |
| [`site/`](site/README.md) | The website generator. It has no content of its own. |
| [`SECURITY.md`](SECURITY.md) | The threat model, what is run against the decoder, and how to report something. |

```sh
bench/fetch.sh                                           # fetch the corpus
cargo test  --manifest-path rust/Cargo.toml
cargo run --release --manifest-path rust/Cargo.toml --example corpus -- bench/corpus
```

## Status, plainly

**The specification is final; the implementation is a prototype, deployed
nowhere.** The wire format is fixed — a stream encoded against v0.4.0 stays
readable, and a change that would break one is a new version — and it is
implemented, every class round-trips, and the parallel encoder is byte-identical
to the serial one. What it has not had is a second reader: Section 20 of the
specification says which parts would most repay one, and Section 17 says which
numbers are measured and which are still arguments. Neither the document being
final nor the numbers being measured makes that reading less worth having.

The crate is packaged to publish and has not been published. What is run
against it -- an adversarial decode suite, five fuzz targets, Miri over the
raw-pointer paths, advisory and licence checks, and a lint that will not let an
`unsafe` block through without a `SAFETY` comment -- is in
[SECURITY.md](SECURITY.md), together with what has not been done.

Two things are open and named as open: the candidate ranking is greedy and a
JWT shows it, and there is no C implementation. A third was closed here — the
donor profiles had been derived for a different R-Set than the one in use, and
re-deriving them for the right one moved the table and moved no measurement
outside the corpus it was fitted on.

Earlier drafts of this format were called base91-jdp, and v0.3.0 of it was a
different design that does not interoperate with this one. Its specification is
in [`spec/history/`](spec/history/README.md); the JavaScript implementation that
went with it has been removed from the working tree and is reachable in the
repository's git history.

## Credit

* **basE91** — Joachim Henke, 2005. The alphabet and the pair coding are his;
  the fixed thirteen-bit symbol is the one departure, and it is what the rest
  of the format is built on.
* **[Base85N](https://base85n.ghadami.de/)** — the Dynamic Passthrough idea, the
  R-Set and donor-profile mechanism, both benchmark corpora, the website
  generator, and the habit of measuring rather than asserting.
* **Zstandard** — Yann Collet. Class 17 carries an unmodified zstd frame.

## License

Mozilla Public License 2.0. See [LICENSE](LICENSE). Provider identification,
contact and the terms the documents may be used under are in
[IMPRESSUM.md](IMPRESSUM.md).

---

*Parts of this repository were drafted with AI assistance and then verified
against measurements. Every number here comes from a run: this format's own
figures from the examples in [`rust/`](rust/README.md), the comparisons against
other codecs from [binary2textbench](https://github.com/keywan-ghadami/binary2textbench).*
