# base91-jdp: basE91 on a JSON-safe alphabet, with Dynamic Passthrough

| Field | Value |
|---|---|
| Version | 0.2.0 |
| Status | Draft |
| Date | 2026-08-22 |
| License | MPL-2.0 |

> **Draft.** The wire format described here is complete and implemented, but it
> has not been in the field. Section 9 keeps 6 233 of the 8 281 header values
> free, and a future version is expected to spend some of them (Section 15).
>
> **Changed in 0.2.0.** `-` joined the R-Set (Section 4.2). It is the one R-Set
> member that *is* in the alphabet: it is substituted not because it cannot be
> written but because two of them in a row would end the segment. A payload
> therefore never contains `-` at all, which retires two special cases from the
> prefix scan and takes text full of `--` — CSS custom properties, Markdown
> rules, command lines — from a mode switch per occurrence to one donor per
> segment. Worth 0.29 % over the benchmark corpus and 5.6 % on `bootstrap.css`
> (Section 14.5).

---

## 1. Abstract

base91-jdp represents arbitrary data as text for the case where the result has
to be embedded in **JSON** — an API payload, a log field, a database text
column, a document inside a document — and where the size of that result
matters.

Its block coder is basE91 (Joachim Henke, 2005) with one substitution in the
alphabet: `"` is dropped and `-` takes its place. basE91 already omits `\` and
`'`; with `"` gone as well, the alphabet contains none of the characters a JSON
string has to escape, so encoded output can be pasted between quotation marks
verbatim and the encoded size *is* the final size.

The substitution is not free of consequence, and the consequence is the second
half of the format. `-` lands on the alphabet's last value, 90, which makes the
pair `--` the one two-character value the block coder can never produce. That
value is spent on a mode signal: `--` switches **Dynamic Passthrough (DP)** on,
and `--` switches it off again. In passthrough, text-like input is written one
output character per input byte instead of being expanded 1.25×, with the seven
byte values that real text contains and the alphabet does not — space, `"`,
newline, `\`, carriage return, `'`, tab — carried as stand-ins borrowed from the
alphabet's rarest characters. `-` is carried the same way, so that the signal
can never occur inside a segment. Anything passthrough cannot carry falls back
to the block coder.

---

## 2. Introduction

### 2.1 Design summary

* **Block mode** is basE91 with its 14-bit threshold lowered by one, which
  removes exactly one of 8 281 pair values from its range. Two characters carry
  13 or 14 bits, chosen from the data, so no padding and no length prefix is
  needed.
* **The removed value, 8 280, is the pair `--`.** In block mode it means "enter
  passthrough"; inside a passthrough segment it means "leave passthrough".
* A passthrough segment is introduced by `--`, a two-character header naming
  the substitution the segment uses, and the bits the block coder still had in
  hand; it is ended by `--`, or by the end of the input.
* **Binary fallback.** A byte the segment cannot carry ends it. The bytes go
  through the block coder until passthrough is worth resuming.
* **The signal cannot collide with the payload.** `-` is an R-Set member, so a
  segment that contains one substitutes it; a payload therefore contains no `-`
  at all and the exit signal needs no escape rule.

### 2.2 Key properties

* **JSON-safe by construction.** The alphabet is disjoint from `{ " , \ }` and
  from every character below `0x20`. Output needs no escaping inside a JSON
  string; nothing an encoder can emit can break the document it sits in.
* **Density.** 13 or 14 bits per two characters in block mode (1.2308× to
  1.1429× size, against Base64's 1.3333× and every Base85's 1.25×); exactly
  1.0× in passthrough.
* **No expansion on decode.** One output byte needs at least one input
  character, so a decoder cannot be made to write more than it reads. There is
  no run-length construct and therefore no decompression bomb.
* **Padding-free.** Any input length; one canonical form for a truncated
  trailing group.
* **Linear time**, in both directions, guaranteed by construction
  (Section 6.6).
* **Readable where the input is readable.** A passthrough segment reproduces
  its input except for the handful of characters the mask names.

### 2.3 What this format is not

