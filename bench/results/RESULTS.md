# base91-jdp — measurements

Everything here is measured, not quoted. Reproduce it with:

```bash
python3 bench/corpus.py     # fetch the corpus (pinned, SHA-256 verified)
node bench/bench.js         # the size tables
node bench/sweep.js         # the parameter sweeps
node bench/signalchar.js    # the signal character, and why segments end
node bench/fillbound.js     # what a run-length mode would be worth
node bench/gzipdecision.js  # when to deflate first, and how few bytes decide it
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
| requests-2.32.3.tar | 655,360 B | 1.333 | 1.015 | 1.206 | **0.767** | 1.044 |
| countries.json | 1,408,911 B | 1.333 | 1.250 | 1.220 | **0.935** | 1.000 |
| countries.min.json | 772,294 B | 1.333 | 1.250 | 1.231 | 1.003 | **1.000** |
| lodash.js | 544,098 B | 1.333 | 1.250 | 1.225 | 1.004 | **1.001** |
| bootstrap.css | 281,046 B | 1.333 | 1.250 | 1.228 | 1.003 | **1.001** |
| requests-models.py | 35,418 B | 1.333 | 1.250 | 1.224 | **0.973** | 1.002 |
| commonmark-spec.txt | 202,827 B | 1.333 | 1.250 | 1.229 | **0.859** | 1.005 |
| requests-history.md | 60,368 B | 1.333 | 1.250 | 1.229 | **0.979** | 1.002 |
| grace_hopper.jpg | 61,306 B | 1.333 | 1.250 | 1.229 | 1.249 | **1.229** |
| minduka_present.png | 13,634 B | 1.333 | 1.250 | **1.229** | 1.250 | **1.229** |
| text files | 3,304,962 B | 1.33333 | 1.25000 | 1.22461 | **0.96474** | 1.00081 |
| binary files | 3,214,726 B | 1.33334 | 1.12450 | 1.20546 | **1.05041** | 1.17037 |
| whole corpus | 6,519,688 B | 1.33333 | 1.18812 | 1.21517 | **1.00698** | 1.08442 |

### Characters per input byte once the output sits in a JSON string

| sample | input | Base64 | Ascii85 | basE91 | Base85N | base91-jdp |
|---|---|---|---|---|---|---|
| sql-wasm.wasm | 659,730 B | 1.333 | 1.291 | 1.246 | 1.239 | **1.208** |
| _cffi_backend.so | 1,068,624 B | 1.333 | 1.070 | 1.209 | **0.965** | 1.187 |
| DejaVuSans.ttf | 756,072 B | 1.333 | 1.287 | 1.241 | 1.232 | **1.217** |
| requests-2.32.3.tar | 655,360 B | 1.333 | 1.027 | 1.222 | **0.767** | 1.044 |
| countries.json | 1,408,911 B | 1.333 | 1.257 | 1.267 | **0.935** | 1.000 |
| countries.min.json | 772,294 B | 1.333 | 1.268 | 1.234 | 1.003 | **1.000** |
| lodash.js | 544,098 B | 1.333 | 1.265 | 1.248 | 1.004 | **1.001** |
| bootstrap.css | 281,046 B | 1.333 | 1.267 | 1.244 | 1.003 | **1.001** |
| requests-models.py | 35,418 B | 1.333 | 1.264 | 1.251 | **0.973** | 1.002 |
| commonmark-spec.txt | 202,827 B | 1.333 | 1.263 | 1.237 | **0.859** | 1.005 |
| requests-history.md | 60,368 B | 1.333 | 1.264 | 1.240 | **0.979** | 1.002 |
| grace_hopper.jpg | 61,306 B | 1.333 | 1.281 | 1.240 | 1.249 | **1.229** |
| minduka_present.png | 13,634 B | 1.333 | 1.281 | 1.239 | 1.250 | **1.229** |
| text files | 3,304,962 B | 1.33333 | 1.26249 | 1.25189 | **0.96474** | 1.00081 |
| binary files | 3,214,726 B | 1.33334 | 1.16265 | 1.22770 | **1.05041** | 1.17037 |
| whole corpus | 6,519,688 B | 1.33333 | 1.21326 | 1.23996 | **1.00698** | 1.08442 |


---

## Where base91-jdp stands

Against **basE91** — the format it is a variant of — the swap costs nothing and
the container saves **12.5 %**: 1.08442 against 1.23996 inside a JSON string.
Same algorithm, same density, one alphabet character different, and no escaping.

Against **Base64**: 18.7 % smaller over the corpus, 25.0 % on text.

Against **Base85N** the picture splits, and the split is the honest summary of
what this format is for:

| | Base85N | base91-jdp | |
|---|---|---|---|
| `sql-wasm.wasm` | 1.239 | **1.208** | −2.5 % |
| `DejaVuSans.ttf` | 1.232 | **1.217** | −1.2 % |
| `grace_hopper.jpg` | 1.249 | **1.229** | −1.6 % |
| `minduka_present.png` | 1.250 | **1.229** | −1.7 % |
| `bootstrap.css` | 1.003 | **1.001** | −0.2 % |
| `countries.min.json` | 1.003 | **1.000** | −0.3 % |
| `lodash.js` | 1.004 | **1.001** | −0.3 % |
| `countries.json` | **0.935** | 1.000 | +7.0 % |
| `requests-history.md` | **0.979** | 1.002 | +2.3 % |
| `requests-models.py` | **0.973** | 1.002 | +3.0 % |
| `commonmark-spec.txt` | **0.859** | 1.005 | +17.0 % |
| `requests-2.32.3.tar` | **0.767** | 1.044 | +36.1 % |
| whole corpus | **1.00698** | 1.08442 | +7.7 % |

base91-jdp wins **every file in the corpus that neither codec can compress** —
the WebAssembly module, the font, the JPEG and the PNG — by 1.2 % to 2.5 %. Where
both formats have run out of structure to exploit, the alphabet is all that is
left. Two characters carrying 13 bits is 1.2308 per byte where five carrying 32
is 1.25 — 1.5 % — and passthrough picks up the rest where a binary happens to
contain a stretch of ASCII.

Every remaining loss is the same loss: Base85N's **Fill** mode has runs to work
with and base91-jdp has no run-length construct at all. The zero padding in a
block-aligned tar, the indentation in pretty-printed JSON, the long space runs
in a specification document. See *What a Fill mode would be worth* below, and
§15 of the specification, which reserves the header space for one.

---

## Parameter sweeps

Each sweep varies one constant with the others at their frozen values
(`MIN_DP_BYTES` 26, `MIN_BINARY_RUN` 4, `MAX_DP_BYTES` 65 536, four profiles,
a two-character header).


### MIN_BINARY_RUN: block-mode bytes before passthrough may resume

| setting | text | binary | whole corpus |
|---|---|---|---|
| `MIN_BINARY_RUN` = 1 | 1.00082 | 1.17038 | 1.08443 |
| `MIN_BINARY_RUN` = 2 | 1.00081 | 1.17037 | 1.08442 |
| `MIN_BINARY_RUN` = 3 | 1.00081 | 1.17037 | 1.08442 |
| `MIN_BINARY_RUN` = 4 | 1.00081 | 1.17037 | **1.08442** |
| `MIN_BINARY_RUN` = 5 | 1.00089 | 1.17046 | 1.08450 |
| `MIN_BINARY_RUN` = 6 | 1.00090 | 1.17049 | 1.08452 |
| `MIN_BINARY_RUN` = 8 | 1.00090 | 1.17050 | 1.08452 |
| `MIN_BINARY_RUN` = 12 | 1.00099 | 1.17062 | 1.08463 |
| `MIN_BINARY_RUN` = 16 | 1.00107 | 1.17072 | 1.08472 |
| `MIN_BINARY_RUN` = 24 | 1.00125 | 1.17086 | 1.08488 |
| `MIN_BINARY_RUN` = 32 | 1.00141 | 1.17100 | 1.08503 |


`MIN_BINARY_RUN` is the constant this format was asked to justify. It answers:
once passthrough has been broken by a byte it cannot carry, how many bytes must
go through the block coder before passthrough may resume?

**Four is the optimum, and it was guessed correctly.** But the margin over 1 is
0.001 %, and the shape of the table is more informative than its minimum. From
1 to 4 the parameter is doing nothing that `MIN_DP_BYTES` was not already
doing: a segment has to be worth its own six-character signal regardless, so
resuming passthrough after a single byte is not a mistake that needs
preventing. At 5 there is a step, and from there the constraint starts
overriding that judgement — forcing bytes through the block coder at 1.2308
that passthrough would have carried at 1.0.

So the answer to "is 4 the right choice" is: yes, and for a reason worth
knowing — 4 is the last value that changes nothing. It is the top of the
plateau, not a peak. The table has the same shape it had in v0.1.0, before `-`
joined the R-Set cut the number of breaks by two thirds, which is some evidence
that the shape is a property of the trade-off rather than of the corpus.


### MIN_DP_BYTES: shortest segment worth a passthrough signal

| setting | text | binary | whole corpus |
|---|---|---|---|
| `MIN_DP_BYTES` = 16 | 1.00081 | 1.17122 | 1.08484 |
| `MIN_DP_BYTES` = 18 | 1.00081 | 1.17086 | 1.08466 |
| `MIN_DP_BYTES` = 20 | 1.00081 | 1.17064 | 1.08455 |
| `MIN_DP_BYTES` = 22 | 1.00081 | 1.17051 | 1.08449 |
| `MIN_DP_BYTES` = 23 | 1.00081 | 1.17044 | 1.08445 |
| `MIN_DP_BYTES` = 24 | 1.00081 | 1.17041 | 1.08444 |
| `MIN_DP_BYTES` = 25 | 1.00081 | 1.17039 | 1.08443 |
| `MIN_DP_BYTES` = 26 | 1.00081 | 1.17037 | 1.08442 |
| `MIN_DP_BYTES` = 27 | 1.00081 | 1.17036 | 1.08441 |
| `MIN_DP_BYTES` = 28 | 1.00081 | 1.17035 | **1.08441** |
| `MIN_DP_BYTES` = 30 | 1.00081 | 1.17035 | 1.08441 |
| `MIN_DP_BYTES` = 32 | 1.00081 | 1.17036 | 1.08441 |
| `MIN_DP_BYTES` = 36 | 1.00081 | 1.17041 | 1.08444 |
| `MIN_DP_BYTES` = 40 | 1.00081 | 1.17047 | 1.08447 |
| `MIN_DP_BYTES` = 48 | 1.00082 | 1.17062 | 1.08454 |


Derived rather than fitted. A segment of `L` bytes costs `L + 6` characters —
two for the entry signal, two for the header, two for the exit — and block mode
charges at most `16/13` characters per byte, so `L + 6 ≤ 16L/13` gives `L ≥ 26`.
The measured optimum is 28, better by 0.0006 %; the plateau runs from 26 to 32.
The derived value is the one in the specification.


### MAX_DP_BYTES: the encoder lookahead bound

| setting | text | binary | whole corpus |
|---|---|---|---|
| `MAX_DP_BYTES` = 256 | 1.02706 | 1.17412 | 1.09957 |
| `MAX_DP_BYTES` = 512 | 1.01374 | 1.17210 | 1.09182 |
| `MAX_DP_BYTES` = 1024 | 1.00702 | 1.17112 | 1.08793 |
| `MAX_DP_BYTES` = 2048 | 1.00368 | 1.17065 | 1.08601 |
| `MAX_DP_BYTES` = 4096 | 1.00207 | 1.17046 | 1.08510 |
| `MAX_DP_BYTES` = 8192 | 1.00134 | 1.17038 | 1.08469 |
| `MAX_DP_BYTES` = 16384 | 1.00100 | 1.17037 | 1.08451 |
| `MAX_DP_BYTES` = 65536 | 1.00081 | 1.17037 | 1.08442 |
| unbounded | 1.00080 | 1.17037 | **1.08441** |


Unlike Base85N, this format carries no length field, so nothing in the wire
format forces a bound — the constant exists only to keep the encoder's memory
finite and its output canonical. 65 536 is within 0.00001 of unbounded and
bounds the encoder's lookahead buffer at 64 KiB. It now binds five times over
the whole corpus, where in v0.1.0 it never bound at all: segments got long
enough for it to matter once `--` stopped ending them.


### NUM_PROFILES: donor rankings the header can select

| setting | text | binary | whole corpus |
|---|---|---|---|
| 1 profile | 1.00129 | 1.17070 | 1.08482 |
| 2 profiles | 1.00094 | 1.17048 | 1.08454 |
| 3 profiles | 1.00091 | 1.17047 | 1.08451 |
| 4 profiles | 1.00081 | 1.17037 | **1.08442** |

### Header width: what the passthrough signal carries

| setting | text | binary | whole corpus |
|---|---|---|---|
| 2 chars, exact mask, 4 profiles (this version) — best `MIN_DP_BYTES` 29 | 1.00081 | 1.17035 | **1.08441** |
| 2 chars, exact mask, 1 profile — best `MIN_DP_BYTES` 29 | 1.00129 | 1.17068 | 1.08481 |
| 2 chars, prefix mask, 4 profiles — best `MIN_DP_BYTES` 29 | 1.00415 | 1.17059 | 1.08622 |
| 1 char, prefix mask, 4 profiles — best `MIN_DP_BYTES` 25 | 1.00358 | 1.17024 | 1.08575 |
| 1 char, prefix mask, 1 profile — best `MIN_DP_BYTES` 26 | 1.00592 | 1.17137 | 1.08750 |
| 1 char, no mask, 4 profiles — best `MIN_DP_BYTES` 25 | 1.00888 | 1.17070 | 1.08867 |
| 1 char, no mask, 1 profile — best `MIN_DP_BYTES` 25 | 1.01467 | 1.17271 | 1.09259 |


Three ways to describe a segment's substitution were measured:

* **exact** — an 8-bit mask naming precisely which R-Set characters occur, so
  only that many donors are spent;
* **prefix** — four bits saying how many of the frequency-ordered R-Set
  characters are covered, so a rare one drags the commoner ones in with it;
* **none** — no mask at all, every segment spends all eight donors.

The exact mask needs a two-character header; the other two fit in one. It wins
by 0.12 %, and it leaves 6 233 header values unassigned instead of a handful,
which is what §15 wants to spend on a run-length mode. Both reasons point the
same way — and the margin is six times what it was in v0.1.0, because with `-`
in the R-Set the mask has eight bits to be exact about and hyphens are common
enough that being exact pays.

The **none** rows are the interesting ones: giving up the mask entirely costs
0.4–0.8 %. That is the measured value of not spending a donor character on an
R-Set character the segment does not contain.

---

## `-` in the R-Set

Version 0.1.0 kept `-` out of the R-Set and handled the collision structurally:
a doubled hyphen in the input ended the passthrough segment, and a segment was
forbidden to end on a single one. Version 0.2.0 substitutes it like any other
R-Set member.

| | 0.1.0, `-` a literal | 0.2.0, `-` substituted |
|---|---|---|
| `bootstrap.css` | 1.060 | **1.001** |
| `requests-history.md` | 1.028 | **1.002** |
| `commonmark-spec.txt` | 1.007 | **1.005** |
| text corpus | 1.00654 | **1.00081** |
| whole corpus | 1.08758 | **1.08442** |

Passthrough segments over the whole corpus: **4 867 → 1 461**. Segments ended by
the signal pair: **2 550 → 0**, by construction. `bootstrap.css` alone went from
2 048 segments to 30.

The cost is one donor character in every segment that contains a hyphen, which
is most of them, and the 0.29 % is net of that. What it also buys is two fewer
special cases: the scan no longer has to stop at a doubled hyphen, and a segment
no longer has to be shortened when it would end on a single one. A payload
simply cannot contain the signal character.

---

## The signal character


### Which doubled character ends a passthrough segment

| signal | occurrences of the pair in the corpus | text | binary | whole corpus |
|---|---|---|---|---|
| `--` | 10,553 | 1.00081 | 1.17037 | 1.08442 |
| `~~` | 328 | 1.00202 | 1.17047 | 1.08508 |
| ```` | 41,002 | 1.00067 | 1.17034 | 1.08433 |
| `^^` | 38 | 1.00121 | 1.17041 | 1.08464 |
| `||` | 303 | 1.00059 | 1.17029 | **1.08426** |
| `@@` | 347 | 1.00091 | 1.17037 | 1.08446 |
| `QQ` | 139 | 1.00059 | 1.17030 | 1.08427 |
| `##` | 203 | 1.00068 | 1.17038 | 1.08436 |

