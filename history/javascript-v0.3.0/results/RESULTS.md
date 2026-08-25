# Measurements

Every number here comes from a run of the benchmarks in this repository, on the
corpus `bench/corpus.py` fetches. Regenerate with:

```bash
python3 bench/corpus.py
npm run bench            # the size tables below
npm run bench:pipeline   # modes, marker crossover, side channel, throughput
npm run bench:sweep      # the constants of spec section 6.9
npm run bench:rs         # the Reed-Solomon study, bench/results/RS.md
```

The corpus is Base85N's, unchanged: 6.52 MB across 13 real files -- three
binary container formats, an uncompressed source tar, a JSON dataset in both
pretty-printed and minified form, JavaScript, CSS and Python source, the
CommonMark specification, a Markdown changelog, a JPEG and a PNG. Using it
unchanged is what makes the comparison a comparison. The donor profiles are
derived on a separate 2.37 MB training corpus (`tools/traincorpus.py`) that
shares no file and no upstream project with it.

The Base85N columns run the upstream Go implementation, v0.5.1. Deflate is
level 6 on both sides of every comparison: an earlier round of this benchmark
compared our level 6 against a reference built at level 9 and drew the wrong
conclusion from a 0.9 % gap that belonged to the compressor, not the codec.

---

## 1. Size

`jdp, no LZ4` is the headerless codec on its own -- passthrough and the block
coder. `base91-jdp` is `encode` with its defaults, which builds both candidates
and keeps the shorter.

### Encoded characters per input byte

| sample | input | Base64 | Ascii85 | basE91 | Base85N | Base64+deflate | Base85N+deflate | jdp, no LZ4 | base91-jdp |
|---|---|---|---|---|---|---|---|---|---|
| sql-wasm.wasm | 659,730 B | 1.333 | 1.247 | 1.211 | 1.239 | 0.653 | **0.612** | 1.228 | 0.851 |
| _cffi_backend.so | 1,068,624 B | 1.333 | 1.026 | 1.190 | 0.965 | 0.465 | **0.436** | 1.228 | 0.629 |
| DejaVuSans.ttf | 756,072 B | 1.333 | 1.240 | 1.219 | 1.232 | 0.681 | **0.638** | 1.229 | 0.790 |
| requests-2.32.3.tar | 655,360 B | 1.333 | 1.015 | 1.206 | 0.767 | 0.268 | **0.251** | 1.065 | 0.382 |
| countries.json | 1,408,911 B | 1.333 | 1.250 | 1.220 | 0.935 | 0.145 | **0.136** | 1.000 | 0.259 |
| countries.min.json | 772,294 B | 1.333 | 1.250 | 1.231 | 1.003 | 0.221 | **0.207** | 1.000 | 0.374 |
| lodash.js | 544,098 B | 1.333 | 1.250 | 1.225 | 1.004 | 0.239 | **0.224** | 1.001 | 0.392 |
| bootstrap.css | 281,046 B | 1.333 | 1.250 | 1.228 | 1.003 | 0.158 | **0.148** | 1.001 | 0.307 |
| requests-models.py | 35,418 B | 1.333 | 1.250 | 1.224 | 0.973 | 0.374 | **0.350** | 1.002 | 0.561 |
| commonmark-spec.txt | 202,827 B | 1.333 | 1.250 | 1.229 | 0.859 | 0.298 | **0.279** | 1.005 | 0.456 |
| requests-history.md | 60,368 B | 1.333 | 1.250 | 1.229 | 0.979 | 0.442 | **0.414** | 1.002 | 0.635 |
| grace_hopper.jpg | 61,306 B | 1.333 | 1.250 | **1.229** | 1.249 | 1.330 | 1.246 | 1.231 | 1.231 |
| minduka_present.png | 13,634 B | 1.333 | 1.250 | **1.229** | 1.250 | 1.334 | 1.250 | 1.231 | 1.231 |
| text files | 3,304,962 B | 1.33333 | 1.25000 | 1.22461 | 0.96474 | 0.19672 | **0.18442** | 1.00081 | 0.33388 |
| binary files | 3,214,726 B | 1.33334 | 1.12450 | 1.20546 | 1.05041 | 0.53412 | **0.50073** | 1.19487 | 0.67613 |
| whole corpus | 6,519,688 B | 1.33333 | 1.18812 | 1.21517 | 1.00698 | 0.36308 | **0.34039** | 1.09650 | 0.50264 |