It is **not** XML-safe or HTML-safe. `<`, `>` and `&` are all in the alphabet
and all have to be escaped in those containers. Ninety-one characters cannot be
found that avoid JSON's, XML's and HTML's syntax at once: printable ASCII minus
space, `"`, `'`, `\`, `<`, `>` and `&` leaves 88. A format that has to survive
XML as well needs a smaller alphabet, which is what
[Base85N](https://base85n.ghadami.de/) is.

---

## 3. Conventions

BCP 14 [RFC2119] [RFC8174] keywords apply when, and only when, they appear in
all capitals.

| Symbol | Meaning |
|---|---|
| `mask` | 8-bit field; bit *j* set ⟺ R-Set character *j* occurs in the segment |
| `profile` | donor profile identifier, 0–3 |
| `k` | `popcount(mask)`, the number of active substitutions, 0 ≤ k ≤ 8 |
| `rank(j)` | `popcount(mask & ((1 << j) - 1))`, the position of bit *j* among the set bits |
| `L` | length of a passthrough segment, in bytes |
| `n` | the encoder's pending bit count; `n_enc` where it needs distinguishing |

*Ratio* means encoded characters divided by input bytes. *Efficiency* means its
reciprocal, as a percentage.

---

## 4. Alphabet, R-Set and donor profiles

### 4.1 The alphabet

Ninety-one characters with values 0–90.

```
ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~-
```

| Values | Characters |
|---|---|
| 0–25 | `A` … `Z` |
| 26–51 | `a` … `z` |
| 52–61 | `0` … `9` |
| 62–89 | `!` `#` `$` `%` `&` `(` `)` `*` `+` `,` `.` `/` `:` `;` `<` `=` `>` `?` `@` `[` `]` `^` `_` `` ` `` `{` `\|` `}` `~` |
| 90 | `-` |

This is basE91's alphabet with the character at value 90, `"`, replaced by `-`.
The four printable ASCII characters it does not contain are space, `"`, `'` and
`\`.

> **Value 90 is load-bearing.** Every property of the mode signal follows from
> `-` sitting there and nowhere else: see Section 6.3.

### 4.2 The R-Set

Eight byte values a passthrough segment carries by substitution. The index *j*
is normative — it fixes the bit positions in `mask`. The first seven are ordered
by frequency in the corpus of Section 14.

| j | Character | Byte | | j | Character | Byte |
|---|---|---|---|---|---|---|
| 0 | space | `0x20` | | 4 | CR | `0x0D` |
| 1 | `"` | `0x22` | | 5 | `'` | `0x27` |
| 2 | LF | `0x0A` | | 6 | TAB | `0x09` |
| 3 | `\` | `0x5C` | | 7 | `-` | `0x2D` |

Members 0 to 6 are the printable and whitespace characters real text is full of
and the alphabet does not contain. Member 7, `-`, is different in kind: it *is*
in the alphabet, and it is substituted not because it cannot be written but
because two of them in a row are the mode signal. Carrying it as a substitution
costs one donor in the segments that contain it, and buys three things:

* a payload never contains `-`, so the exit signal is unambiguous with no
  escape mechanism and no rule about what a segment may end on;
* text dense in `--` — CSS custom properties, Markdown rules, command lines —
  costs one donor per segment instead of a mode switch per occurrence;
* which character sits on value 90 stops being a size decision (Section 14.6).

Members 0 to 6 and the alphabet are disjoint. Together the alphabet and the
R-Set cover 98 of the 256 byte values; the other 158 are not representable in
passthrough.

### 4.3 Donor profiles

A **donor profile** is an ordered sequence of exactly eight distinct alphabet
characters, none of them `-`.

> A profile is not an alphabet. It is a ranking. Only its first `k` entries have
> any effect on a segment with `k` active substitutions.

| ID | Rank → 0 1 2 3 4 5 6 7 |
|---|---|
| 0 | `$` `~` `^` `%` `#` `@` `>` `<` |
| 1 | `@` `&` `!` `~` `%` `<` `$` `^` |
| 2 | `%` `@` `#` `<` `~` `>` `$` `^` |
| 3 | `*` `$` `?` `&` `^` `\|` `~` `%` |

Derivation and the reason there are four of them are in Section 14.4.

### 4.4 Substitution derivation

Given `profile` and `mask`:

```
rank = 0
for j in 0..7:
    if mask & (1 << j):
        donor(j) = PROFILE[profile][rank]
        rank += 1
```