A candidate that appears in a donor profile takes the place of `-` there, since `-` is an ordinary alphabet character once it is not the signal.


Which character sits on alphabet value 90 — and so which doubled character
becomes the mode signal — is a free choice once `"` is out of the alphabet. Each
candidate above is measured with R-Set membership moving with it, which is what
makes the rows comparable.

**The spread is 0.08 %, and `--` sits 0.015 % off the best.** That is the point
of the table rather than an aside. The same measurement in v0.1.0 put `--` 0.3 %
behind `QQ`, and that was the strongest argument for picking a different
character. Substituting `-` closed the gap, so the choice is now settled on the
grounds it should have been settled on all along: `-` is what the JSON-safety
swap put on value 90, and a doubled hyphen reads as a separator where `QQ` reads
as data.


### Why a passthrough segment ends

| sample | segments | bytes in passthrough | signal pair | byte outside the alphabet | no viable profile | lookahead cap |
|---|---|---|---|---|---|---|
| sql-wasm.wasm | 349 | 2.7 % | 0 | 347 | 2 | 0 |
| _cffi_backend.so | 361 | 2.3 % | 0 | 361 | 0 | 0 |
| DejaVuSans.ttf | 57 | 1.0 % | 0 | 54 | 3 | 0 |
| requests-2.32.3.tar | 335 | 73.3 % | 0 | 237 | 98 | 0 |
| countries.json | 78 | 100.0 % | 0 | 1 | 74 | 2 |
| countries.min.json | 49 | 100.0 % | 0 | 0 | 47 | 1 |
| lodash.js | 71 | 99.9 % | 0 | 1 | 68 | 1 |
| bootstrap.css | 30 | 100.0 % | 0 | 1 | 27 | 1 |
| requests-models.py | 10 | 99.9 % | 0 | 0 | 9 | 0 |
| commonmark-spec.txt | 104 | 99.4 % | 0 | 43 | 60 | 0 |
| requests-history.md | 15 | 99.9 % | 0 | 4 | 10 | 0 |
| grace_hopper.jpg | 2 | 0.2 % | 0 | 2 | 0 | 0 |
| minduka_present.png | 0 | 0.0 % | 0 | 0 | 0 | 0 |
| whole corpus | 1,461 | 58.8 % | 0 | 1,051 | 398 | 5 |


