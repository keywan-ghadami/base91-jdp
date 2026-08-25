# base91-jdp, Rust prototype

An encoder and decoder for **specification v0.4.0**, written to find out
whether the format encodes at the density the specification projected, and
whether the parallel and vector paths it was shaped for actually pay.

```rust
let data = b"{\"user\":\"ada\",\"id\":42}";
let text = base91_jdp::encode(data);
assert_eq!(base91_jdp::decode(&text).unwrap(), data);
```

```sh
cargo test                                    # round trip, canonicity, adversarial decode
cargo run --release --example corpus -- ../bench/corpus
cargo run --release --example sweep  -- ../bench/corpus
cargo +nightly test --features simd           # the same, with the vector paths on
```

## What it implements, and what it does not

Every class of the specification: passthrough with the eight-member R-Set and
the donor profiles, the packed bases, the run and chained-gap classes, the
compressed segment, and the block coder under all of them — with a decoder for
each. Class 20 is behind the default `zstd` feature, which brings in the `zstd`
crate; `--no-default-features` builds the container alone, and its decoder then
refuses class 20 with `Code::UnknownClass`, as section 15.5 requires of an
implementation that ships without it.

## What it found

Three things the specification did not say before this crate encoded a byte,
and one it said wrongly.

**The prefix scans were swallowing the runs.** The first run over the benchmark
corpus produced 1.03809 characters per byte where the specification projected
1.00464. Passthrough carries a zero byte at one character each — NUL is an
R-Set member — so the greedy prefix scan of section 11.1 was absorbing exactly
the runs that `ZRUN` carries at eighty-nine for three characters. `tar` came out
at 1.0018 against a projected 0.7823. The **run break** is that finding, and it
is now normative in section 11.1.

**`MIN_BINARY_RUN` should not exist.** 0.3.0 set it to 4, measured against a
structure this version does not have. Swept here, zero is the best value: with
typed segments there is nothing for it to prevent, and forcing four bytes
through block mode after every segment costs 0.3 %.

Together those two are three percent of the corpus, and they take the encoder
from behind Base85N to ahead of it with no compressor on either side:

| | Base85N | jdp 0.3.0 | 0.4.0 projected | **0.4.0, this crate** |
|---|---|---|---|---|
| core corpus, 6.52 MB | 1.00698 | 1.09650 | 1.00464 | **0.98354** |
| Silesia, 202 MiB | 1.05114 | 1.09861 | 1.03434 | **1.03792** |

**Parallel encoding works, and the obvious join does not.** Cutting the input
at multiples of thirteen bytes lets each worker start with an empty accumulator
(section 14.5). Splicing a worker's output only where that assumption held
outright fires on a fifth to a half of chunks, and repairing a whole chunk
otherwise makes the parallel encoder slower than the serial one. Bounding the
repair at the first segment boundary both paths reach gives 2 to 3.5× on four
threads, with output byte-identical to a serial encode — asserted by the tests
at four chunk sizes, down to a single symbol group.

**Scanning is what encoding costs, and on compressed data none of it is
needed.** The block coder alone runs at 549 MB/s; the encoder around it, running
the candidate scan of section 11.1 at every position and finding nothing at
every position, managed 31.

Two things fix it, and they are complements:

*The per-window decision* (`src/detect.rs`) — a magic number, or a byte
histogram over four kilobytes saying the entropy is above 7.4 bits. Then the
whole window goes through block mode with no scan at all. It needs no vector
unit and no nightly.

*The vector candidate mask* (`src/simd.rs`) — one step per thirty-two bytes
giving a bit per position: could anything open here. It carries the cases the
decision does not fire on.

| high-entropy input | stable | `--features simd` |
|---|---|---|
| deciding per window | **2 030 MB/s** | **2 010 MB/s** |
| scanning everything | 31 MB/s | 125 MB/s |
| block coder alone | 3 090 MB/s | 3 090 MB/s |

On payloads that are actually compressed — the case section 10 puts a zstd
frame into a segment for:

| payload | speed | size cost |
|---|---|---|
| `countries.json` at zstd −3 | 30 → 2 276 MB/s | +0.03 % |
| `lodash.js` at zstd −9 | 33 → 2 202 MB/s | none |
| the source tar, gzipped | 33 → 1 961 MB/s | none |
| `sql-wasm.wasm`, raw deflate | 33 → 2 126 MB/s | none |

Raw deflate carries no magic number and is caught by entropy alone, which is
why both signals are there. Over the whole core corpus the decision fired on no
window of any file that was not already compressed, and the ratio moved from
0.97944 to 0.97945.

Where the decision does not fire, the vector mask still pays, most on the files
closest to incompressible:

| input | stable | `--features simd` | |
|---|---|---|---|
| grace_hopper.jpg, decision off | 32 MB/s | 62 MB/s | 1.94× |
| sql-wasm.wasm | 36 MB/s | 62 MB/s | 1.72× |
| commonmark-spec.txt | 87 MB/s | 103 MB/s | 1.18× |
| lodash.js | 82 MB/s | 93 MB/s | 1.13× |

The same probe applied to the *passthrough* prefix scan loses, in four
arrangements, and `src/simd.rs` records each with its number. The two results
are the same lesson from both sides: a vector probe pays when it settles many
bytes per call. "Can anything start here" settles thirty-two bytes of a
compressed payload every time; "does this byte change the passthrough state"
settles two or three of English text.

**The block coder is a table, not arithmetic.** Thirteen bytes to sixteen
characters went 256 → 549 → 1 289 → 3 090 MB/s, and the last and largest step
removed work rather than adding cleverness: a pair value is at most 8 191, so
8 192 entries of two bytes — sixteen kilobytes, half a typical L1 — give both
characters in one aligned load. No division, no reciprocal, no alphabet lookup.

A vector unit does not help there. Extracting the eight symbols with two byte
shuffles and a variable shift measures 1 180 MB/s against the `u128` path's
3 050: the symbols must leave the vector registers for the table lookup, and
moving eight lanes out costs more than eight 128-bit shifts. It is implemented,
verified against the scalar path on 20 000 random groups, and not used —
`src/simd.rs` says what a vector path would need instead.

**Compression is the throughput, and nothing else is.** The container encodes
at 3.3 GB/s and does not move with the level; zstd goes from 515 MB/s at level
−5 to 2 MB/s at 19, and the whole encoder tracks it within noise at every one:

| level | chars/byte | whole encode | zstd alone | container alone |
|---|---|---|---|---|
| −5 | 0.2518 | 487 MB/s | 515 MB/s | 3 334 MB/s |
| 1 | 0.1635 | 430 MB/s | 462 MB/s | 3 342 MB/s |
| 3 | 0.1511 | 325 MB/s | 365 MB/s | 3 336 MB/s |
| 9 | 0.1206 | 61 MB/s | 59 MB/s | 3 335 MB/s |
| 19 | 0.0986 | 2 MB/s | 2 MB/s | 3 313 MB/s |

Any throughput claim about a compressing encoder here is a claim about zstd.

Section 11.2's rule — build both candidates, keep the shorter — costs three to
six times the throughput and buys one part in thirty thousand over the corpus
(0.34444 against 0.34445), because the *uncompressed* candidate is the slow one.
`encode_zstd` skips the comparison; `encode_auto` makes it.

**The short group found two more.** Fifty-five field-level samples under 200
bytes, in `bench/wire_samples.py`, are the only benchmark that reaches the
packed bases of section 9 at all — and the only one where three characters of
segment overhead are visible. Against Base64: hex digests −50 %, decimal
identifiers −47 %, UUIDs −37 %, tokens −25 %, protocol text −21 %, and −32.5 %
over the group.

The first fault: the candidate comparison counted only the characters block
mode *writes*, and block mode emits whole symbols, so the remainder is input it
has consumed and not yet paid for. Six digits went to block mode at eight
characters where `DEC` takes seven. Weighing the deferred bits is worth 0.12 %
on the core corpus too, where it had been invisible.