The set bits of `mask` consume the **first `k` characters** of the selected
profile. Within a passthrough segment so described:

* an input byte equal to `R_CHARS[j]` for a set bit *j* SHALL be written as
  `donor(j)`;
* an input byte equal to any `donor(j)` of a **set** bit is not representable
  and cannot occur in the segment;
* every other alphabet character represents itself, `donor(j)` of a **clear**
  bit included;
* every other byte is not representable.

### 4.5 Escape characters

There are none, and no escape sequences.

---

## 5. Bit and byte order

Block mode is basE91's, unchanged: the accumulator is filled from the low end,
`b |= byte << n`, and the low 13 or 14 bits are taken off it. A pair's value is
`d0 + d1 × 91`, low digit first.

Every other multi-character field in this format — the header of Section 6.4,
the pending-bit field of Section 6.5 — uses the same convention: value
`Σ dᵢ × 91ⁱ`, low digit first.

---

## 6. Encoding

### 6.1 State

An encoder holds a bit accumulator `b` with `n` valid bits, `0 ≤ n ≤ 13`, and a
count `binaryRun` of the bytes it has put through block mode since the last
passthrough segment ended. Before the first byte, `b = n = 0` and `binaryRun`
is taken to be infinite: entering passthrough at position 0 costs no exit
signal and no pending bits.

### 6.2 Main loop

While input remains:

**Step 1 — passthrough scan.** If `binaryRun >= MIN_BINARY_RUN`, run the scan
of Section 6.6 at the current position, giving `(L, mask, profile)`. Otherwise
take `L = 0`.

**Step 2 — suitability.** Use passthrough if and only if `L >= MIN_DP_BYTES`.
If so, emit the segment per Sections 6.4 and 6.5, consume `L` bytes, set
`binaryRun = 0`, and repeat.

**Step 3 — block mode.** Otherwise put exactly one byte through the block coder
of Section 6.3, increment `binaryRun`, and repeat.

At the end of the input: if the last thing emitted was a passthrough segment
that ran to the end, nothing follows it. Otherwise flush the block coder per
Section 6.7.

### 6.3 Block mode

For each byte:

```
b |= byte << n ;  n += 8
if n > 13:
    v = b & 8191
    if v > BLOCK_THRESHOLD:   b >>= 13 ;  n -= 13
    else:                     v = b & 16383 ;  b >>= 14 ;  n -= 14
    emit ALPHABET[v % 91], ALPHABET[v / 91]
```

This is basE91 with one change: `BLOCK_THRESHOLD` is **87**, where basE91 uses
88.

That single decrement is what the whole format rests on. The values a pair can
take are `[0, 8191]` from the 13-bit branch and `{8192 + t : 0 ≤ t ≤
BLOCK_THRESHOLD}` from the 14-bit branch. With the threshold at 88 those cover
`[0, 8280]` exactly — all `91² = 8281` of them. With it at 87 they cover
`[0, 8279]`, and **8 280 is unreachable**. Since `8280 = 90 + 90 × 91`, the
unreachable pair is `-` twice.

The cost is one of 8 281 states, or 0.0017 % of the block coder's capacity: too
small to appear in the fifth decimal place of any measurement in Section 14.

### 6.4 Entering passthrough

Emit, in this order:

1. the two characters `--`;
2. the **header**, two characters, value

   ```
   h = hi + 2 × (mask + 256 × profile)
   ```

   where `hi = 1` if `n >= 8` and 0 otherwise (Section 6.5);
3. the **pending bits**: nothing if `n = 0`; one character of value `b` if
   `1 ≤ n ≤ 6`; two characters of value `b` if `7 ≤ n ≤ 13`.

Then set `b = n = 0` and emit the payload of Section 6.5.

The header is a *value*, written low digit first like every other pair, so
`h = 0` is `AA` and `h = 91` is `AB`.

### 6.5 The pending bits, and why one bit says how many

Block mode's accumulator holds bits belonging to bytes the encoder has already
consumed but not yet emitted. A passthrough segment's bytes come *after* those
bytes, so the pending bits have to be emitted before the segment starts. They
cannot simply be padded out to a full pair: 13 bits of padding for 1 bit of
data would make a decoder produce a byte that does not exist.