Three reasons a segment can end, and one of them is now extinct: **0** breaks on
the signal pair, 1 051 on a byte outside the alphabet — chiefly UTF-8 above
U+007F — and 398 because no donor profile could lend a character the text did
not already use. The lookahead cap binds five times.

The `donor` column is where more profiles would help, and the sweep above says
they would help by about 0.001 %. The `unrepresentable` column is the one
nothing in this format can fix: a multi-byte character has no single-character
representation.

---

## The run-length mode this format does not have

Every loss against Base85N in the table above is its Fill mode. This section
bounds what closing that gap would be worth, and then measures the alternative,
which is why the gap stays open.

The bound assumes each maximal run of five or more identical bytes could be
carried by a five-character signal covering up to 2 048 bytes, and that those
bytes currently cost what the encoder actually spends on them (1.0 inside a
passthrough segment, 16/13 in block mode). It is generous: it ignores the cost
of leaving and re-entering passthrough around a run inside a segment, and it
assumes lengths land exactly.

| sample | now | runs >= 5 | bytes in runs | bound with Fill | deflate first | Base85N |
|---|---|---|---|---|---|---|
| sql-wasm.wasm | 1.208 | 864 | 1.0 % | 1.203 | **0.602** | 1.239 |
| _cffi_backend.so | 1.187 | 51,534 | 34.9 % | 1.001 | **0.428** | 0.965 |
| DejaVuSans.ttf | 1.217 | 412 | 1.2 % | 1.206 | **0.627** | 1.232 |
| requests-2.32.3.tar | 1.044 | 7,819 | 34.3 % | 0.739 | **0.245** | 0.767 |
| countries.json | 1.000 | 41,072 | 36.9 % | 0.777 | **0.124** | 0.935 |
| countries.min.json | 1.000 | 87 | 0.1 % | 1.000 | **0.202** | 1.003 |
| lodash.js | 1.001 | 13,064 | 14.4 % | 0.977 | **0.219** | 1.004 |
| bootstrap.css | 1.001 | 108 | 0.3 % | 1.000 | **0.144** | 1.003 |
| requests-models.py | 1.002 | 694 | 21.1 % | 0.889 | **0.343** | 0.973 |
| commonmark-spec.txt | 1.005 | 1,450 | 21.1 % | 0.829 | **0.274** | 0.859 |
| requests-history.md | 1.002 | 161 | 4.8 % | 0.967 | **0.406** | 0.979 |
| grace_hopper.jpg | 1.229 | 3 | 0.1 % | 1.228 | **1.226** | 1.249 |
| minduka_present.png | 1.229 | 0 | 0.0 % | 1.229 | **1.229** | 1.25 |
| whole corpus | 1.08442 | | | 0.96471 | **0.33194** | 1.00698 |