The second is open. The ranking is greedy and compares candidates of different
lengths by total saving, so a JWT — three base64url runs separated by dots —
goes to passthrough at 1.032 where three packed segments would be cheaper.
Ranking by saving per byte instead is *worse* on both corpora, which says the
problem is the greediness rather than the criterion. An exact segmentation over
the pending-bit state is affordable at these sizes and nobody has measured it.

**Compress, or not — for `countries.json` there is nothing to weigh.**

| | chars/byte | encode | decode |
|---|---|---|---|
| no compressor | 0.8772 | 37 MB/s | 128 MB/s |
| **zstd −5** | **0.2518** | **454 MB/s** | **249 MB/s** |
| zstd 9 | 0.1206 | 60 MB/s | 712 MB/s |

Three and a half times smaller and twelve times faster, decoding twice as fast
on top; everything up to level 9 beats not compressing on both axes. The scan
is expensive exactly on the data that compresses well, and compressing first
hands the container a payload with nothing in it to look for.

It reverses completely on a JPEG — 1.2308 at 2 493 MB/s uncompressed against
1.2311 at 1 229 compressed — and the same entropy sample tells the two apart.
`encode_smart` decides from it and builds only one candidate; over the core
corpus it produces byte-identical output to `encode_auto`'s build-both on all
thirteen files at every level tried, at three to thirteen times the throughput.

**Does zstd make the classes unnecessary?** No, and the short group says why.
Over its 55 samples compressing everything costs 1.2713 characters per byte at
level 3 — *worse than Base64* — against 0.9252 for the plain encoding, and it
is smaller on one of the fifty-five. A twelve-byte name costs 2.083 through a
compressed segment. An LZ77 window that opens on 150 bytes is empty, and no
level fixes it.

That is with the eleven bytes of frame header the segment already implies taken
off (see below). Before that it was 1.4040 and the conclusion was the same
only louder.

Taking each family away:

| classes enabled | short corpus | core corpus |
|---|---|---|
| all | **0.9252** | **0.9783** |
| no runs | 0.9681 | 1.1372 |
| no packed bases | 1.0491 | 0.9784 |
| no passthrough | 0.9597 | 1.0620 |
| block coder alone | 1.2394 | 1.2308 |

The runs carry the core corpus, the packed bases carry the short one and do
nothing at all for the core, passthrough matters to both.

**With compression on, none of them is reached.** The core corpus goes 98.85 %
through `ZSTD` and 1.15 % through block mode, and switching off every class
together leaves the ratio at 0.52273 either way. The machinery earns its place
in two situations and no others: a payload too short for a compressor to have a
window, and a caller who cannot or will not link one.

**Could the run classes go, if compression were mandatory?** `ZRUN` and `RUN`
cannot, and throughput is the reason nobody would guess: taking them out makes
the encoder *slower*, 52 MB/s to 40 on the core corpus and 66 to 59 on the
short one. A run class consumes eighty-nine bytes for three characters in one
step; it is not a cost the scan pays, it is the scan's cheapest exit. They also
still pay in size below the compression crossover, which is around a hundred
bytes — even with compression applied wherever it wins, dropping them costs
3.0 % over the short corpus.

`ZMIX` could, and has been removed. It was worth 0.53 % on the core corpus and
0.15 % on Silesia without a compressor, nothing on the short corpus, nothing
with compression, and its throughput was neutral — against eight of the
forty-four classes, a chain grammar, a canonicity rule and an error code.
Sections 17.7 and 18.7 keep the record, including the projection that justified
building it and was wrong by a factor of ten.

What the ablation *did* remove is three classes: `DEC`, `ALPHA_U` and `ALNUM`,
whose alphabets are contained in `HEXL`, `B32` and `B64` at the same width. A
subset at equal width can never produce a shorter segment, and dropping all
three left both corpora unchanged to every digit reported.

**What a segment and a frame say twice.** A zstd frame inside a typed segment
repeats most of its own header: the magic number says what the signal said, the
content-size field and the block header's size field say what the length field
said, and the checksum answers a question this format does not ask. Eleven
bytes come off before anything about the compression changes.