### Characters per input byte once the output sits in a JSON string

| sample | input | Base64 | Ascii85 | basE91 | Base85N | Base64+deflate | Base85N+deflate | jdp, no LZ4 | base91-jdp |
|---|---|---|---|---|---|---|---|---|---|
| sql-wasm.wasm | 659,730 B | 1.333 | 1.291 | 1.246 | 1.239 | 0.653 | **0.612** | 1.228 | 0.851 |
| _cffi_backend.so | 1,068,624 B | 1.333 | 1.070 | 1.209 | 0.965 | 0.465 | **0.436** | 1.228 | 0.629 |
| DejaVuSans.ttf | 756,072 B | 1.333 | 1.287 | 1.241 | 1.232 | 0.681 | **0.638** | 1.229 | 0.790 |
| requests-2.32.3.tar | 655,360 B | 1.333 | 1.027 | 1.222 | 0.767 | 0.268 | **0.251** | 1.065 | 0.382 |
| countries.json | 1,408,911 B | 1.333 | 1.257 | 1.267 | 0.935 | 0.145 | **0.136** | 1.000 | 0.259 |
| countries.min.json | 772,294 B | 1.333 | 1.268 | 1.234 | 1.003 | 0.221 | **0.207** | 1.000 | 0.374 |
| lodash.js | 544,098 B | 1.333 | 1.265 | 1.248 | 1.004 | 0.239 | **0.224** | 1.001 | 0.392 |
| bootstrap.css | 281,046 B | 1.333 | 1.267 | 1.244 | 1.003 | 0.158 | **0.148** | 1.001 | 0.307 |
| requests-models.py | 35,418 B | 1.333 | 1.264 | 1.251 | 0.973 | 0.374 | **0.350** | 1.002 | 0.561 |
| commonmark-spec.txt | 202,827 B | 1.333 | 1.263 | 1.237 | 0.859 | 0.298 | **0.279** | 1.005 | 0.456 |
| requests-history.md | 60,368 B | 1.333 | 1.264 | 1.240 | 0.979 | 0.442 | **0.414** | 1.002 | 0.635 |
| grace_hopper.jpg | 61,306 B | 1.333 | 1.281 | 1.240 | 1.249 | 1.330 | 1.246 | **1.231** | **1.231** |
| minduka_present.png | 13,634 B | 1.333 | 1.281 | 1.239 | 1.250 | 1.334 | 1.250 | **1.231** | **1.231** |
| text files | 3,304,962 B | 1.33333 | 1.26249 | 1.25189 | 0.96474 | 0.19672 | **0.18442** | 1.00081 | 0.33388 |
| binary files | 3,214,726 B | 1.33334 | 1.16265 | 1.22770 | 1.05041 | 0.53412 | **0.50073** | 1.19487 | 0.67613 |
| whole corpus | 6,519,688 B | 1.33333 | 1.21326 | 1.23996 | 1.00698 | 0.36308 | **0.34039** | 1.09650 | 0.50264 |

### What the three readings say

**Against the plain binary-to-text codecs, twice as good**: 0.503 against
Base85N's 1.007 and Base64's 1.333.

**Against deflate-then-encode, 48 % worse**: 0.503 against Base85N+deflate's
0.340. That is the price of LZ4 over deflate. It was chosen on the grounds
that a specification which demands LZ4 demands a few hundred lines and one
which demands deflate demands a library -- a judgement about who can implement
the format, not about compression.

**On data that will not compress, the only column that does not lose.** A
deflate pipeline expands an already-compressed file past plain Base64:
`grace_hopper.jpg` goes to 1.330 and `minduka_present.png` to 1.334, where
base91-jdp stays at 1.231 by measuring both candidates and keeping the
headerless one.

---

## 2. The pipeline

### Which mode each file lands in