The deflate column takes the smaller of the two paths per file, which is
what the rule in bench/gzipdecision.js chooses.

Read the last three columns across. **The generous bound on a Fill mode reaches
0.965; deflating the same corpus first reaches 0.332.** On `countries.json` —
the file Fill was most obviously for, 36.9 % of it space runs eight to
thirty-one long — Fill would reach 0.777 and deflate reaches 0.124, six times
better, because deflate also has something to say about the thousand repetitions
of `"official"` that a run-length mode cannot see.

So the construct is worth about a fifth of what a call to zlib is worth, on
exactly the payloads where zlib is available. It would also cost the property
that Section 13 of the specification currently gets for free: a decoder that
cannot be made to write more than it reads needs no expansion bound, and one
with a run-length mode does.

**And compression is where base91-jdp stops splitting the decision at all.**
Deflate the corpus and encode the result:

| whole corpus, deflate -9 then encoded | characters per input byte |
|---|---|
| Base85N | 0.33741 |
| **base91-jdp** | **0.33194** |

1.6 % smaller, across the board rather than on four files. Once the payload is
incompressible the alphabet is the only thing left, and 91 characters carry
13 bits per two where 85 carry 32 per five. Compression turns every payload into
the case base91-jdp already won.

The remaining question is therefore not whether to build Fill. It is when to
compress — measured next.