A decoder does not have to be told `n` in full, because it can derive it modulo
8. When the encoder holds `n` bits back, the decoder has received `8m − n` bits
for `m` consumed bytes, so it is holding `(−n) mod 8` bits itself. That fixes
`n mod 8`, leaving two candidates in `0..13`, and one bit chooses between them:

```
n_enc = ((8 − n_dec) mod 8) + 8 × hi
```

`n_enc > 13` is malformed. The number of characters the pending bits occupy
follows from `n_enc` alone: 0 for `n_enc = 0`, one for `1 ≤ n_enc ≤ 6` (six bits
always fit in one character, since `2⁶ − 1 = 63 ≤ 90`), two otherwise.

### 6.6 The passthrough prefix scan

The scan finds the longest prefix of the remaining input that one
`(mask, profile)` pair can carry, subject to `MAX_DP_BYTES`.

For each byte `c`, compute the state including `c` would produce, test it, and
commit only on success:

```
if c is R_CHARS[j] for some j:            # '-' is R_CHARS[7]
    if mask already has bit j:  accept, nothing changes
    new_mask = mask | (1 << j)
    new_k    = k + 1
    new_min  = min_donor
else if c is an alphabet character:
    r        = the per-profile rank vector of c   (7 where c is absent from a profile)
    new_min  = elementwise min(min_donor, r)
    if new_min == min_donor:    accept, nothing changes
    new_mask = mask ;  new_k = k
else:
    STOP                                  # not representable under any mask

new_profile = the smallest p with new_min[p] >= new_k
if no such p exists:
    STOP                                  # every profile would lend a character the text uses

commit:  mask, k, min_donor, profile  ←  the tentative values
```

`min_donor[p]` is the lowest rank any literal in the segment holds in profile
`p`, and a profile is viable exactly while that is at least `k`.

**The signal cannot occur in a payload.** `-` is `R_CHARS[7]`, so a segment
containing one sets bit 7 and writes a donor in its place; a segment that cannot
set bit 7 — because no profile could lend that many donors — stops at the `-`
instead. Either way no `-` reaches the payload, so no rule about doubled
hyphens or about what a segment may end on is needed. No donor may be `-`
(Section 4.3), which is what closes the argument.

> **Normative.** On STOP, the values that describe the emitted segment are those
> in effect **before** the byte that ended the scan was examined.

### 6.7 Leaving passthrough

After the payload: emit `--` if any input remains, and reset `b = n = 0`. If no
input remains, emit nothing — the end of the input ends the segment.

### 6.8 The final flush

When the input ends in block mode with `n > 0`, emit basE91's trailing group:
`ALPHABET[b % 91]`, followed by `ALPHABET[b / 91]` if `n > 7` or `b > 90`.

### 6.9 Constants

| Constant | Value | Notes |
|---|---|---|
| `BLOCK_THRESHOLD` | 87 | basE91's 88, less one, to free the pair `--` |
| `MIN_DP_BYTES` | 26 | Shortest segment at which passthrough is never larger than block mode |
| `MIN_BINARY_RUN` | 4 | Block-mode bytes before passthrough may resume (Section 14.2) |
| `MAX_DP_BYTES` | 65 536 | Encoder lookahead bound; makes the output canonical and the encoder's memory finite |
| `HEADER_CHARS` | 2 | Header width |
| `NUM_PROFILES` | 4 | Donor profiles (Section 14.4) |
| `R_LEN` | 8 | R-Set size, and the width of `mask` |

`MIN_DP_BYTES` is derived rather than fitted. A segment of `L` bytes costs
`L + 6` characters — two for the entry signal, two for the header, two for the
exit — while block mode charges at most `16/13` characters per byte. `L + 6 ≤
16L/13` gives `L ≥ 26`. The measured optimum on the corpus of Section 14 is 28,
better by 0.0006 %; the plateau runs from 26 to 32.

### 6.10 Canonicity

Encoder output is deterministic:

1. **Maximal prefix.** The scan of Section 6.6 takes the longest prefix it
   accepts, subject to `MAX_DP_BYTES`.
2. **Smallest viable profile.** The numerically smallest identifier that is
   viable for that prefix.
3. **Exact mask.** `mask` has a set bit for every R-Set character occurring in
   the segment and for no other.
