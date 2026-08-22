# base91-jdp — measurements

Everything here is measured, not quoted. Reproduce it with:

```bash
python3 bench/corpus.py     # fetch the corpus (pinned, SHA-256 verified)
node bench/bench.js         # the size tables
node bench/sweep.js         # the parameter sweeps
node bench/signalchar.js    # the signal character, and why segments end
```

## Method

**Corpus.** Base85N's benchmark corpus, unchanged: 6.52 MB across 13 real
files, fetched from pinned upstream archives and verified against recorded
SHA-256 digests. Three binary container formats, an uncompressed source tar, a
JSON dataset pretty-printed and minified, JavaScript, CSS and Python source, the
CommonMark specification, a Markdown changelog, a JPEG and a PNG. Using Base85N's
corpus unchanged is what makes the comparison against Base85N a comparison.

*text* means the JSON, code, spec and prose files (3.30 MB); *binary* means the
binaries, the archive and the images (3.21 MB).

**Codecs.** Base64 is Node's. Ascii85 and basE91 are in `bench/refcodecs.js` and
are checked to round-trip. Base85N is the upstream Go implementation, v0.5.1,
run by `bench/base85n` — not quoted from its documentation — and every file it
reports is decoded again before its size is used. base91-jdp is `src/`, with the
frozen constants of the specification.

**Ratio** is encoded characters per input byte. Lower is better; 1.000 means the
output is the same size as the input.

**Bold** marks the smallest output in a row.

## Two views of the same numbers

The first table is raw output size. The second is what the output costs once it
sits inside a JSON string, where `"` and `\` have to be escaped — which is the
number that matters if the output is going into JSON, and the reason this format
exists. Base85N and base91-jdp have identical figures in both tables, because
neither can emit a character JSON has to escape. Ascii85 and basE91 cannot say
that: both contain `"`, and basE91's alphabet is exactly base91-jdp's except for
that one character.


### Encoded characters per input byte

| sample | input | Base64 | Ascii85 | basE91 | Base85N | base91-jdp |
|---|---|---|---|---|---|---|
| sql-wasm.wasm | 659,730 B | 1.333 | 1.247 | 1.211 | 1.239 | **1.208** |
| _cffi_backend.so | 1,068,624 B | 1.333 | 1.026 | 1.190 | **0.965** | 1.187 |
| DejaVuSans.ttf | 756,072 B | 1.333 | 1.240 | 1.219 | 1.232 | **1.217** |
| requests-2.32.3.tar | 655,360 B | 1.333 | 1.015 | 1.206 | **0.767** | 1.046 |
| countries.json | 1,408,911 B | 1.333 | 1.250 | 1.220 | **0.935** | 1.000 |
| countries.min.json | 772,294 B | 1.333 | 1.250 | 1.231 | 1.003 | **1.000** |
| lodash.js | 544,098 B | 1.333 | 1.250 | 1.225 | 1.004 | **1.002** |
| bootstrap.css | 281,046 B | 1.333 | 1.250 | 1.228 | **1.003** | 1.060 |
| requests-models.py | 35,418 B | 1.333 | 1.250 | 1.224 | **0.973** | 1.001 |
| commonmark-spec.txt | 202,827 B | 1.333 | 1.250 | 1.229 | **0.859** | 1.007 |
| requests-history.md | 60,368 B | 1.333 | 1.250 | 1.229 | **0.979** | 1.028 |
| grace_hopper.jpg | 61,306 B | 1.333 | 1.250 | 1.229 | 1.249 | **1.229** |
| minduka_present.png | 13,634 B | 1.333 | 1.250 | **1.229** | 1.250 | **1.229** |
| text files | 3,304,962 B | 1.33333 | 1.25000 | 1.22461 | **0.96474** | 1.00655 |
| binary files | 3,214,726 B | 1.33334 | 1.12450 | 1.20546 | **1.05041** | 1.17091 |
| whole corpus | 6,519,688 B | 1.33333 | 1.18812 | 1.21517 | **1.00698** | 1.08759 |

### Characters per input byte once the output sits in a JSON string

