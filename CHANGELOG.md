# Changelog

## Unreleased — v0.4.0 draft

A different format, not a revision of the last one: head-of-stream mode markers
became typed segments, LZ4 became zstd, and the Reed-Solomon layer went. A
0.3.0 stream and a 0.4.0 stream do not interoperate and neither decoder reads
the other's output. The specification is `spec/base91-jdp-v0.4.0.md` and its
Sections 18.1, 18.4 and 18.5 say why each thing went.

A compressed segment carries as little of a zstd frame as the format can get
away with. The magic number repeats what the segment signal said, the content
size and the block header's size field repeat the length field, and the
checksum answers a question Section 2.3 says this format does not ask — eleven
bytes in all. Class 17 drops the first six by asking zstd for a magicless frame
with no content size, checksum or dictionary id; the new class 20, `ZBLK`,
drops the last five by carrying a bare compressed block on any payload that
fits in one, which is everything up to 128 KiB. It is worth nothing on a
megabyte and 11 % of the encoding on a protocol field, and it halves the length
at which compression starts to pay. Specification Sections 10.1, 10.2 and 17.20.

Measured head to head against Base85N 0.5.1, both implementations built from
source and run in one process under one timing loop, so the comparison is of
two encodings rather than two languages. Smaller in every configuration: 2.3 %
on the core corpus and 1.3 % on Silesia with neither side compressing, 14.2 %
on field-length payloads, 41 % to 69 % with a compressor. At zstd −5, 48 %
smaller and 14 % faster at the same time. Slower in two places, both stated
rather than buried: six times slower to encode without a compressor, which is
the candidate scan, and two to three times slower to decode in every
configuration, which is what 91 characters cost a byte-oriented decoder against
85. And two things Base85N does that this format does not — its alphabet is
safe in XML, HTML and CSV where this one is dense inside a JSON string and
nothing more, and a compressed payload is not readable where passthrough leaves
text legible. Specification Section 17.21.

Finding that took three per-byte costs out of the decoder, none of them the
format: a whole-stream copy to strip whitespace no stream contains, a walk over
the R-Set per byte of passthrough where a table answers in one lookup, and a
missing bulk path for block mode where the encoder has had one from the start.
Decode went from 213 MB/s to 381 on the core corpus.

The reference implementation moved with it. v0.4.0 is implemented in
`rust/`; the JavaScript package that implemented v0.3.0 is complete, tested and
kept under `history/javascript-v0.3.0/`, where it is no longer published to npm
and no longer maintained. The repository root no longer ships a library: what
is at the root is the specification, the corpora and the tooling.

Everything below this line is the history of v0.3.0 and earlier.

---

The format is a draft and has had no users, so nothing here is a compatibility
note. These entries exist so that we can tell which version we mean when we
talk about one.

## 0.3.0 — 2026-08-23

The format grows a compressor, error correction and a way of saying which of
those it is using. All three sit on one change to the block coder.

### The fixed thirteen-bit symbol

basE91 lets a pair carry thirteen or fourteen bits, chosen from the data, and
so reaches all 8 281 pair values. base91-jdp now fixes symbols at thirteen bits.
Thirteen bytes are eight symbols in sixteen characters, exactly, and 8 192
through 8 280 become values no encoded stream can contain.

That costs 1.114 % over the corpus — nothing at all on text, 0.14 % to 0.17 %
on high-entropy binary, and 1.6 % to 3.4 % on structured binary, which is the
input the compressed mode exists for. An earlier estimate of 0.08 % came from
deflated input and does not hold for raw binary: the fourteen-bit branch fires
when a symbol's low thirteen bits are small, which zero-heavy binaries hit
constantly.

It buys three things nothing else could have: eighty-nine free pair values, a
symbol layer a Reed-Solomon code can sit on, and a bound of three bytes on what
one damaged character can reach.

### The marker