---

## When to deflate first, and how few bytes decide it

base91-jdp has no run-length or dictionary construct on purpose. A payload with
structure in it belongs to a real compressor, and deflate output is
incompressible, so it encodes at a flat 1.2308 — the case this format is best
at. What that leaves the caller is one decision, and this section measures how
cheaply it can be made.

Passthrough carries text at 1.0, so deflating pays exactly when deflate
compresses to below `1 / 1.2308 = 81.25 %` of what the direct path would have
charged. The rule under test estimates both rates from the first N bytes:

```
a = |encode(prefix)|     / N      the direct cost per byte
b = |deflateRaw(prefix)| / N      the compression ratio
deflate  iff  (16/13) · b · len  <  a · len
```

A short prefix understates compression, because a cold dictionary finds no
long-range matches, so the rule leans towards encoding directly. Buying that
bias off is what N is for.

### Rule A: deflate the prefix, encode the prefix

| bytes inspected | payloads | correct | compressible | borderline | incompressible | near-ties correct | bytes lost vs the oracle |
|---|---|---|---|---|---|---|---|
| 32 | 6174 | 38.4 % | 31.6 % | 69.2 % | 99.5 % | 57.7 % | 127.796 % |
| 64 | 5310 | 43.8 % | 42.8 % | 43.5 % | 100.0 % | 51.3 % | 94.594 % |
| 128 | 4446 | 89.2 % | 89.4 % | 40.7 % | 98.6 % | 84.4 % | 21.271 % |
| 256 | 3582 | 99.4 % | 99.7 % | 33.3 % | 98.3 % | 89.4 % | 1.928 % |
| 512 | 2718 | 99.8 % | 100.0 % | 40.0 % | 100.0 % | 88.9 % | 0.002 % |
| 1024 | 1890 | 99.7 % | 100.0 % | 25.0 % | 100.0 % | 85.7 % | 0.002 % |
| 2048 | 1254 | 99.5 % | 100.0 % | 14.3 % | 100.0 % | 80.0 % | 0.002 % |
| 4096 | 768 | 99.2 % | 100.0 % | 0.0 % | 100.0 % | 66.7 % | 0.002 % |
| 8192 | 414 | 98.6 % | 100.0 % | 0.0 % | -- | 0.0 % | 0.002 % |
| 16384 | 168 | 100.0 % | 100.0 % | -- | -- | -- | 0.000 % |