| sample | input | Base64 | Ascii85 | basE91 | Base85N | base91-jdp |
|---|---|---|---|---|---|---|
| sql-wasm.wasm | 659,730 B | 1.333 | 1.291 | 1.246 | 1.239 | **1.208** |
| _cffi_backend.so | 1,068,624 B | 1.333 | 1.070 | 1.209 | **0.965** | 1.187 |
| DejaVuSans.ttf | 756,072 B | 1.333 | 1.287 | 1.241 | 1.232 | **1.217** |
| requests-2.32.3.tar | 655,360 B | 1.333 | 1.027 | 1.222 | **0.767** | 1.046 |
| countries.json | 1,408,911 B | 1.333 | 1.257 | 1.267 | **0.935** | 1.000 |
| countries.min.json | 772,294 B | 1.333 | 1.268 | 1.234 | 1.003 | **1.000** |
| lodash.js | 544,098 B | 1.333 | 1.265 | 1.248 | 1.004 | **1.002** |
| bootstrap.css | 281,046 B | 1.333 | 1.267 | 1.244 | **1.003** | 1.060 |
| requests-models.py | 35,418 B | 1.333 | 1.264 | 1.251 | **0.973** | 1.001 |
| commonmark-spec.txt | 202,827 B | 1.333 | 1.263 | 1.237 | **0.859** | 1.007 |
| requests-history.md | 60,368 B | 1.333 | 1.264 | 1.240 | **0.979** | 1.028 |
| grace_hopper.jpg | 61,306 B | 1.333 | 1.281 | 1.240 | 1.249 | **1.229** |
| minduka_present.png | 13,634 B | 1.333 | 1.281 | 1.239 | 1.250 | **1.229** |
| text files | 3,304,962 B | 1.33333 | 1.26249 | 1.25189 | **0.96474** | 1.00655 |
| binary files | 3,214,726 B | 1.33334 | 1.16265 | 1.22770 | **1.05041** | 1.17091 |
| whole corpus | 6,519,688 B | 1.33333 | 1.21326 | 1.23996 | **1.00698** | 1.08759 |


---

## Where base91-jdp stands

Against **basE91**, which it is a variant of: identical density, and 12.3 % less
inside a JSON string (1.08759 against 1.23996), because basE91's `"` has to be
escaped and passthrough is not in basE91 at all.

Against **Base64**: 18.4 % smaller over the whole corpus, 24.5 % on text.

Against **Base85N** the picture splits cleanly, and the split is the honest
summary of what this format is for:

| | Base85N | base91-jdp | |
|---|---|---|---|
| `sql-wasm.wasm` | 1.239 | **1.208** | −2.5 % |
| `DejaVuSans.ttf` | 1.232 | **1.217** | −1.2 % |
| `grace_hopper.jpg` | 1.249 | **1.229** | −1.6 % |
| `minduka_present.png` | 1.250 | **1.229** | −1.7 % |
| `countries.min.json` | 1.003 | **1.000** | −0.3 % |
| `lodash.js` | 1.004 | **1.002** | −0.2 % |
| `requests-models.py` | **0.973** | 1.001 | +2.9 % |
| `countries.json` | **0.935** | 1.000 | +7.0 % |
| `bootstrap.css` | **1.003** | 1.060 | +5.7 % |
| `commonmark-spec.txt` | **0.859** | 1.007 | +17.2 % |
| `requests-2.32.3.tar` | **0.767** | 1.046 | +36.4 % |
| whole corpus | **1.00698** | 1.08759 | +8.0 % |

base91-jdp wins **every file in the corpus that neither codec can compress** —
the WebAssembly module, the font, the JPEG and the PNG — and it wins them by
1.2 % to 2.5 %. That is the whole of the case for 91 characters over 85: where
both formats have run out of structure to exploit, the alphabet is the only
thing left. Two characters carrying 13 bits is 1.2308 per byte where five
carrying 32 is 1.25 — 1.5 % — and passthrough picks up the rest where a binary
happens to contain a stretch of ASCII.

It loses everywhere Base85N's **Fill** mode has runs to work with: the zero
padding in a block-aligned tar, the indentation in pretty-printed JSON, the
long space runs in a specification document. base91-jdp has no run-length
construct at all — see §15 of the specification, which reserves the signal
space for one and says why it is not in version 0.1.0.

`bootstrap.css` is a different loss and worth separating out: it is not about
Fill, it is about `--`. See the signal-character section below.

---

## Parameter sweeps

Each sweep varies one constant with the others at their frozen values
(`MIN_DP_BYTES` 26, `MIN_BINARY_RUN` 4, `MAX_DP_BYTES` 65 536, four profiles,
a two-character header).


### MIN_BINARY_RUN: block-mode bytes before passthrough may resume

