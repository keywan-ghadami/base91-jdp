# base91-jdp

**basE91 with a JSON-safe alphabet, LZ4 inside, and error correction that costs
0.1 %.**

[![Spec](https://img.shields.io/badge/spec-v0.3.0%20draft-yellow)](spec/history/base91-jdp-v0.3.0.md)
[![Next](https://img.shields.io/badge/next-v0.4.0%20draft-orange)](spec/base91-jdp-v0.4.0.md)
[![License](https://img.shields.io/badge/license-MPL--2.0-green)](LICENSE)

```js
import { encode, encodeText, decode } from 'base91-jdp';

encodeText('{"user":"ada","id":42,"role":"admin"}');
// --EA{$user$:$ada$,$id$:42,$role$:$admin$}      41 characters for 37 bytes

encode(bigJsonArray);            // 17,181 bytes -> 4,522 characters
// }-jB4X7mFz<0GYWxMi:mFQV0...   LZ4 inside, 0.263 characters per byte
```

Both go into a JSON string verbatim. No escaping, no `\"`, no `\\`, nothing
that can break the document they sit in. Small payloads stay legible, because
`$` is standing in for the quotation mark and everything else is itself; large
ones get compressed, and the encoder decides which by measuring both.

---

## What it is

[basE91](http://base91.sourceforge.net/) (Joachim Henke, 2005) is the densest
widely-implemented binary-to-text encoding that stays in printable ASCII. Its
91-character alphabet leaves out `\` and `'` -- but not `"`, so its output has
to be escaped inside a JSON string, and the density it gained on paper it gives
back in the file.

base91-jdp makes one substitution: **`"` leaves the alphabet and `-` takes its
place.** With `"`, `\` and `'` all absent, and no character below `0x20`, the
alphabet is disjoint from everything a JSON string has to escape. The encoded
size *is* the final size.

That substitution decides everything else. `-` lands on the alphabet's last
value, 90, so the pair `--` is worth 8 280 -- above anything a thirteen-bit
symbol can spell. base91-jdp fixes symbols at thirteen bits where basE91 lets
them float between thirteen and fourteen, which leaves **eighty-nine pair
values no encoded stream can contain**. Those eighty-nine carry everything the
format says about itself:

* **`--` opens and closes a passthrough segment**, in which text is written one
  character per byte instead of being expanded 1.23x;
* **the eighty-eight below it are mode markers**, two characters at the head of
  a stream saying it carries LZ4, error correction, or both.

A stream that wants neither pays nothing at all: no marker, no header, no
padding.

### Passthrough

The seven byte values real text is full of and the alphabet does not contain --
space, `"`, newline, `\`, carriage return, `'`, tab -- are written as stand-ins
borrowed from the alphabet's rarest characters, named per segment by a
two-character header. There is no escape mechanism and no escape character; a
segment either carries a byte or it does not.

`-` is carried the same way, and that is worth its own sentence: it is the one
stand-in that is *in* the alphabet, substituted not because it cannot be written
but because two in a row would end the segment. So a payload never contains `-`
at all, the exit signal cannot collide with anything, and text dense in `--`
costs one stand-in per segment rather than a mode switch per occurrence:

```js
encodeText('--bs-blue: #0d6efd; --bs-indigo: #6610f2; --bs-purple: #6f42c1;');
// --<C~~bs~blue:$#0d6efd;$~~bs~indigo:$#6610f2;$~~bs~purple:$#6f42c1;
//        67 characters for 63 bytes -- `~` stands in for `-`, `$` for the space
```

When a byte cannot be carried -- a UTF-8 continuation byte, a byte of a JPEG --
the segment ends and the bytes go through the block coder until passthrough is
worth resuming. That is the **binary fallback**, and it is why the format
handles mixed content without being told which is which.

### The marker

Detection is total, not probabilistic. A headerless stream *cannot* begin with
a marker, because no packer can write those values -- there is no escape clause
and no exclusion rule anywhere in the encoder. That is what the fixed
thirteen-bit symbol is bought for.

Every marker's second character is `-`, and classic basE91 cannot produce `-`
at all, so a `-` in second place also answers "is this base91-jdp or is it
classic basE91?". Classic basE91 needs no flag to stay out of band.

| marker | compression | error correction |
|---|---|---|
| none | none | none |
| `~-` | none | Reed-Solomon |
| `}-` | LZ4 | Reed-Solomon |
| `\|-` | none | check pattern only |
| `{-` | LZ4 | check pattern only |

### Error correction, and a check that costs nothing

Reed-Solomon over GF(2^13) repairs **two damaged symbols per 4 096-symbol
codeword for 0.098 %** of the stream. The field is the point: the channel
damages *characters*, and one pair is exactly one GF(2^13) symbol, so one
damaged character is one damaged symbol. Byte-level parity would need six
parity bytes and 2.4 % to say the same thing.

The eighty-eight free pair values do a second job inside the body. A symbol
that falls in a scattered window of eighty-eight values may be written as one
of them instead, which carries one bit **without moving a single character** --
2.3 % to 4.3 % of symbols on the corpus, 793 to 3 460 bits per segment. Those
bits hold a check pattern derived from each codeword's own contents, which is
what narrows the hole where Reed-Solomon, overwhelmed, lands on a different
valid codeword.

### The damage bound

Segments are 256 KiB of payload, divided by `--`, each with its own LZ4
dictionary. Because no packed symbol can spell `--`, a reader that has lost its
place finds the next separator and carries on -- so segment boundaries are not
a chain, and there is no length field anywhere that one damaged symbol could
take out.

* One to eight flipped bits in a protected stream: **repaired**, exactly, in
  240 of 240 trials over 3 MiB.
* Damage that overwhelms a codeword: **one segment**.
* A separator destroyed outright: **two**, because they merge.
* Nothing costs a third. Measured over 1 200 trials with bursts of 4 to 4 096
  characters: worst case 512 KiB of payload, and not one run returned altered
  bytes without saying a segment was lost.

## Where it wins, and where it does not

Measured on Base85N's benchmark corpus, unchanged: 6.52 MB of real files.
Characters per input byte, once the output sits in a JSON string. Full tables
and method: [`bench/results/RESULTS.md`](bench/results/RESULTS.md).

| | Base64 | basE91 | [Base85N](https://base85n.ghadami.de/) | Base64 +deflate | Base85N +deflate | **base91-jdp** |
|---|---|---|---|---|---|---|
| text files | 1.333 | 1.252 | 0.965 | 0.197 | **0.184** | 0.334 |
| binary files | 1.333 | 1.228 | 1.050 | 0.534 | **0.501** | 0.676 |
| whole corpus | 1.333 | 1.240 | 1.007 | 0.363 | **0.340** | 0.503 |

Three readings, and they say different things.

**Against the plain binary-to-text codecs it is twice as good.** 0.503 against
Base85N's 1.007 and Base64's 1.333, because it carries a compressor and they do
not.

**Against deflate-then-encode it is 48 % worse.** That is the price of LZ4 over
deflate, and it is a deliberate one: a specification that demands LZ4 demands a
few hundred lines, and one that demands deflate demands a library. If you
already have zlib in the process and size is the only thing you care about,
deflate then Base85N is smaller. It is also two formats, two failure modes, no
error correction and no damage bound.

**On data that will not compress it is the only column that does not lose:**

| sample | Base64 +deflate | Base85N | base91-jdp |
|---|---|---|---|
| `grace_hopper.jpg` | 1.330 | 1.249 | **1.231** |
| `minduka_present.png` | 1.334 | 1.250 | **1.231** |

A deflate pipeline *expands* an already-compressed file past plain Base64.
base91-jdp builds both candidates, compares their exact sizes and keeps the
shorter, so it never does. That is also why there is no threshold to tune:

```
payload    32 B         64 B         128 B and up
text       headerless   headerless   framed
JSON       headerless   framed       framed
random     headerless   headerless   headerless      <- never framed, at any size
```

Short payloads keep the passthrough encoding, which is why the 37-byte example
at the top is still readable:

```
input     {"user":"ada","id":42,"role":"admin"}          37 bytes
Base64    eyJ1c2VyIjoiYWRhIiwiaWQiOjQyLCJyb2xlIjoiYWRtaW4ifQ==    52
Base85N   %nU$w{~user~:~ada~^~id~:42^~role~:~admin~}              42
base91-jdp  --EA{$user$:$ada$,$id$:42,$role$:$admin$}             41
```

### So which should you use?

* Output goes into **XML, HTML or an SVG attribute** -> not this. `<`, `>` and
  `&` are all in this alphabet. Use [Base85N](https://base85n.ghadami.de/),
  whose alphabet contains none of them.
* **Smallest possible output, zlib already in the process, nothing else
  matters** -> deflate, then Base85N. 0.340 against this format's 0.503.
* **One tool, no dependency, and a stream that says what it is** ->
  base91-jdp. Twice as small as any plain encoding, and the decoder needs
  nothing but the decoder.
* **The stream may be damaged and you need to know, or need it repaired** ->
  base91-jdp with `protect`. One flipped bit is repaired; worse damage costs a
  bounded piece and is reported. No other option here offers this at 0.1 %.
* Payload is **incompressible binary in JSON** -- a key, a hash, a thumbnail, a
  media file -> base91-jdp, by 1.2 % to 8 % against everything else.
* Payload is a **short JSON or text field in a JSON document** -> base91-jdp,
  and it stays legible.

## Install

```bash
npm install base91-jdp
```

Node 18.11 or newer, ESM only, zero dependencies. The core runs unchanged in
browsers, Deno and Bun.

## API

| Function | Description |
|---|---|
| `encode(bytes, options?): string` | Encode binary data. |
| `decode(text, options?): Uint8Array` | Decode; whitespace in the input is skipped. |
| `decodeDetailed(text)` | Decode and report the mode, the segments, what was repaired and what was lost. |
| `encodeText(text, options?): string` | Encode a string as UTF-8. |
| `decodeText(text, options?): string` | Decode to a string; throws on invalid UTF-8. |
| `ALPHABET: string` | The 91 characters, in value order. |
| `MODES` | The mode markers and what each one means. |
| `CONSTANTS` | The frozen constants of the specification. |
| `makeCodec(config)` | The parameterised core, for experiments. |

### Options

```js
encode(bytes, {
  compress: 'auto',   // 'auto' | 'never' | 'always'
  protect: 'auto',    // 'auto' | 'check' | true | false
});
```

`compress: 'auto'` builds the LZ4 candidate, works out its exact size, and
takes it only if it is shorter than the alternative. There is no threshold to
tune and no size below which it gives up guessing.

`protect` answers two questions that are easy to run together and must not be
-- whether error correction is wanted, and whether a frame is wanted at all:

| value | error correction | check pattern | frame |
|---|---|---|---|
| `'auto'` | once it is close to free | when framed | if it is smaller |
| `true` | yes | yes | always |
| `'check'` | no | yes | always |
| `false` | no | when framed | if it is smaller |

`'check'` is the useful one for data that will not compress: damage is
reported rather than repaired, and it costs no characters at all.

### Decoding a stream that may be damaged

```js
import { decode, decodeDetailed } from 'base91-jdp';

decode(text);                      // throws DAMAGED_SEGMENT if anything was lost
decode(text, { partial: true });   // returns the segments that survived

const seen = decodeDetailed(text);
seen.mode;      // 'lz4', 'stored', 'lz4Checked', 'storedChecked', or undefined
seen.repaired;  // symbols error correction put back
seen.damaged;   // the segments it could not, with a reason each
```

Everything throws `Base91JdpError` on malformed input, with a `code` from
`ERR`. There is one error type whichever layer refused: `INVALID_CHARACTER`,
`UNEXPECTED_EOS`, `UNDEFINED_SIGNAL`, `INVALID_FLUSH`, `INVALID_FINAL_BLOCK`,
`RESERVED_PAIR`, `UNKNOWN_MODE`, `EXTENDED_HEADER`, `MALFORMED_FRAME`,
`MALFORMED_PAIRS`, `DAMAGED_SEGMENT`.

```js
import { encodeText, decodeText } from 'base91-jdp';

const encoded = encodeText('Grüße aus München');
JSON.stringify({ note: encoded });   // needs no escaping, whatever the input was
decodeText(encoded);                 // 'Grüße aus München'
```

## Command line

```bash
base91jdp photo.jpg > photo.b91              # encode; LZ4 if it helps
base91jdp -d photo.b91 > photo.jpg           # decode
base91jdp --protect yes backup.tar > b.b91   # add error correction
base91jdp -d --partial --verbose b.b91       # salvage what survived
base91jdp -w 100 dump.bin                    # wrap at 100 characters
```

Input is treated as raw bytes; output carries no trailing newline. Whitespace is
skipped on decode, so wrapped output decodes without preprocessing. A damaged
stream is a non-zero exit unless `--partial` is given, because returning short
output silently is the one thing a decoder must not do.

## Compressing outside the format

`encode` already tries LZ4 and keeps it only when it is smaller, so there is
nothing to do for the ordinary case. Two situations are worth knowing about.

**You need it smaller and you have zlib.** Deflate compresses better than LZ4 by
a wide margin. Deflate first and the format will notice it cannot improve on
what you handed it:

```js
import { deflateRawSync } from 'node:zlib';
import { encode } from 'base91-jdp';

encode(deflateRawSync(payload, { level: 6 }));   // 0.332 on the corpus
encode(payload);                                 // 0.503
```

The cost is that the reader now needs zlib too, and a stream that no longer
says what it is.

**Your data is already compressed.** A JPEG, a zip, a video, a key: hand it
over as it is. LZ4 will be tried, will not help, and the headerless encoding
wins on size -- 1.231 characters per byte, which is better than anything else
here and better than deflating it first, which would expand it.

## Safety properties

* **Nothing it emits needs escaping in JSON.** Not a design goal met by
  testing -- a property of the alphabet, which contains no `"`, no `\` and no
  character below `0x20`.
* **A headerless stream cannot expand.** Passthrough is exactly 1:1 and a
  symbol yields at most two bytes.
* **A framed stream can**, because it carries a compressor -- a megabyte of
  zeros is under 6 000 characters, and reading it back produces a megabyte. A
  decoder bounds this by the segment size, which is fixed, times a segment
  count the input bounds.
* **The check pattern is not a MAC.** It detects accident. Anyone who can
  rewrite the stream can rewrite the pattern with it.
* **Malformed input throws**, with a code, and never reads outside its buffer.
* **A damaged segment is never returned as though it were sound.** Either the
  decode fails or the loss is reported.

## Specification

[`spec/history/base91-jdp-v0.3.0.md`](spec/history/base91-jdp-v0.3.0.md) defines the format
completely: the alphabet, the fixed thirteen-bit symbol and the eighty-nine
values it frees, the marker and the modes, passthrough and its header, the
framed body and its damage bound, the side channel, error correction, the LZ4
block format, canonicity, error handling, and the measurements behind every
constant.

Status is **draft**. The format is complete, implemented and measured, but it
has not been in the field. Eighty-three markers are unassigned and one is
reserved to say "a longer header follows", so there is room without anything
being spent on it.

**A successor is in draft.** [`spec/base91-jdp-v0.4.0.md`](spec/base91-jdp-v0.4.0.md)
replaces the head-of-stream mode markers with typed segments, LZ4 with zstd,
and the error correction with a run of classes that measure better on both
benchmark corpora. It is not implemented here yet, and everything this README
describes is 0.3.0, which is what the code in `src/` does.

## Tests and benchmarks

```bash
npm test                    # 76 tests: round trip, adversarial decode, damage bound
python3 bench/corpus.py     # fetch the benchmark corpus (pinned, verified)
npm run bench               # the size tables
npm run bench:pipeline      # modes, side channel, throughput per layer
npm run bench:sweep         # the parameter sweeps
npm run bench:rs            # the Reed-Solomon study
```

The Base85N columns of the benchmark run the upstream Go implementation
(v0.5.1) and need Go on the path; without it those columns are left out rather
than filled in from documentation. The LZ4 fixtures in `test/lz4-fixtures.js`
were produced by upstream liblz4 and check that `src/lz4.js` speaks the block
format rather than a private dialect of it.

## Credit

* **basE91** -- Joachim Henke, 2005. The alphabet and the pair coding are his;
  the fixed thirteen-bit symbol is the one departure, and it is what the rest
  of the format is built on.
* **LZ4** -- Yann Collet. `src/lz4.js` implements the block format from its
  specification, with no code taken from the reference implementation.
* **[Base85N](https://base85n.ghadami.de/)** — the Dynamic Passthrough idea, the
  R-Set and donor-profile mechanism, the benchmark corpus, and the habit of
  measuring rather than asserting all come from it.
* **[base94max](https://github.com/keywan-ghadami/base94max)** — the measurements
  that made the case for a JSON-safe basE91 in the first place.

## License

Mozilla Public License 2.0. See [LICENSE](LICENSE).

---

*Parts of this repository were drafted with AI assistance and then verified
against measurements. Every number in the README and in RESULTS.md comes from a
run of the benchmark in this repository.*