Two characters at the head of a stream, drawn from the values no packer can
write, say whether it carries LZ4, error correction, or both. Detection is
total: a headerless stream *cannot* begin with a marker, so there is no escape
clause and no exclusion rule anywhere in the encoder.

Every marker's second character is `-`, which classic basE91 cannot produce at
all, so classic basE91 stays out of band without a flag.

A stream that wants neither compression nor protection still pays nothing.

### LZ4, inside

`src/lz4.js` implements the block format with no dependency, and is checked
against upstream liblz4 in both directions: it reads our blocks byte for byte,
we read its blocks, and our ratio lands within 0.7 % of its. Fixtures in
`test/lz4-fixtures.js` keep that check running without anyone needing the
reference installed.

Over the corpus base91-jdp is now **0.50264** characters per byte against
Base85N's 1.00698 — twice as good — and against Base85N applied to deflated
bytes, 0.34039, it is 48 % worse. That is the price of LZ4 over deflate,
measured rather than assumed. Where it wins outright is data that will not
compress: 1.231 on a JPEG, where a deflate pipeline goes to 1.334.

The encoder builds the framed candidate, computes its exact size and compares.
There is no size threshold in the format.

### Error correction, and a check that costs nothing

Reed-Solomon over GF(2^13) repairs two damaged symbols per 4 096-symbol
codeword for 0.098 %. A check pattern rides in the free pair values at no cost
in characters at all.

Segments of 256 KiB are divided by `--`, each with its own LZ4 dictionary. One
to eight flipped bits in a protected stream are repaired exactly. Damage that
overwhelms a codeword costs one segment; a separator destroyed outright costs
two, because they merge; nothing costs a third. Measured over 1 200 trials with
bursts of 4 to 4 096 characters, and not one run returned altered bytes without
reporting a loss.

### Errors

Every rejection is a `Base91JdpError` with a code, whichever layer refused.

### Fixed during development, both found by measurement

**The check pattern coupled the segments.** It mixed in a stream-wide codeword
counter, so a burst that destroyed one separator merged two segments, shifted
the counter, and failed the check for every segment after it — one 256-character
burst cost eleven segments of sixteen. The counter is now local to its segment.
This was the length chain the separators were designed to remove, reintroduced
through the back door.

**The side channel was carrying a fifth of what it should.** The window had been
the top eighty-eight symbol values, chosen because the offset was tidy;
thirteen-bit symbols are nothing like uniform and almost nothing lands up there.
Measured over forty distributions, the window is now scattered, and the channel
carries 2.3 % to 4.3 % of symbols against 0.2 % to 0.5 % before, at the same
encoded size to the character.

### Also

- `decodeDetailed` reports the mode, the segments, what was repaired and what
  was lost. `decode(text, { partial: true })` returns what survived.
- `--protect`, `--compress`, `--partial` and `--verbose` on the command line.
- `bench/pipeline.js`: which mode each file lands in, where the marker starts
  paying for itself, what the side channel carries, throughput per layer.
- `bench/bench.js` gains the comparison that was missing: deflate-then-encode.
- Pairs are spelled low digit first everywhere. `src/pack.js` had disagreed
  with the rest of the format and with the specification.
- 76 tests, up from 44.

## 0.2.0 — 2026-08-22

`-` joined the R-Set. It is the one member that *is* in the alphabet:
substituted not because it cannot be written but because two in a row would end
a segment. A payload therefore never contains `-` at all, which retired two
special cases from the prefix scan and took text full of `--` — CSS custom
properties, Markdown rules, command lines — from a mode switch per occurrence to
one donor per segment. Worth 0.29 % over the corpus and 5.6 % on
`bootstrap.css`.

## 0.1.0 — 2026-08-21

basE91 with `"` swapped out of the alphabet for `-`, which leaves the alphabet
disjoint from everything a JSON string escapes, and Dynamic Passthrough on the
pair the swap frees.