| setting | text | binary | whole corpus |
|---|---|---|---|
| `MIN_BINARY_RUN` = 1 | 1.00659 | 1.17092 | 1.08762 |
| `MIN_BINARY_RUN` = 2 | 1.00657 | 1.17092 | 1.08761 |
| `MIN_BINARY_RUN` = 3 | 1.00656 | 1.17091 | 1.08760 |
| `MIN_BINARY_RUN` = 4 | 1.00655 | 1.17091 | **1.08759** |
| `MIN_BINARY_RUN` = 5 | 1.00709 | 1.17099 | 1.08791 |
| `MIN_BINARY_RUN` = 6 | 1.00707 | 1.17101 | 1.08791 |
| `MIN_BINARY_RUN` = 8 | 1.00703 | 1.17103 | 1.08789 |
| `MIN_BINARY_RUN` = 12 | 1.00740 | 1.17114 | 1.08814 |
| `MIN_BINARY_RUN` = 16 | 1.00767 | 1.17123 | 1.08832 |
| `MIN_BINARY_RUN` = 24 | 1.00827 | 1.17141 | 1.08871 |
| `MIN_BINARY_RUN` = 32 | 1.00879 | 1.17160 | 1.08907 |


`MIN_BINARY_RUN` is the constant this format was asked to justify. It answers:
once passthrough has been broken by a byte it cannot carry — most often `--`
itself — how many bytes must go through the block coder before passthrough may
resume?

**Four is the optimum, and it was guessed correctly.** But the margin over 1 is
0.003 %, and the shape of the table is more informative than its minimum. From
1 to 4 the parameter is doing nothing that `MIN_DP_BYTES` was not already
doing: a segment has to be worth its own six-character signal regardless, so
resuming passthrough after a single byte is not a mistake that needs
preventing. At 5 there is a step, and from there the constraint starts
overriding that judgement — forcing bytes through the block coder at 1.2308
that passthrough would have carried at 1.0.

So the answer to "is 4 the right choice" is: yes, and for a reason worth
knowing — 4 is the last value that changes nothing. It is the top of the
plateau, not a peak.


### MIN_DP_BYTES: shortest segment worth a passthrough signal

| setting | text | binary | whole corpus |
|---|---|---|---|
| `MIN_DP_BYTES` = 16 | 1.00681 | 1.17179 | 1.08815 |
| `MIN_DP_BYTES` = 18 | 1.00675 | 1.17141 | 1.08794 |
| `MIN_DP_BYTES` = 20 | 1.00668 | 1.17118 | 1.08779 |
| `MIN_DP_BYTES` = 22 | 1.00662 | 1.17105 | 1.08770 |
| `MIN_DP_BYTES` = 23 | 1.00660 | 1.17098 | 1.08765 |
| `MIN_DP_BYTES` = 24 | 1.00658 | 1.17095 | 1.08763 |
| `MIN_DP_BYTES` = 25 | 1.00656 | 1.17093 | 1.08761 |
| `MIN_DP_BYTES` = 26 | 1.00655 | 1.17091 | 1.08759 |
| `MIN_DP_BYTES` = 27 | 1.00655 | 1.17090 | 1.08758 |
| `MIN_DP_BYTES` = 28 | 1.00654 | 1.17089 | 1.08758 |
| `MIN_DP_BYTES` = 30 | 1.00654 | 1.17089 | **1.08758** |
| `MIN_DP_BYTES` = 32 | 1.00656 | 1.17090 | 1.08759 |
| `MIN_DP_BYTES` = 36 | 1.00666 | 1.17095 | 1.08767 |
| `MIN_DP_BYTES` = 40 | 1.00678 | 1.17101 | 1.08776 |
| `MIN_DP_BYTES` = 48 | 1.00705 | 1.17117 | 1.08797 |


Derived rather than fitted. A segment of `L` bytes costs `L + 6` characters —
two for the entry signal, two for the header, two for the exit — and block mode
charges at most `16/13` characters per byte, so `L + 6 ≤ 16L/13` gives `L ≥ 26`.
The measured optimum is 30, better by 0.001 %; the plateau runs from 26 to 32.
The derived value is the one in the specification.


### MAX_DP_BYTES: the encoder lookahead bound