`compress::lean` takes the first six — magicless frame format, no content size,
no checksum, no dictionary id, all of them ordinary zstd settings.
`compress::strip` takes the last five, on any payload that came out as a single
compressed block, which is every payload up to 128 KiB: the two-byte frame
header and the three-byte block header are then fully determined, so the
segment goes out as class `ZBLK` and the decoder writes those five bytes rather
than reading them. It reads a frame this process just produced, using only the
stable frame format of RFC 8878 — not zstd's block API, which does the same
thing more directly and which upstream has deprecated for removal.

| | short group, level 3 | `countries.json` |
|---|---|---|
| stock frames | 3 403 chars | 209 509 chars |
| lean frames | 3 129 | 209 501 |
| stripped blocks | **3 027** | 209 501 |

Nothing on a megabyte, 11 % on a protocol field. What it buys is the crossover:
64 bytes of a zero-padded record now compress to 0.453 where they used to cost
0.609 and lose to the plain encoding, and over the short corpus an encoder that
compresses where compression wins goes from 0.9252 to **0.9194** — the first
time compression has improved that number at all.

**Where compression starts to pay.** `examples/firstwin.rs` sweeps it byte by
byte, against the whole plain encoder rather than against block mode:

| payload | level −5 | level 3 | level 19 |
|---|---|---|---|
| zero-padded record | 48 | **43** | 44 |
| repeated JSON record | 79 | 79 | **77** |
| repeated log line | 157 | 113 | **113** |
| repeated hex digest | 277 | 114 | **111** |
| English prose | 285 | 103 | **93** |
| high-entropy binary | never | never | never |

At level 3 the same table read 72, 90, 125, 134 and 137 before the frame
headers came off. It is not a constant and not monotone — prose at level 3
first wins at 103 bytes but only wins at every longer length from 121 — so
`encode_auto` compares rather than consulting a threshold. Note what level −5
costs on short payloads: choosing it for throughput also gives up compression
on everything under a few hundred bytes.

The 128 KiB ceiling is why class 17 still exists. Cutting a large input into
128 KiB pieces so that every segment could be stripped costs 1.9 % at level −5
and 4.7 % at levels 3 and 9 on the core corpus, because each piece starts with
an empty window. Five bytes are not worth four percent; `examples/onelblock.rs`
is the measurement.

**A radix-91 arithmetic coder, tried and rejected.** `examples/rans91.rs` is a
working rANS whose renormalisation base is 91, so it emits alphabet characters
directly instead of packing bits into them — no packing loss, and it can carry
a model. It is exact on both corpora, with a uniform model, a trained order-0
model and an eight-lane interleaved variant.

| | core corpus | short group | MB/s |
|---|---|---|---|
| block coder, as shipped | 1.230769 | 1.230769 | 2 950 |
| rANS, uniform model | 1.228551 | 1.3083 | 110 / 211 interleaved |
| rANS, trained order-0 | 0.9819 | 1.0475 | |
| **the classes, as shipped** | **0.9835** | **0.9252** | |

Below the 1.229295 floor on the core corpus, which is not a violation of
information theory but a small accidental model: the state update grows with
the symbol's cumulative frequency, so low byte values cost slightly less. It
vanishes on high-entropy input — three characters over a 61 KB JPEG — which is
exactly what the block coder sees once compression is on. Meanwhile it is 14×
slower even interleaved, because base-2ᵏ renormalisation is a shift and base-91
renormalisation is a division. Specification section 18.12 has the full
account.

## A feature flag for more speed

`--features simd` needs **nightly**, because `std::simd` is unstable. It is the
only thing in this crate that is nightly, and the crate is stable-only and
portable without it. Enabling it may never change output: the vector paths
answer questions the scalar code would answer the same way, a conservative
answer costs a scalar step, and the whole test suite runs under both.

```sh
cargo +nightly build --release --features simd
```

## Status

This is a prototype for a draft. The wire format may still change, nothing here
is published to crates.io, and the specification's section 20 says what review
would be most useful. Bugs found here are bugs in the format until shown
otherwise — that is what it is for.