4. **Empty mask implies profile 0.** If `mask = 0` then `profile` MUST be 0.
5. **Block mode consumes one byte at a time**, and passthrough is tested before
   each of them, subject to `MIN_BINARY_RUN`.
6. **The pending bits take the fewest characters that carry them**, per
   Section 6.5.

---

## 7. Decoding

### 7.1 The logical stream

The decoder operates on a **logical stream** obtained by removing ASCII space
(`0x20`), TAB (`0x09`), LF (`0x0A`) and CR (`0x0D`) from the physical input.
None of the four is in the alphabet, so line-wrapped output decodes unchanged.
All the steps below read from the logical stream.

The decoder holds a bit accumulator `b` with `n` valid bits, `0 ≤ n ≤ 7`, and is
in block mode at the start.

### 7.2 Block mode

Read two characters, `d0` and `d1`, and form `V = d0 + d1 × 91`.

* If `V = 8280` — the pair `--` — this is the mode signal; go to Section 7.4.
* Otherwise `V` is a block:

  ```
  b |= V << n
  n += (V & 8191) > BLOCK_THRESHOLD ? 13 : 14
  while n > 7:  emit b & 0xFF ;  b >>= 8 ;  n -= 8
  ```

If only one character remains, it is the trailing group of Section 6.8: emit
`(b | (d0 << n)) & 0xFF` and stop. It is an error if `n = 0`, or if
`d0 ≥ 2^(8−n)`: neither can arise from an encoder, and both would mean bits
that decode to nothing.

### 7.3 Passthrough

Read one character `c`.

* If `c` is `-` and the next character is also `-`, consume both: the segment
  ends and block mode resumes with `b = n = 0`.
* If `c` is `donor(j)` for a set bit *j*, emit `R_CHARS[j]`.
* Otherwise `c` MUST be an alphabet character, and its own byte value is
  emitted.

The end of the input ends the segment.

### 7.4 The mode signal

Read the two header characters as `h = d0 + d1 × 91`.

```
if h >= 2 × 256 × NUM_PROFILES:  error UNDEFINED_SIGNAL

hi      =  h & 1
rest    = (h - hi) / 2
mask    =  rest % 256
profile = (rest - mask) / 256

n_enc = ((8 - n) % 8) + 8 * hi
if n_enc > 13:  error INVALID_FLUSH

if n_enc > 0:
    read one character  -> w                    # value, low digit first
    if n_enc > 6:  read a second -> w += d * 91
    if w >= 2^n_enc:  error INVALID_FLUSH
    b |= w << n ;  n += n_enc     # n + n_enc is a multiple of 8 by construction
    while n > 7:  emit b & 0xFF ;  b >>= 8 ;  n -= 8

b = n = 0
derive donor(j) from (profile, mask) per Section 4.4
```

Passthrough then runs per Section 7.3.

---

## 8. Value and digit conversion

```
digits_to_value(d0, d1)  :  V = d0 + d1 × 91
value_to_digits(V)       :  d0 = V mod 91 ;  d1 = V div 91
```

---

## 9. Signal interpretation

Two characters span `91² = 8 281` values. Block mode occupies 8 280 of them and
the mode signal takes the one left over.

| Range of V | Interpretation |
|---|---|
| 0 … 8 279 | Block: 13 or 14 bits, told apart by `(V & 8191) > 87` |
| 8 280 | Mode signal: enter passthrough, or leave it |

The header that follows an entering signal has its own 8 281 values:

| Range of h | Interpretation |
|---|---|
| 0 … 2 047 | Passthrough segment: `hi`, 8-bit `mask`, `profile` 0–3 |
| 2 048 … 8 280 | `FUTURE_SIGNAL_SPACE`. MUST be rejected. |

**6 233 values are unassigned.** They are where a run-length construct would go
(Section 15).

---

## 10. Error handling

| Code | Condition |
|---|---|
| `INVALID_CHARACTER` | A significant character outside the alphabet |
| `UNEXPECTED_EOS` | The input ends while a pair, a header or a pending-bit field is still required |
| `UNDEFINED_SIGNAL` | A header value in `FUTURE_SIGNAL_SPACE` |
| `INVALID_FLUSH` | `n_enc > 13`, or a pending-bit field carrying more bits than `n_enc` |
| `INVALID_FINAL_BLOCK` | A trailing single character with `n = 0`, or one whose value does not fit the bits still owed |