| sample | input | mode | segments | chars/byte |
|---|---|---|---|---|
| sql-wasm.wasm | 659,730 B | lz4 | 3 | 0.8507 |
| _cffi_backend.so | 1,068,624 B | lz4 | 5 | 0.6293 |
| DejaVuSans.ttf | 756,072 B | lz4 | 3 | 0.7902 |
| requests-2.32.3.tar | 655,360 B | lz4 | 3 | 0.3818 |
| countries.json | 1,408,911 B | lz4 | 6 | 0.2588 |
| countries.min.json | 772,294 B | lz4 | 3 | 0.3738 |
| lodash.js | 544,098 B | lz4 | 3 | 0.3921 |
| bootstrap.css | 281,046 B | lz4 | 2 | 0.3070 |
| requests-models.py | 35,418 B | lz4 | 1 | 0.5609 |
| commonmark-spec.txt | 202,827 B | lz4 | 1 | 0.4557 |
| requests-history.md | 60,368 B | lz4 | 1 | 0.6349 |
| grace_hopper.jpg | 61,306 B | headerless | 1 | 1.2305 |
| minduka_present.png | 13,634 B | headerless | 1 | 1.2308 |
| **whole corpus** | 6,519,688 B | | | **0.50264** |

### Where the marker starts paying for itself

A framed stream costs two characters that a headerless one does not.
Below some size the compressor cannot make them back. That size is not
a constant in the format -- `encode` compares the two candidates and
takes the shorter -- so this is a measurement, not a threshold.

| payload | text | JSON | source | random bytes |
|---|---|---|---|---|
| 16 B | headerless | headerless | headerless | headerless |
| 32 B | headerless | headerless | headerless | headerless |
| 64 B | headerless | framed | framed | headerless |
| 128 B | framed | framed | framed | headerless |
| 256 B | framed | framed | framed | headerless |
| 512 B | framed | framed | framed | headerless |
| 1024 B | framed | framed | framed | headerless |
| 4096 B | framed | framed | framed | headerless |

### Side channel, on real data

| sample | symbols | slots | rate | bits per segment |
|---|---|---|---|---|
| sql-wasm.wasm | 280,608 | 6,507 | 2.319 % | 2,169 |
| _cffi_backend.so | 336,252 | 14,605 | 4.343 % | 2,921 |
| DejaVuSans.ttf | 298,704 | 10,381 | 3.475 % | 3,460 |
| requests-2.32.3.tar | 125,104 | 4,006 | 3.202 % | 1,335 |
| countries.json | 182,284 | 4,758 | 2.610 % | 793 |
| countries.min.json | 144,328 | 3,988 | 2.763 % | 1,329 |
| lodash.js | 106,656 | 3,320 | 3.113 % | 1,107 |
| bootstrap.css | 43,140 | 1,845 | 4.277 % | 923 |
| commonmark-spec.txt | 46,216 | 1,190 | 2.575 % | 1,190 |

The window is 88 of 8192 symbol values, scattered by v * 8179 mod 8192, so uniform data would give 1.074 % and real data gives more.
Those bits cost no characters at all: a symbol in the window is written
as its reserved pair value, and the pair is still two characters wide.

### Throughput, by layer

Measured on countries.json, 1,408,911 bytes.

| layer | encode | decode |
|---|---|---|
| LZ4 alone | 231.8 MB/s | 229.8 MB/s |
| bytes to symbols | 730.8 MB/s | -- |
| frame, check only | 401.8 MB/s | 108.7 MB/s |
| frame with Reed-Solomon | 209.2 MB/s | 98.8 MB/s |
| symbols to characters | 352.8 MB/s | 399.2 MB/s |
| whole pipeline | 88.2 MB/s | 74.7 MB/s |
| whole pipeline, protected | 84.8 MB/s | 40.0 MB/s |

Segments are 256 KiB of payload and codewords are 4092 data symbols plus 4 parity, so the parity costs 0.098 %.

---

## 3. What the fixed thirteen-bit symbol costs

basE91 takes fourteen bits when a symbol's low thirteen are at most 88, so its
density depends on the data. Against the fixed symbol, per file:

| file | adaptive | fixed | cost |
|---|---|---|---|
| all seven text files | -- | -- | 0.000 % |
| `grace_hopper.jpg` | 1.22879 | 1.23055 | 0.143 % |
| `minduka_present.png` | 1.22869 | 1.23082 | 0.173 % |
| `DejaVuSans.ttf` | 1.21739 | 1.22894 | 0.949 % |
| `sql-wasm.wasm` | 1.20838 | 1.22777 | 1.605 % |
| `requests-2.32.3.tar` | 1.04385 | 1.06491 | 2.018 % |
| `_cffi_backend.so` | 1.18713 | 1.22765 | 3.413 % |
| **whole corpus** | 1.08442 | 1.09650 | **1.114 %** |

An earlier estimate of 0.08 % was taken from deflated input and does not hold
for raw binary. Zero-heavy binaries hit the fourteen-bit branch constantly:
`_cffi_backend.so` was averaging 13.478 bits per pair, a 47.8 % branch rate
against the 1.086 % of uniform data.

Structured binary is exactly what the compressed mode exists for, so this cost
lands on a headerless stream and almost never on a framed one. What it buys is
the eighty-nine free pair values, the symbol layer error correction sits on,
and a three-byte bound on what one damaged character reaches.

---

## 4. The damage bound

4 MiB of payload in sixteen segments, protected, bursts of mangled characters
placed at random, 200 trials per width:

| burst | worst damage | in segments | silently wrong |
|---|---|---|---|
| 4 characters | 262 144 B | 1.00 | 0 |
| 64 | 524 288 B | 2.00 | 0 |
| 256 | 262 144 B | 1.00 | 0 |
| 1 024 | 524 288 B | 2.00 | 0 |
| 4 096 | 524 288 B | 2.00 | 0 |

Two segments is the bound: one for a codeword error correction cannot repair,
two for a separator destroyed outright, because the segments on either side of
it merge. It is reached and never exceeded, and no run in 1 200 returned
altered bytes without reporting a damaged segment.

Scattered damage is a different question and the answer is that there is none:
over 3 MiB with one to eight flipped bits, all 240 trials were repaired
exactly.

An earlier arrangement failed this at 256-character bursts, losing eleven
segments of sixteen, because the check pattern mixed in a stream-wide codeword
counter: a lost separator shifted it and failed every segment after. The
counter is per segment now.

---

## 5. The side-channel window

Which eighty-eight of the 8 192 symbol values carry a bit is a free choice, and
it decides how much the channel carries. Forty distributions -- raw and
LZ4-compressed text, source, JSON, CSV, XML, images, uniform bytes, zeros:

| window | worst case | mean |
|---|---|---|
| the top 88 values | 0.000 % | 0.5 % |
| the bottom 88 | 0.000 % | 11 % |
| every 91st value | 0.55 % | 4.4 % |
| `v x 8179 mod 8192 < 88` | **0.52 %** | 4.4 % |

The bottom window carries the most where it works -- LZ4 writes two-byte
offsets whose high byte is zero whenever a match is near, and those make small
symbols -- and nothing at all on repeated raw text. The choice went on the
worst case: a check that can vanish is not a check.

Every scattered window performs within noise of every other, so the multiplier
was chosen on the synthetic shapes and then checked against the corpus, which
the search never saw. 8 179 is -13 modulo 8 192, and thirteen is the symbol
width, so it steps the window across bit-alignment classes rather than along
the grain of the data.

---

## 6. LZ4 against the reference

`src/lz4.js` was checked against upstream liblz4 (python-lz4 4.x) in both
directions over 168 inputs: the reference decoder accepts our blocks and
returns the original bytes for every one, our decompressor reads its blocks,
and our compressed size is **1.0068** of its. `test/lz4-fixtures.js` keeps ten
of its blocks so the check runs without the reference installed.

---

## 7. The constants

`npm run bench:sweep` produces the tables. With the fixed symbol they still sit
on the plateaus that chose them: `MIN_BINARY_RUN` = 4 is the optimum,
`MIN_DP_BYTES` = 26 is within 0.001 % of the measured best (28) and is the
value derivation gives, and `MAX_DP_BYTES` is a bound on encoder memory rather
than a ratio choice.

`bench/results/RS.md` holds the study that chose GF(2^13) over GF(2^8) and
n=4096 over the alternatives.
