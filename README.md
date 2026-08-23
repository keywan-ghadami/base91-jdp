# base91-jdp

**basE91 with a JSON-safe alphabet, and a passthrough mode that carries text at
one character per byte.**

[![Spec](https://img.shields.io/badge/spec-v0.2.0%20draft-yellow)](spec/base91-jdp-v0.2.0.md)
[![License](https://img.shields.io/badge/license-MPL--2.0-green)](LICENSE)

```js
import { encode, decode } from 'base91-jdp';

encode(new TextEncoder().encode('{"user":"ada","id":42,"role":"admin"}'));
// --EA{$user$:$ada$,$id$:42,$role$:$admin$}      41 characters for 37 bytes
```

That output goes into a JSON string verbatim. No escaping, no `\"`, no `\\`,
nothing that can break the document it sits in — and the payload is still
legible, because `$` is standing in for the quotation mark and everything else
is itself.

---

## What it is

[basE91](http://base91.sourceforge.net/) (Joachim Henke, 2005) is the densest
widely-implemented binary-to-text encoding that stays in printable ASCII: two
characters carry 13 or 14 bits, chosen adaptively, for about 81.3 % efficiency
against Base64's 75 %. Its 91-character alphabet leaves out `\` and `'` — but
not `"`, so its output has to be escaped inside a JSON string, and the density
it gained on paper it gives back in the file.

base91-jdp makes one substitution: **`"` leaves the alphabet and `-` takes its
place.** With `"`, `\` and `'` all absent, and no character below `0x20`, the
alphabet is disjoint from everything a JSON string has to escape. The encoded
size *is* the final size.

That substitution pays for a second feature. `-` lands on the alphabet's last
value, 90, which makes the pair `--` the one two-character value the block coder
can never produce — lower basE91's 14-bit threshold by one and value 8 280
becomes unreachable, at a cost of one state in 8 281. That freed pair is the
mode signal:

* `--` in block mode switches **Dynamic Passthrough** on;
* `--` in passthrough switches it off again.

In passthrough, input is written **one output character per input byte** instead
of being expanded 1.25×. The seven byte values real text is full of and the
alphabet does not contain — space, `"`, newline, `\`, carriage return, `'`, tab —
are written as stand-ins borrowed from the alphabet's rarest characters, named
per segment by a two-character header. There is no escape mechanism and no
escape character; a segment either carries a byte or it does not.

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

When a byte cannot be carried — a UTF-8 continuation byte, a byte of a JPEG —
the segment ends and the bytes go through the block coder until passthrough is
worth resuming. That is the **binary fallback**, and it is why the format
handles mixed content without being told which is which.

## Where it wins, and where it does not

Measured on Base85N's benchmark corpus, unchanged: 6.52 MB of real files.
Characters per input byte, once the output sits in a JSON string. Full tables,
method and every sweep: [`bench/results/RESULTS.md`](bench/results/RESULTS.md).

| | Base64 | Ascii85 | basE91 | [Base85N](https://base85n.ghadami.de/) | base91-jdp |
|---|---|---|---|---|---|
| text files | 1.333 | 1.262 | 1.252 | **0.965** | 1.001 |
| binary files | 1.333 | 1.163 | 1.228 | **1.050** | 1.170 |
| whole corpus | 1.333 | 1.213 | 1.240 | **1.007** | 1.084 |

**Against basE91** — the format it is a variant of — the swap costs nothing and
the container saves **12.5 %**. Same algorithm, same density, one character
different, and no escaping.

**Against Base64**: 18.7 % smaller over the corpus, 25.0 % on text.

On the text corpus it lands at **1.00081** — passthrough carries real source,
JSON and prose at essentially one character per byte.

**Against Base85N** it splits, and the split is the honest summary of what this
format is for:

| sample | Base85N | base91-jdp |
|---|---|---|
| `sql-wasm.wasm` | 1.239 | **1.208** |
| `DejaVuSans.ttf` | 1.232 | **1.217** |
| `grace_hopper.jpg` | 1.249 | **1.229** |
| `minduka_present.png` | 1.250 | **1.229** |
| `bootstrap.css` | 1.003 | **1.001** |
| `countries.min.json` | 1.003 | **1.000** |
| `lodash.js` | 1.004 | **1.001** |
| `countries.json` | **0.935** | 1.000 |
| `commonmark-spec.txt` | **0.859** | 1.005 |
| `requests-2.32.3.tar` | **0.767** | 1.044 |
| whole corpus | **1.007** | 1.084 |

base91-jdp wins **every file neither codec can compress** — the WebAssembly
module, the font, the JPEG, the PNG — by 1.2 % to 2.5 %. Where both formats have
run out of structure to exploit, the alphabet is all that is left, and 91
characters beat 85 by about 1.5 %.

It also wins on **short payloads**, because a segment that runs to the end of
the input needs no closing signal:

```
input     {"user":"ada","id":42,"role":"admin"}          37 bytes
Base64    eyJ1c2VyIjoiYWRhIiwiaWQiOjQyLCJyb2xlIjoiYWRtaW4ifQ==    52
Base85N   %nU$w{~user~:~ada~^~id~:42^~role~:~admin~}              42
base91-jdp  --EA{$user$:$ada$,$id$:42,$role$:$admin$}             41
```

It loses everywhere Base85N's **Fill** mode has runs to work with: the zero
padding in a block-aligned tar, the indentation in pretty-printed JSON, the long
space runs in a specification document. base91-jdp has no run-length construct
at all, and that is a decision rather than an omission. The whole 7.7 % gap is
that one construct — but the same files, deflated first, go to 0.332, and there
base91-jdp is ahead of Base85N by 1.6 %. A mode that wins back a fifth of what a
call to zlib wins, on exactly the payloads where zlib is available, is not worth
the format complexity or the expansion bound it would cost. `npm run bench:fill`
has the numbers; §15 of the specification keeps 6 233 header values reserved in
case that judgement ever changes.

### So which should you use?

* Output goes into **XML, HTML or an SVG attribute** → not this. `<`, `>` and
  `&` are all in this alphabet. Use [Base85N](https://base85n.ghadami.de/),
  whose alphabet contains none of them.
* **You can compress before encoding** → base91-jdp, on everything, by 1.6 %.
  See [Compress first](#compress-first--and-how-to-know-when); this is the case
  the format is built for and the one where the split below stops mattering.
* Payload is **incompressible binary in JSON** — a key, a hash, a thumbnail, a
  compressed blob, a media file → base91-jdp, by 1.2 % to 2.5 %.
* Payload has **long runs of one byte** and must go in **uncompressed** — zero
  padding, deep indentation → Base85N. Its Fill mode wins by more than the
  alphabet loses.
* Payload is **ordinary text with no long runs**, uncompressed — minified JSON,
  CSS, a log line → base91-jdp, narrowly.
* Payload is a **short JSON or text field in a JSON document** → base91-jdp, by
  a character or two.
* You already use **basE91** and the output lands in JSON → base91-jdp, by
  12 %, for a one-character change to the alphabet.
* You can send **raw bytes** → send raw bytes. Any encoding is a loss.

## Install

```bash
npm install base91-jdp
```

Node 18.11 or newer, ESM only, zero dependencies. The core runs unchanged in
browsers, Deno and Bun.

## API

| Function | Description |
|---|---|
| `encode(bytes: Uint8Array): string` | Encode binary data. |
| `decode(text: string): Uint8Array` | Decode; whitespace in the input is skipped. |
| `encodeText(text: string): string` | Encode a string as UTF-8. |
| `decodeText(text: string): string` | Decode to a string; throws on invalid UTF-8. |
| `ALPHABET: string` | The 91 characters, in value order. |
| `R_CHARS: number[]` | The eight substituted byte values, in mask-bit order. |
| `PROFILES: string[]` | The donor profiles. |
| `CONSTANTS` | The frozen constants of the specification. |
| `makeCodec(config)` | The parameterised core, for experiments. |

Everything throws `Base91JdpError` on malformed input, with a `code` from
`ERR`: `INVALID_CHARACTER`, `UNEXPECTED_EOS`, `UNDEFINED_SIGNAL`,
`INVALID_FLUSH`, `INVALID_FINAL_BLOCK`.

```js
import { encodeText, decodeText } from 'base91-jdp';

const encoded = encodeText('Grüße aus München');
JSON.stringify({ note: encoded });   // needs no escaping, whatever the input was
decodeText(encoded);                 // 'Grüße aus München'
```

## Command line

```bash
base91jdp photo.jpg > photo.b91          # encode
base91jdp -d photo.b91 > photo.jpg       # decode
gzip -9 < dump.bin | base91jdp -w 100    # wrap at 100 characters
```

Input is treated as raw bytes; output carries no trailing newline. Whitespace is
skipped on decode, so wrapped output decodes without preprocessing.

## Compress first — and how to know when

base91-jdp is a transcoder, not a compressor. It has no run-length or dictionary
construct on purpose: the answer to a payload with structure in it is a real
compressor, not a mode. Deflate output is incompressible, so it encodes at a
flat 1.2308 — which is exactly the case base91-jdp is best at.

**And that is where it wins outright.** Deflate the corpus first and the
comparison stops splitting, because once the payload is incompressible the
alphabet is the only thing left:

| whole corpus, deflate -9 then encoded | characters per input byte |
|---|---|
| Base85N | 0.33741 |
| **base91-jdp** | **0.33194** — 1.6 % smaller |

The same 1.5 % that shows up on a JPEG shows up on everything, because
compression has turned everything into a JPEG as far as an encoder is concerned.
A run-length mode would not get close: bounding one generously puts the corpus
at 0.965 where deflate reaches 0.332, so the construct base91-jdp declined to
build is worth about a fifth of what calling zlib is worth.

That leaves the caller one decision, and it is not obvious, because passthrough
already carries text at 1.0. **Deflating pays exactly when deflate compresses to
below 81 %** — `1 / 1.2308` — of what passthrough would have charged.

Measured over 6 174 payloads from 64 B to 256 KiB (`npm run bench:gzip`):

| payload | deflate wins |
|---|---|
| deflate ratio < 0.8 | 100 %, at every size from 64 B up |
| deflate ratio 0.8–1.0 | 16 % at 64 B, 58 % at 128 B, 100 % from 512 B |
| deflate expands it | never |

So the decision is essentially *does deflate compress this at all*, with a
size correction that only bites below 256 bytes.

**Below ~4 KB, do not predict — try both.** Deflating 4 KB takes 0.046 ms; the
two encodes cost less than the branch is worth arguing about.

**Above that, 512 bytes of prefix are enough.** Deflate the first 512 bytes,
encode the first 512 bytes, and compare the two rates:

```js
import { deflateRawSync } from 'node:zlib';
import { encode } from 'base91-jdp';

const PROBE = 512;
const BLOCK = 16 / 13;          // what block mode charges per byte

function encodeSmart(bytes) {
  const deflate = (b) => deflateRawSync(b, { level: 9 });
  if (bytes.length <= 4096) {                       // cheap enough to try both
    const a = encode(bytes);
    const b = encode(deflate(bytes));
    return b.length < a.length ? { deflated: true, text: b } : { deflated: false, text: a };
  }
  const probe = bytes.subarray(0, PROBE);
  const jdpRate = encode(probe).length / PROBE;     // ~1.0 text, ~1.23 binary
  const gzRate = deflate(probe).length / PROBE;
  return BLOCK * gzRate < jdpRate
    ? { deflated: true, text: encode(deflate(bytes)) }
    : { deflated: false, text: encode(bytes) };
}
```

How the prefix size pays off, over payloads longer than the prefix:

| bytes inspected | decisions correct | bytes lost against a perfect oracle |
|---|---|---|
| 128 | 89.2 % | 21.3 % |
| 256 | 99.4 % | 1.9 % |
| **512** | **99.8 %** | **0.002 %** |
| 4096 | 99.2 % | 0.002 % |

512 is where the curve stops moving. The probe costs **0.038 ms** against
**74.6 ms** to deflate a 1.4 MB payload outright — two thousand times cheaper
than finding out the hard way.

The residual 0.002 % is not a rounding artefact of the rule; it is what the
remaining mistakes are worth. They are all near-ties: where deflate neither
compresses nor expands, the two paths land within 0.2 % of each other (median),
so picking the wrong one costs almost nothing. The decisions that matter are the
easy ones.

Note that the stream does not record which path you took — there is no
"deflated" bit in the format. Carry it yourself, in a sibling JSON field or a
one-character prefix of your own.

## Safety properties

* **Nothing it emits needs escaping in JSON.** Not a design goal met by
  testing — a property of the alphabet, which contains no `"`, no `\` and no
  character below `0x20`.
* **A decoder can never write more than it reads.** Passthrough is exactly 1:1
  and a block pair yields at most two bytes, so there is no decompression bomb
  and no expansion bound to get wrong. Formats with a run-length construct need
  one; this one does not have the construct.
* **No length field is carried**, so no length is attacker-controlled.
* **Malformed input throws**, and never reads outside its buffer.

## Specification

[`spec/base91-jdp-v0.2.0.md`](spec/base91-jdp-v0.2.0.md) defines the format
completely: alphabet, the threshold change that frees `--`, the passthrough
signal and its header, the prefix scan, canonicity, error handling, and the
measurements behind every constant.

Status is **draft**. The format is complete and implemented, but it has not been
in the field, and §15 reserves space for a run-length mode that a later version
is expected to add.

## Tests and benchmarks

```bash
npm test                    # round trip, canonicity, adversarial decode
python3 bench/corpus.py     # fetch the benchmark corpus (pinned, verified)
npm run bench               # the size tables
npm run bench:sweep         # the parameter sweeps
npm run bench:signal        # the signal character, and why segments end
npm run bench:fill          # what a run-length mode would be worth
```

The Base85N column of the benchmark runs the upstream Go implementation
(v0.5.1) and needs Go on the path; without it that column is left out rather
than filled in from documentation.

## Credit

* **basE91** — Joachim Henke, 2005. The block coder is his, unchanged but for
  one alphabet character and one threshold.
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
