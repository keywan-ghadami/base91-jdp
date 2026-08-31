# Base91z

**A binary-to-text encoding: arbitrary bytes written as characters, so they can
travel inside a text protocol. When that protocol is JSON, this is the smallest
one there is.**

[![Website](https://img.shields.io/badge/website-base91z-1f6feb)](https://keywan-ghadami.github.io/base91z/)
[![Spec](https://img.shields.io/badge/spec-v0.4.0%20final-green)](spec/base91z-v0.4.0.md)
[![Implementation](https://img.shields.io/badge/implementation-Rust%20prototype-blue)](rust/README.md)
[![License](https://img.shields.io/badge/license-MPL--2.0-green)](LICENSE)

Text protocols carry text. An API request, a log line, a config file, a
database column — put a key, a thumbnail, a certificate or a compressed blob
into one and the bytes have to be written as characters first. **Base64** is
how that is almost always done, and it costs a third more size, on every byte,
forever.

Base91z does the same job and produces less:

```rust
let text = base91z::encode(b"{\"user\":\"ada\",\"id\":42,\"role\":\"admin\"}");
// C-CAl{$user$:$ada$,$id$:42,$role$:$admin$}      42 characters for 37 bytes

let text = base91z::encode(&big_json);             // 0.37 characters per byte
```

Against Base64's 1.33 characters a byte, on the same files. One entry point: it
compresses where compression pays and carries the payload with a typed class
where it does not, and the same `decode` reads either.

Both go into a JSON string verbatim. No escaping, no `\"`, no `\\`, nothing
that can break the document they sit in — not as a property that was tested for
but as a property of the alphabet, which contains none of the characters a JSON
string has to escape. **The encoded size is the final size.**

---

## What it is

A binary-to-text encoding has one job: turn bytes into characters that a text
format will carry unharmed, and turn them back. What separates one from another
is **which characters it is willing to use**, because that decides how many of
them a byte costs. Base64 uses 64 of them and costs 1.333 characters a byte.
Printable ASCII holds 94, and an encoding that spends more of them costs less:
the ones in the table below land around 1.2.

The catch is that the text format reserves some of those characters for itself.
JSON reserves `"` and `\`, and a string containing either grows: each one is
written as two. So an encoding that spends those characters is smaller in the
abstract and not in the file, which is the only place size is spent.

[basE91](http://base91.sourceforge.net/) (Joachim Henke, 2005) is the densest
widely-implemented encoding that stays in printable ASCII. Its 91-character
alphabet leaves out `\` and `'` — but not `"`, so its output has to be escaped
inside a JSON string, and the density it gained on paper it gives back in the
file.

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

## One format, not a pipeline

Everybody else puts binary into JSON the same way: **compress, then encode.**
`zstd(payload)` into `base64(...)` into the field. Two libraries, two decisions
for the caller, and two places to get it wrong. Base91z is not that pipeline
made shorter — the compressor is *inside* the format, and that changes four
things a pipeline cannot change:

* **It compresses only where compression pays.** A pipeline compresses
  unconditionally: it hands zstd a 40-byte identifier, or a JPEG, and pays a
  frame's overhead plus the encoding expansion for a payload that got no
  smaller — often larger. Base91z decides per input, and where a compressor has
  nothing to find it reaches for a typed class instead. On an already-compressed
  image at level 19 that decision alone is the difference between doing the
  work and not: it runs at 0.017 of Base64's time, because it declines.
* **The compressed payload is not a stranger to the encoder.** A pipeline's
  encoder is handed opaque bytes and can only spread them over its alphabet.
  Base91z knows the segment is a zstd frame, so it drops the eleven bytes of
  frame the format already accounts for — the magic number repeats the segment
  signal, the content size and the block header repeat the length field, and the
  checksum answers a question this format does not ask. Class 20 goes further
  and carries a bare compressed block.
* **Compression and typed classes are chosen against each other, per segment.**
  Not "compress the whole thing or none of it": a stream can carry a run, then a
  hex field at four bits a character, then a compressed segment, and pick each
  one because it measured shorter than block mode for those bytes.
* **The result needs no escaping.** A pipeline's output goes into a JSON string
  and the document escapes what the alphabet spent; Base91z's does not, because
  the alphabet has nothing to escape. That is a size difference the pipeline
  pays at the very last step, after all its work is done.

And for the caller it is one function:

```rust
let text = base91z::encode(&payload);   // infallible; there is always a valid encoding
let back = base91z::decode(&text)?;     // reads whatever encode chose
```

No compressor to pick, no level to thread through, no branch for "was it worth
compressing", no second dependency, and nothing to get out of step between the
two ends. `decode` reads every choice the encoder could have made.

## What that costs: the text is not a function of the bytes alone

The knob that makes this work is also the one thing a pipeline does better.
**The same payload can encode to different strings**, because the compression
level is a parameter and each level produces a different zstd frame:

```
40 000 bytes of the CommonMark spec, through base91z::encode_at
  level  -5    21 754 characters
  level   1    14 574          ← the default, what `encode` uses
  level   3    13 940
  level   9    12 814
  level  19    12 384
five levels, five different strings, all decoding to the same 40 000 bytes
```

```sh
cargo run --release --example levels -- bench/corpus/commonmark-spec.txt
```

Within one level it is deterministic, and deliberately so: specification
Section 11.3 fixes every choice the encoder makes, down to which class wins a
tie, so two conforming encoders agree character for character and the parallel
encoder is byte-identical to the serial one. What Section 11.3 does *not* fix is
the level, the zstd build underneath, or the class set an encoder chose to
implement — and while the format is young, a tuning table can move too.

So **do not treat the encoded text as an identity.** A signature over the
string, a cache key, a dedup key, an ETag, a fixture a test compares against —
all of these assume that the same bytes give the same characters forever, and
here they do not. Sign and hash the **payload**, before encoding or after
decoding, where the bytes are the bytes; `decode(encode(x)) == x` always holds,
which is the guarantee that actually carries.

**Base64 is the honest choice when the text itself has to be reproducible.**
It has no parameters, so the same bytes give the same string on every machine
and every version, forever — and Base85N and Ascii85 are stable the same way.
If a protocol signs the encoded form rather than the payload, that property is
worth more than the size, and this format does not offer it. `encode_plain`
takes the compressor out and is stable within a version, but "within a version"
is not the same promise.

## Which one you want

| | |
|---|---|
| **[Base85N](https://base85n.ghadami.de/)** | Enterprise-grade where compatibility is the requirement. Its 85-character alphabet is safe in JSON, XML, HTML *and* CSV; its passthrough leaves text legible in the encoded stream; it is the fastest of these to encode and to decode. Take it when the output has to survive several formats, be read by a person, or go through tooling you do not control. |
| **Base91z** | Take it when **size is the requirement** and the destination is JSON. Compression is inside the format rather than a stage in front of it, and it is smaller than every pipeline below at the same compressor setting. Not where the encoded text has to be byte-stable across levels and versions — see above. |
| **Base64** | Take it when you need no argument. Everything reads it, and the same bytes give the same string forever, which matters if you sign the string. It costs a third of the payload again. |

Base91z is not a competitor to Base85N for XML, HTML or CSV: `<`, `&` and `,`
are Base91z alphabet characters. It is denser because it spends six characters
Base85N deliberately does not.

## Against the alternatives

**The measurements live at [bench.ghadami.de](https://bench.ghadami.de).**
That is [binary2textbench](https://github.com/keywan-ghadami/binary2textbench):
six encodings, every one built from source and run in one process under one
timing loop, so what is compared is encodings and not languages — with the JSON
escaping inside the clock, because a string that has to be escaped costs what it
costs in the document and not on paper. The page has the ranking, size, speed,
every number per file and per category, and the provenance of each. It is kept
current there; this section says what it currently says, and does not copy the
tables, which would only go stale here.

**Size: smallest in the column that decides the file.** Against a corpus of
thirteen files, Base91z encodes to **0.374 characters a byte** where Base64 is
1.333 — and it is smallest of the six *inside a JSON string*, which is the only
place these strings are ever spent. Classic basE91 and Base94Max come out
slightly smaller as raw strings, because their symbols are wider than Base91z's
fixed thirteen bits, and give it back the moment the string enters the document
it was made for: their alphabets contain `"`.

**Speed: at the front on encode, behind on decode.** At zstd level 1 Base91z
encodes at 0.86 of Base64's time, the lowest of the six — though tied with
Base85N rather than ahead of it, since the gap between them is inside the
run-to-run spread, and b2tb's rule is that a gap inside the spread is not an
ordering. Decoding is where it is behind: 1.39
against Base85N's 0.93. Ninety-one characters are a harder job for a
byte-oriented decoder than eighty-five, and that is the trade the alphabet
buys.

**On short payloads it does not compress at all, and still wins.** Over 55
field samples under 200 bytes — where a compressor has no window, and where a
field in a JSON document actually lives — the typed classes carry it at
**0.925 against Base64's 1.371**: hex digests 50 % smaller than Base64, decimal
identifiers 47 %, UUIDs 37 %.

**Read every chars-per-byte figure as "on this corpus", not "per byte,
always".** Only Base64 is a constant, at 1.33333 whatever the bytes are. The
others all have a case that spends their alphabet differently and are therefore
measuring the input as much as the codec: classic basE91 takes thirteen bits
for a pair of characters but fourteen whenever those thirteen come to 88 or
less, so it costs 1.2297 on random bytes and approaches 8/7 on zero runs;
Ascii85 writes four zero bytes as the single character `z`; Base85N passes a
byte through as itself wherever its alphabet already holds it; and Base91z is a
compressor and a set of typed classes, which is the extreme of the same thing.
This is why a figure can sit below 8 / log₂ 91 = 1.2293 — the bound for a
*fixed* radix-91 code — without being an arithmetic error.

## The neighbourhood

| | |
|---|---|
| **[basE91](http://base91.sourceforge.net/)** (Henke, 2005) | What this is built on. Denser than Base64 and widely implemented, but its alphabet contains `"`, so its output has to be escaped inside a JSON string, and roughly two per cent of the density it gained goes back. Reading and writing it is 30 lines either way, and [binary2textbench](https://github.com/keywan-ghadami/binary2textbench/blob/main/runner-rust/src/codecs.rs) contains a complete encoder if you want one to read. |
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