| setting | text | binary | whole corpus |
|---|---|---|---|
| `MAX_DP_BYTES` = 256 | 1.03156 | 1.17455 | 1.10207 |
| `MAX_DP_BYTES` = 512 | 1.01883 | 1.17260 | 1.09465 |
| `MAX_DP_BYTES` = 1024 | 1.01250 | 1.17166 | 1.09098 |
| `MAX_DP_BYTES` = 2048 | 1.00935 | 1.17121 | 1.08916 |
| `MAX_DP_BYTES` = 4096 | 1.00781 | 1.17102 | 1.08829 |
| `MAX_DP_BYTES` = 8192 | 1.00709 | 1.17095 | 1.08788 |
| `MAX_DP_BYTES` = 16384 | 1.00675 | 1.17092 | 1.08770 |
| `MAX_DP_BYTES` = 65536 | 1.00655 | 1.17091 | 1.08759 |
| unbounded | 1.00652 | 1.17091 | **1.08758** |


Unlike Base85N, this format carries no length field, so nothing in the wire
format forces a bound — the constant exists only to keep the encoder's memory
finite and its output canonical. 65 536 is within 0.00001 of unbounded and
bounds the encoder's lookahead buffer at 64 KiB.


### NUM_PROFILES: donor rankings the header can select

| setting | text | binary | whole corpus |
|---|---|---|---|
| 1 profile | 1.00704 | 1.17104 | 1.08791 |
| 2 profiles | 1.00672 | 1.17098 | 1.08772 |
| 3 profiles | 1.00656 | 1.17096 | 1.08762 |
| 4 profiles | 1.00655 | 1.17091 | **1.08759** |

### Header width: what the passthrough signal carries

| setting | text | binary | whole corpus |
|---|---|---|---|
| 2 chars, exact mask, 4 profiles (this version) — best `MIN_DP_BYTES` 30 | 1.00654 | 1.17089 | **1.08758** |
| 2 chars, exact mask, 1 profile — best `MIN_DP_BYTES` 30 | 1.00703 | 1.17102 | 1.08789 |
| 2 chars, prefix mask, 4 profiles — best `MIN_DP_BYTES` 30 | 1.00802 | 1.17108 | 1.08842 |
| 1 char, prefix mask, 4 profiles — best `MIN_DP_BYTES` 24 | 1.00705 | 1.17069 | 1.08774 |
| 1 char, prefix mask, 1 profile — best `MIN_DP_BYTES` 25 | 1.00763 | 1.17125 | 1.08831 |
| 1 char, no mask, 4 profiles — best `MIN_DP_BYTES` 25 | 1.01352 | 1.17144 | 1.09139 |
| 1 char, no mask, 1 profile — best `MIN_DP_BYTES` 24 | 1.01711 | 1.17303 | 1.09399 |


This is the design decision the header sweep exists to settle, and it is closer
than it looks. Three ways to describe a segment's substitution were measured:

* **exact** — a 7-bit mask naming precisely which R-Set characters occur, so
  only that many donors are spent;
* **prefix** — three bits saying how many of the frequency-ordered R-Set
  characters are covered, so a rare one drags the commoner ones in with it;
* **none** — no mask at all, every segment spends all seven donors.

The exact mask needs a two-character header; the other two fit in one. The
two-character header wins by 0.02 % over the best one-character variant, which
is nearly nothing — but it also leaves 7 257 header values unassigned instead
of 33, and that is what §15 of the specification wants to spend on a
run-length mode. Both reasons point the same way.

The **none** rows are the interesting ones: giving up the mask entirely costs
0.4–0.6 %. That is the measured value of not spending a donor character on an
R-Set character the segment does not contain.

---

## The signal character


### Which doubled character ends a passthrough segment

| signal | occurrences of the pair in the corpus | text | binary | whole corpus |
|---|---|---|---|---|
| `--` | 10,553 | 1.00655 | 1.17091 | 1.08759 |
| ```` | 41,002 | 1.00584 | 1.17058 | 1.08707 |
| `||` | 303 | 1.00090 | 1.17027 | 1.08442 |
| `QQ` | 139 | 1.00053 | 1.17035 | **1.08426** |
| `##` | 203 | 1.00918 | 1.17077 | 1.08885 |

Candidates that appear in a donor profile are measured with the profiles that do not use them, so the rows are not perfectly like for like.


Which character sits on alphabet value 90 — and so which doubled character
becomes the mode signal — is a free choice once `"` is out of the alphabet.
Text containing that character doubled cannot be passed through, so the choice
has a price, and `--` is the most expensive of the plausible ones: 0.3 % over
`QQ`.

Nearly all of it is one file. `bootstrap.css` is 281 kB of CSS custom
properties, every single one of which begins `--`, and it accounts for 2 046 of
the 2 550 signal-caused segment breaks in the whole corpus. Strip that file out
and the choice costs about 0.05 %.