An implementation MUST NOT read outside its input buffer, and MUST NOT
terminate the process, on malformed input.

---

## 11. Implementation guidance (informative)

### 11.1 Skipping binary stretches

On high-entropy input almost every position takes step 3 and consumes one byte,
so the scan is re-entered for nothing 8 000 times per 8 kB. An encoder may bail
out of the scan early instead: passthrough can begin at a position only if
`MIN_DP_BYTES` representable bytes do, and 158 of the 256 byte values are not
representable, so on random input that test fails within the first two or three
bytes.

Unlike a block-aligned format, there is nothing to be gained by *skipping*
positions: block mode here consumes one byte at a time, and every position is a
decision point. Cheapening the test is the whole optimisation.

### 11.2 Tracking the profiles at once

The scan keeps, per profile, the lowest rank any literal has held in it. Four
such numbers fit in one 32-bit word, one per byte lane, and both operations the
scan needs are then branch-free — see Base85N's specification, section 11.2,
for the lane arithmetic; it applies here unchanged with four lanes instead of
eight.

### 11.3 Streaming

An encoder needs `MAX_DP_BYTES` of lookahead and nothing else; a decoder needs
no lookahead at all beyond one character, to see whether a `-` in passthrough
begins the exit signal. Neither holds state across a segment boundary except
the block accumulator, which Section 6.4 flushes explicitly.

---

## 12. Conformance testing

### 12.1 Structural

* The alphabet has 91 distinct characters, none of them `"`, `\`, `'` or below
  `0x20`, and `-` is at value 90.
* `R_CHARS` has eight distinct entries; the first seven are not in the
  alphabet, and the eighth is `-`.
* Each profile has eight distinct alphabet characters and does not contain `-`.
* `8280` is not in the range of the block coder, for any input.

### 12.2 Round trip

* Random binary at every length 0–300, plus 1 023, 1 024, 1 025, 65 535,
  65 536, 65 537.
* Text with every one of the 256 masks over the R-Set.
* `-`, `--`, `---` and longer runs, at the start, in the middle and at the end
  of the input, and immediately before and after a segment boundary; and a
  segment in which no profile can lend a donor for `-`, so that the scan has to
  stop at one.
* Mixed text and binary, exercising every block↔passthrough transition, with
  the pending bit count `n` taking each of its 14 values at a transition.

### 12.3 Canonicity

* No active donor occurs as a literal inside an emitted segment.
* The emitted `profile` is the smallest viable one for the accepted prefix.
* `mask = 0` is emitted only with `profile = 0`.
* `mask` has a set bit for exactly the R-Set characters in the segment.
* No emitted segment contains `-` at all.
* The pending bits take 0, 1 or 2 characters exactly as Section 6.5 requires.

### 12.4 Adversarial decode

* Header values from both sides of `FUTURE_SIGNAL_SPACE`.
* Every `hi` and pending-bit-count combination, valid and not, including
  `n_enc = 14` and `n_enc = 15`, which no encoder can produce.
* Pending-bit fields carrying more bits than `n_enc` allows. (`n + n_enc` is a
  multiple of 8 by construction, so a field that fails to close the byte cannot
  be built; there is nothing to test there.)
* A signal at the very end of the input, with and without a header behind it.
* Trailing groups of one character with `n = 0`.
* Segment payloads containing characters outside the alphabet.

---

## 13. Security considerations

base91-jdp is an encoding, not a cryptographic transform. The decoder is the
security-relevant surface.

* **There is no expansion.** Every emitted byte consumes at least one input
  character: passthrough is 1:1 and a block pair yields at most two bytes. A
  decoder's output is bounded by its input, so no decompression-bomb defence is
  needed — unlike formats with a run-length construct, which have to bound it
  explicitly.
* **Lengths are not attacker-controlled**, because no length is carried in the
  stream. A segment ends where the exit signal is, and the exit signal is data
  the decoder has already read.
* **Whitespace skipping must be incremental**, or a padded stream costs
  quadratic time.
* **Output is arbitrary binary.** Callers MUST NOT assume it is printable,
  NUL-terminated or text, whatever the input looked like.

---

