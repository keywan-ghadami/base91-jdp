# Base91z

**basE91 on an alphabet JSON never has to escape, with typed segments and zstd
inside.**

[![Spec](https://img.shields.io/badge/spec-v0.4.0%20draft-orange)](spec/base91z-v0.4.0.md)
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

Thirteen files, 6.52 MB. Both codecs built from source and run in one process
under one timing loop, so what is compared is encodings and not languages
([`rust/examples/against.rs`](rust/examples/against.rs)). The second column is
what the string costs **inside a JSON document**, which is where these strings
go.

### No compressor

| | chars/byte | in a JSON string | encode |
|---|---|---|---|
| Base64 | 1.33333 | same | — |
| classic basE91 | 1.21517 | 1.23996 | 394 MB/s |
| Base85N | 1.00698 | same | 483 MB/s |
| **Base91z**, container only | **0.98354** | same | 76 MB/s |

### With a compressor

Base64, basE91 and Base85N have none, so those rows are what a caller has to
build. Base91z has one, and it is the same zstd.

| | chars/byte | in a JSON string | encode |
|---|---|---|---|
| deflate → Base64 | 0.36127 | same | — |
| deflate → basE91 | 0.33323 | 0.33598 | 27 MB/s |
| zstd 1 → basE91 | 0.37383 | 0.37706 | 315 MB/s |
| zstd 1 → Base85N | 0.37992 | same | 364 MB/s |
| **Base91z**, zstd 1 | **0.37431** | same | **388 MB/s** |
| zstd 3 → basE91 | 0.34344 | 0.34634 | 220 MB/s |
| zstd 3 → Base85N | 0.34897 | same | 250 MB/s |
| **Base91z**, zstd 3 | **0.34443** | same | **290 MB/s** |
| zstd 9 → basE91 | 0.31325 | 0.31595 | 62 MB/s |
| zstd 9 → Base85N | 0.31830 | same | 62 MB/s |
| **Base91z**, zstd 9 | **0.31449** | same | **64 MB/s** |

**Smallest in the JSON column at every compressor setting, and fastest at each
one.** Classic basE91 is very slightly smaller as a raw string — its symbol
floats between thirteen and fourteen bits where Base91z fixes it at thirteen —
and gives that back and more the moment the string enters the document it was
made for. `deflate → basE91` is the one row that is smaller in JSON than
Base91z at zstd 3; it runs at 27 MB/s against 290, and Base91z at zstd 9 is
smaller than it.

On payloads too short for a compressor to have a window, which is where a field
in a JSON document lives, no compressor is used at all and the typed classes
carry it: over 55 field samples under 200 bytes, **0.925 against Base64's
1.371** — hex digests 50 % smaller than Base64, decimal identifiers 47 %,
UUIDs 37 %.

Decoding is where Base91z is behind: 584 MB/s against Base85N's 1 331 on the
same corpus. Ninety-one characters are a harder job for a byte-oriented decoder
than eighty-five.

## The neighbourhood

| | |
|---|---|
| **[basE91](http://base91.sourceforge.net/)** (Henke, 2005) | What this is built on. Denser than Base64 and widely implemented, but its alphabet contains `"`, so its output has to be escaped inside a JSON string — the 1.21517 above becomes 1.23996. Reading and writing it is 30 lines either way, and [`rust/examples/against.rs`](rust/examples/against.rs) contains a complete encoder if you want one to read. |
| **[Base94](https://vorakl.com/articles/base94/)** | The whole printable-ASCII alphabet, and therefore the densest ASCII encoding possible at 1.221 characters a byte. It buys that by treating the file as one big number, which is quadratic: usable for tens of kilobytes, not for a megabyte. It also contains `"`, `\`, `<`, `&` and `,`, so it is safe in nothing. |
| **[Base122](https://blog.kevinalbs.com/base122)** (Albertson) | Not ASCII: it encodes into UTF-8 and is about 14 % smaller than Base64 *as bytes*. Its author does not recommend it for web pages, and control characters in it do not survive copy-paste. Where a length check, a regular expression or a terminal is in the path, a non-ASCII string is a different kind of risk from a large one. |
| **[Ascii85](https://en.wikipedia.org/wiki/Ascii85)** | 1.25 characters a byte and long deployed in PostScript and PDF. Its alphabet contains `"` and `\`. |

## The repository

| | |
|---|---|
| [`spec/`](spec/README.md) | The current specification, v0.4.0. Superseded versions in [`spec/history/`](spec/history/README.md). |
| [`rust/`](rust/README.md) | The implementation: encoder, decoder, every class, parallel encoding, an optional vector path. |
| [`bench/`](bench/README.md) | How the three corpora are fetched. The numbers are in Section 17 of the specification. |
| [`history/`](history/README.md) | The v0.3.0 JavaScript implementation and the projections a prototype replaced. Not maintained. |
| [`site/`](site/README.md) | The website generator. It has no content of its own. |

```sh
bench/fetch.sh                                           # fetch the corpus
cargo test  --manifest-path rust/Cargo.toml
cargo run --release --manifest-path rust/Cargo.toml --example corpus -- bench/corpus
```

## Status, plainly

**Draft, and not deployed anywhere.** The wire format is complete and
implemented, every class round-trips, and the parallel encoder is byte-identical
to the serial one. What it has not had is a second reader: Section 20 of the
specification says which parts would most repay one, and Section 17 says which
numbers are measured and which are still arguments.

Three things are open and named as open: the donor profiles were derived for a
different R-Set than the one in use, the candidate ranking is greedy and a JWT
shows it, and there is no C implementation.

Earlier drafts of this format were called base91-jdp, and v0.3.0 of it was a
different design that does not interoperate with this one. It is kept in
[`history/`](history/javascript-v0.3.0/README.md).

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