### Rule B: deflate the prefix, estimate the direct cost by a byte scan

No encoding at all -- just the share of bytes passthrough can carry.

| bytes inspected | payloads | correct | compressible | borderline | incompressible | near-ties correct | bytes lost vs the oracle |
|---|---|---|---|---|---|---|---|
| 32 | 6174 | 24.4 % | 14.9 % | 70.6 % | 100.0 % | 54.9 % | 150.035 % |
| 64 | 5310 | 31.5 % | 29.7 % | 42.2 % | 100.0 % | 50.7 % | 117.314 % |
| 128 | 4446 | 77.9 % | 77.9 % | 25.9 % | 100.0 % | 85.6 % | 42.154 % |
| 256 | 3582 | 98.9 % | 99.2 % | 26.7 % | 100.0 % | 90.9 % | 5.370 % |
| 512 | 2718 | 99.5 % | 99.8 % | 10.0 % | 100.0 % | 88.9 % | 0.416 % |
| 1024 | 1890 | 99.7 % | 100.0 % | 25.0 % | 100.0 % | 85.7 % | 0.002 % |
| 2048 | 1254 | 99.5 % | 100.0 % | 14.3 % | 100.0 % | 80.0 % | 0.002 % |
| 4096 | 768 | 99.2 % | 100.0 % | 0.0 % | 100.0 % | 66.7 % | 0.002 % |
| 8192 | 414 | 98.6 % | 100.0 % | 0.0 % | -- | 0.0 % | 0.002 % |
| 16384 | 168 | 100.0 % | 100.0 % | -- | -- | -- | 0.000 % |