## 14. Measurements

Full method, per-file numbers and every sweep: `bench/results/RESULTS.md`.

### 14.1 Corpus

The benchmark corpus is Base85N's, unchanged: 6.52 MB across 13 real files,
fetched from pinned upstream archives by `bench/corpus.py` — three binary
container formats, an uncompressed source tar, a JSON dataset in both
pretty-printed and minified form, JavaScript, CSS and Python source, the
CommonMark specification, a Markdown changelog, a JPEG and a PNG. Using it
unchanged is what makes the comparison a comparison.

The donor profiles are derived on a separate 2.37 MB training corpus
(`tools/traincorpus.py`) that shares no file and no upstream project with it.

### 14.2 `MIN_BINARY_RUN`

The parameter answers: after passthrough has been broken by a byte it cannot
carry, how many bytes must go through block mode before it may resume? Ratio
over the whole corpus:

| `MIN_BINARY_RUN` | 1 | 2 | 3 | **4** | 5 | 8 | 16 | 32 |
|---|---|---|---|---|---|---|---|---|
| ratio | 1.08443 | 1.08442 | 1.08442 | **1.08442** | 1.08450 | 1.08452 | 1.08472 | 1.08503 |

Four is the optimum, and the margin over 1 is 0.001 % — the interesting feature
of the table is not the minimum but the step between 4 and 5. Below it the
parameter is doing nothing that `MIN_DP_BYTES` was not already doing: a
passthrough segment has to be worth its own signal regardless, so re-entering
early is not a mistake to be prevented. Above it the constraint starts
overriding that judgement and forcing block mode on bytes that would have been
carried at 1.0.

### 14.3 Overall

Characters per input byte, and the same figure once the output sits in a JSON
string, where `"` and `\` have to be escaped:

| | Base64 | Ascii85 | basE91 | Base85N | base91-jdp |
|---|---|---|---|---|---|
| text, raw | 1.33333 | 1.25000 | 1.22461 | **0.96474** | 1.00081 |
| binary, raw | 1.33334 | 1.12450 | 1.20546 | **1.05041** | 1.17037 |
| whole corpus, raw | 1.33333 | 1.18812 | 1.21517 | **1.00698** | 1.08442 |
| whole corpus, in JSON | 1.33333 | 1.21326 | 1.23996 | **1.00698** | 1.08442 |

Against basE91, the format it is a variant of, the swap costs nothing and the
container saves 12.5 %: 1.23996 against 1.08442 inside a JSON string. Against
Base64 it saves 18.7 %. Text lands at 1.00081 — passthrough carries the text
corpus at essentially one character per byte.

Against Base85N it wins where neither codec's compressing mode can do anything
and the alphabet is all that is left — every incompressible binary in the
corpus — and loses everywhere Base85N's Fill mode has runs to work with:

| sample | Base85N | base91-jdp |
|---|---|---|
| `sql-wasm.wasm` | 1.239 | **1.208** |
| `DejaVuSans.ttf` | 1.232 | **1.217** |
| `grace_hopper.jpg` | 1.249 | **1.229** |
| `minduka_present.png` | 1.250 | **1.229** |
| `bootstrap.css` | 1.003 | **1.001** |
| `lodash.js` | 1.004 | **1.001** |
| `countries.min.json` | 1.003 | **1.000** |
| `requests-2.32.3.tar` | **0.767** | 1.044 |
| `commonmark-spec.txt` | **0.859** | 1.005 |
| `countries.json` | **0.935** | 1.000 |

### 14.4 The donor profiles

Derived greedily by `tools/deriveprofiles.js`: profiles are added one at a time,
each chosen to minimise encoded size on the training corpus given the ones
already in the table, and within a profile the donor positions are filled left
to right. Gains on the training corpus:

| profiles | 1 | 2 | 3 | **4** | 5 | 6 |
|---|---|---|---|---|---|---|
| gain | — | 0.245 % | 0.067 % | **0.050 %** | 0.013 % | 0.019 % |

Four is where the curve flattens. Letters and digits are kept out of the
candidate pool on principle: a rare capital is rare across all text and common
in the one file that uses it, so it breaks segments in bursts. Allowing them
into the pool wins 0.1 % on the corpus it is derived from and loses 0.1 % on the
one it is measured on, which is what overfitting looks like.

### 14.5 `-` in the R-Set

Version 0.1.0 kept `-` out of the R-Set and handled the collision structurally:
a doubled hyphen in the input ended the segment, and a segment was forbidden to
end on a single one. Moving it in was worth:

| | 0.1.0, `-` a literal | 0.2.0, `-` substituted |
|---|---|---|
| `bootstrap.css` | 1.060 | **1.001** |
| `requests-history.md` | 1.028 | **1.002** |
| text corpus | 1.00654 | **1.00081** |
| whole corpus | 1.08758 | **1.08442** |

`bootstrap.css` is the case: 281 kB of CSS custom properties, every one of them
beginning `--`, which broke 2 046 passthrough segments. It now breaks none, and
the whole corpus went from 4 867 segments to 1 461. The cost is one donor
character in every segment that contains a hyphen, which is most of them — and
that cost is what the 0.29 % is net of.

### 14.6 The signal character

Which character sits on value 90 is a free choice once `"` is out of the
alphabet. `--` costs what its occurrences in text cost:

Which character sits on value 90 is a free choice once `"` is out of the
alphabet. Measured with R-Set membership moving with the candidate, as
Section 4.2 requires:

| signal | occurrences of the pair in the corpus | whole-corpus ratio |
|---|---|---|
| `--` | 10 553 | 1.08442 |
| `` `` `` | 41 002 | 1.08433 |
| `~~` | 328 | 1.08508 |
| `^^` | 38 | 1.08464 |
| `\|\|` | 303 | **1.08426** |
| `QQ` | 139 | 1.08427 |

The spread is 0.08 % and `--` sits 0.015 % off the best — which is the point of
the table rather than an aside. In version 0.1.0 the same measurement put `--`
0.3 % behind, and that was the strongest argument for choosing a different
character. Substituting `-` closed the gap, so the choice is now settled on the
grounds it should have been settled on all along: `-` is what the JSON-safety
swap put on value 90, and a doubled hyphen reads as a separator where `QQ`
reads as data.

### 14.7 What is not measured, and what is left on the table

* **The corpus is 13 files.** Every constant in Section 6.9 sits on a plateau
  rather than at a fitted optimum, for that reason.
* **There is no run-length construct.** A zero-padded ELF, a block-padded tar
  and pretty-printed JSON indentation are all carried at 1.0 or 1.2308 where a
  Fill mode would carry them at almost nothing. That is the whole of the gap to
  Base85N on this corpus, and Section 9 leaves the space to close it. Bounding
  the gain by assuming every run of five or more identical bytes cost five
  characters puts the whole corpus at 0.965 — below Base85N's 1.007 — with 36 %
  of `countries.json` and 34 % of the tar and the ELF sitting in such runs.
* **UTF-8 above U+007F breaks passthrough.** A multi-byte character is not
  representable, so prose in a language that uses accents runs through block
  mode. `commonmark-spec.txt` and `requests-history.md` both pay for this.

---

## 15. Future signal space (informative)

6 233 of the header's 8 281 values are unassigned, and one obvious construct
would fit in them: a run-length mode along the lines of Base85N's Fill,
carrying a repeated byte and a length instead of a payload. A header value of
2 048 or more could mean "a fill, and one further header character follows",
which gives `6233 × 91 = 567 203` states — enough for any byte value and a
length to 2 048 — at five characters plus the pending bits. Section 14.7 bounds
what it would be worth on this corpus at 0.965 characters per byte, against
1.084 today and Base85N's 1.007.

It is deliberately not in version 0.2.0. Adding it changes the security
argument of Section 13 — a decoder that can be made to write more than it reads
needs an expansion bound, and this one currently does not need one at all — and
it needs the run-break rule inside a passthrough segment that Base85N spends
its section 6.2 on, with a threshold of its own to measure.

---

## 16. References

* Joachim Henke, *basE91*, 2005. <http://base91.sourceforge.net/>
* Keywan Ghadami, *Base85N v0.5.0*, 2026. <https://base85n.ghadami.de/> — the
  passthrough design, the R-Set and donor-profile mechanism, and the benchmark
  corpus are taken from it.
* RFC 8259, *The JavaScript Object Notation (JSON) Data Interchange Format*.