The format keeps `--` anyway:

* it is the character the JSON-safety swap put on value 90 in the first place —
  `"` had to go, `-` was the natural replacement, and `-` is basE91's other
  omission;
* it reads as a separator rather than as data, which is what a mode marker
  should look like in output a human may have to eyeball;
* a codec whose mode marker is `QQ` cannot be debugged by looking at it.

The 0.3 % is the price of that. It is measured, it is documented, and the
specification does not propose to change it.


### Why a passthrough segment ends

| sample | segments | bytes in passthrough | signal pair | byte outside the alphabet | no viable profile | lookahead cap |
|---|---|---|---|---|---|---|
| sql-wasm.wasm | 348 | 2.7 % | 0 | 347 | 1 | 0 |
| _cffi_backend.so | 362 | 2.3 % | 2 | 360 | 0 | 0 |
| DejaVuSans.ttf | 64 | 1.0 % | 2 | 59 | 3 | 0 |
| requests-2.32.3.tar | 461 | 72.7 % | 201 | 220 | 40 | 0 |
| countries.json | 50 | 100.0 % | 0 | 1 | 45 | 3 |
| countries.min.json | 12 | 100.0 % | 0 | 0 | 0 | 11 |
| lodash.js | 103 | 99.6 % | 69 | 1 | 32 | 0 |
| bootstrap.css | 2,051 | 93.2 % | 2,046 | 1 | 3 | 0 |
| requests-models.py | 3 | 100.0 % | 0 | 0 | 2 | 0 |
| commonmark-spec.txt | 153 | 99.1 % | 72 | 44 | 36 | 0 |
| requests-history.md | 160 | 95.4 % | 157 | 2 | 0 | 0 |
| grace_hopper.jpg | 2 | 0.2 % | 0 | 2 | 0 | 0 |
| minduka_present.png | 0 | 0.0 % | 0 | 0 | 0 | 0 |
| whole corpus | 3,769 | 58.4 % | 2,549 | 1,037 | 162 | 14 |


The three reasons a segment ends, over the whole corpus: 2 550 times on the
signal pair, 1 283 times because no donor profile could lend a character the
text did not already use, 1 028 times on a byte outside the alphabet — chiefly
UTF-8 above U+007F. The lookahead cap is never reached.

The `donor` column is where more profiles would help and the sweep above says
they would help by 0.003 %; the `unrepresentable` column is the one nothing in
this format can fix, since a multi-byte character has no single-character
representation.

---

## Donor profiles

Derived by `tools/deriveprofiles.js` on a 2.37 MB training corpus
(`tools/traincorpus.py`: jQuery, marked, handlebars, click, jinja2, rich) that
shares no file and no upstream project with the benchmark corpus. Profiles are
added one at a time, each chosen to minimise encoded size given the ones already
in the table; within a profile the donor positions are filled left to right.

| profiles | chars per byte, training | gain |
|---|---|---|
| 1 | 1.02083 | — |
| 2 | 1.01860 | 0.224 % |
| 3 | 1.01818 | 0.042 % |
| **4** | **1.01794** | **0.024 %** |
| 5 | 1.01783 | 0.011 % |
| 6 | 1.01776 | 0.007 % |

Four is where the curve flattens.

### Letters make bad donors, and the hold-out is how you find out

The first derivation run allowed any alphabet character into the candidate
pool, and it picked capitals: `Z K Y ~ % @ ^`. On the training corpus that beat
the best punctuation-only table by 0.1 %. On the benchmark corpus it lost by
0.1 %:

Both tables below have four profiles, derived the same way, differing only in
what the candidate pool allowed:

| donor pool | training corpus | benchmark corpus |
|---|---|---|
| any character (`ZKY~%@^`, …) | **1.01753** | 1.08877 |
| punctuation only (`^~$%@#<`, …) | 1.01794 | **1.08759** |

The mechanism is not subtle once seen. `^ ~ % @ $` are rare when all text is
counted together and *concentrated* where they occur at all — jQuery is full of
`$`, jinja2 templates of `%`, CSS of `^`. A rare capital is rare everywhere and
still common in the one file that happens to use it, because identifiers and
words are made of letters. Over a training corpus of six projects the capitals
looked safe; over thirteen unrelated files they broke segments in bursts.

Letters and digits are now excluded from the pool on principle, and the
principle is stated in the tool rather than in the table it produced.