### What the right answer actually is, by payload size

| size | payloads | compressible | borderline | incompressible | overall |
|---|---|---|---|---|---|
| 64 B | 864 | 100 % (234) | 16 % (533) | 0 % (97) | 37 % (864) |
| 128 B | 864 | 100 % (639) | 58 % (210) | 0 % (15) | 88 % (864) |
| 256 B | 864 | 100 % (839) | 83 % (12) | 0 % (13) | 98 % (864) |
| 512 B | 864 | 100 % (847) | 100 % (5) | 0 % (12) | 99 % (864) |
| 1024 B | 828 | 100 % (814) | 100 % (2) | 0 % (12) | 99 % (828) |
| 2048 B | 636 | 100 % (623) | 100 % (1) | 0 % (12) | 98 % (636) |
| 4096 B | 486 | 100 % (473) | 100 % (1) | 0 % (12) | 98 % (486) |
| 8192 B | 354 | 100 % (342) | -- | 0 % (12) | 97 % (354) |
| 16384 B | 246 | 100 % (240) | 100 % (6) | -- | 100 % (246) |
| 65536 B | 114 | 100 % (114) | -- | -- | 100 % (114) |
| 262144 B | 54 | 100 % (54) | -- | -- | 100 % (54) |

Share of payloads where deflating first gives the smaller output, with the
number of payloads in that cell in brackets.

### How far apart the two paths are

| payload | share | median gap | 90th percentile gap | share within 5 % |
|---|---|---|---|---|
| compressible | 85 % (5219) | 72.8 % | 218.0 % | 1 % |
| borderline | 12 % (770) | 6.1 % | 14.7 % | 44 % |
| incompressible | 3 % (185) | 7.6 % | 20.6 % | 49 % |

**512 bytes is where the curve stops moving**: 99.8 % of decisions correct and
0.002 % of bytes lost against a perfect oracle, against 1.9 % at 256 and 21 % at
128. Rule B, which skips the encode and estimates the direct cost from the share
of bytes passthrough could carry, needs 1 024 bytes to reach the same place — the
encode is worth doing.

Three things the tables say that the headline does not:

* **The decision is mostly "does deflate compress this at all".** Where deflate
  reaches 0.8, it wins at every size from 64 bytes up; where deflate expands, it
  never wins. Only the 0.8–1.0 band depends on payload size, and only below 256
  bytes.
* **The residual 0.002 % is not the rule being imprecise, it is the mistakes
  being cheap.** Every remaining error is a near-tie: where deflate neither
  compresses nor expands, the two paths land within a few per cent of each
  other. The decisions that carry weight are the ones the rule gets right.
* **Below about 4 KB there is nothing to predict.** Deflating 4 KB takes
  0.046 ms; running both paths and picking the smaller costs less than the
  branch is worth arguing about. The probe earns its keep on large payloads:
  0.038 ms for 512 bytes against 74.6 ms to deflate a 1.4 MB payload outright.


---

## Donor profiles

Derived by `tools/deriveprofiles.js` on a 2.37 MB training corpus
(`tools/traincorpus.py`: jQuery, marked, handlebars, click, jinja2, rich) that
shares no file and no upstream project with the benchmark corpus. Profiles are
added one at a time, each chosen to minimise encoded size given the ones already
in the table; within a profile the donor positions are filled left to right.

| profiles | chars per byte, training | gain |
|---|---|---|
| 1 | 1.02005 | — |
| 2 | 1.01760 | 0.245 % |
| 3 | 1.01693 | 0.067 % |
| **4** | **1.01643** | **0.050 %** |
| 5 | 1.01630 | 0.013 % |
| 6 | 1.01611 | 0.019 % |

Four is where the curve flattens; on the hold-out corpus the fifth and sixth are
worth 0.001 % between them.

### Letters make bad donors, and the hold-out is how you find out

The first derivation run allowed any alphabet character into the candidate pool,
and it picked capitals: `Z K Y ~ % @ ^`. Both tables below have four profiles,
derived the same way, differing only in what the pool allowed:

| donor pool | training corpus | benchmark corpus |
|---|---|---|
| any character (`ZKY~%@^`, …) | **1.01753** | 1.08877 |
| punctuation only (`^~$%@#<`, …) | 1.01794 | **1.08759** |

(Both measured under v0.1.0's seven-member R-Set, which is where the question
came up.)

The mechanism is not subtle once seen. `^ ~ % @ $` are rare when all text is
counted together and *concentrated* where they occur at all — jQuery is full of
`$`, jinja2 templates of `%`, CSS of `^`. A rare capital is rare everywhere and
still common in the one file that happens to use it, because identifiers and
words are made of letters. Over a training corpus of six projects the capitals
looked safe; over thirteen unrelated files they broke segments in bursts.

Letters and digits are now excluded from the pool on principle, and the
principle is stated in the tool rather than in the table it produced.
