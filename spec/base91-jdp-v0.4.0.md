# base91-jdp: basE91 on a JSON-safe alphabet, with typed segments

| Field | Value |
|---|---|
| Version | 0.4.0 |
| Status | Draft |
| Date | 2026-08-25 |
| License | MPL-2.0 |
| Supersedes | 0.3.0 |

> **Draft.** The wire format is complete and there is a prototype encoder and
> decoder for all of it, class 20 included, in `rust/`. Section 17 is measured
> against that prototype except where it says otherwise, and Section 17.3 and
> the run break of Section 11.1 are both things the prototype found and the
> arithmetic had missed. Thirteen of the forty-four segment classes are
> unassigned, and a further forty-five are reachable through the escape, so the
> format has room without spending any of it today.

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

The substitution decides the rest of the format. `-` lands on the alphabet's
last value, 90, so the pair `--` is worth 8 280 — above everything a
thirteen-bit symbol can spell. base91-jdp fixes symbols at thirteen bits rather
than letting them float between thirteen and fourteen as basE91 does, which
leaves **eighty-nine pair values that no encoded stream can contain**. Those
eighty-nine values are the format's entire signalling mechanism: eighty-eight of
them open a typed segment, and the last one, `--`, is the escape.

A typed segment says what kind of bytes it carries, and the format writes that
kind at the density it deserves:

| What the bytes are | Written as | Characters per byte |
|---|---|---|
| a run of one repeated byte | the run's length | 0.03 and below |
| decimal digits, hex | 4 bits each | 0.62 |
| base32, letters, hex with separators | 5 bits each | 0.77 |
| base64, alphanumerics | 6 bits each | 0.92 |
| text the alphabet can carry | one character each | 1.00 |
| anything else | the block coder | 1.23 |

A stream that wants none of this pays nothing: no signal, no header, no
padding, no terminator.

---

## 2. Introduction

### 2.1 Design summary

* **Symbols are thirteen bits, always.** Two characters are one pair, worth
  `d0 + d1 × 91`; thirteen bytes are eight symbols in sixteen characters,
  exactly. This costs up to 3.4 % against basE91's adaptive coder on structured
  binary (Section 17.2) and buys the one thing the format is built on:
  eighty-nine pair values that no payload can produce.

* **A signal is one pair.** Values 8 192 … 8 279 carry a segment class and a
  flush flag in two characters. There is no header, no marker at the head of
  the stream, and no probability attached to detection.

* **Segments are length-delimited.** A length field follows the signal; there
  is no exit signal and no character a payload is forbidden to contain. This is
  what lets a hyphen be an ordinary literal again (Section 18.3).

* **Passthrough** carries text at one character per byte. Eight byte values
  that real data is full of and the alphabet does not contain — space, `"`,
  newline, `\`, carriage return, `'`, tab, NUL — are carried as stand-ins
  borrowed from the alphabet's rarest characters.

* **Runs are the length alone.** A run of zero bytes is three characters for up
  to eighty-nine of them and five for up to 8 369, because the class *is* the
  byte value; any other repeated byte costs one pair more. Where short runs
  alternate with a few bytes that are not zero — a symbol table, a relocation
  table — one segment carries the whole alternation, with the gap width in the
  class rather than in a field. Section 18.2 is why this is in the format rather
  than left to the compressor.

* **Packed bases** carry text drawn from a restricted alphabet at four, five or
  six bits per byte. A hex string costs 0.62 characters per byte where
  passthrough would cost 1.00 and the block coder 1.23. Nothing is trained and
  nothing is learned: the class names the alphabet, the encoder checks
  membership.

* **Compression is a segment class** carrying a zstd frame. The frame carries
  its own length and its own checksum, so the format specifies neither.

* **The encoder decides by measuring.** For each candidate it computes the
  exact character count and takes the smallest, with the plain block coder
  always among the candidates. No candidate is ever worse than block mode, so
  the output is bounded by it (Section 11.2).

### 2.2 Key properties

| Property | Value |
|---|---|
| Alphabet | 91 characters, none of which JSON escapes |
| Expansion, incompressible input | 1.2308 characters per byte, plus at most 2 |
| Expansion, text without compression | ~1.00 characters per byte |
| Expansion, hex or digits | ~0.62 characters per byte |
| Expansion, a run of one byte | 3 characters per 89 bytes and better |
| Overhead of a stream that uses no segment | zero characters |
| Overhead of a segment | three characters, typical |
| Shortest segment that pays for itself | 2 bytes (zero run), 5 (hex), 10 (text) |
| Classes an encoder may skip | all of them; a decoder MUST implement all |

### 2.3 What this format is not

It is not an archiver and it is not the smallest thing you can do to bytes. A
strong compressor applied before encoding will beat everything here on large
inputs, and Section 10 is how you do that inside the format rather than beside
it.

It offers no integrity guarantee. There is no checksum, no error correction and
no terminator, and a truncated stream may decode as a shorter valid one. This
is deliberate: the format is a transport encoding, and it protects data no
better and no worse than Base64 does. Where a compressed segment is used, the
zstd frame's own checksum applies to that segment; nothing else is covered.
Callers who need integrity put it outside — where it can also cover the parts
of their payload that never entered this format.

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
| `L` | length of a segment, in bytes |
| `w` | width in bits of one input byte in a packed base |
| `n` | the encoder's pending bit count; `n_enc` where it needs distinguishing |

*Ratio* means encoded characters divided by input bytes.

---

## 4. Alphabet

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

> **Value 90 is load-bearing.** `91 × 90 = 8 190`, so every pair from 8 192 up
> has `-` as its high digit, and no such pair can be produced by packing.
> Section 5.2 is the consequence.

---

## 5. Bit and byte order, and the pair space

A pair's value is `d0 + d1 × 91`, **low digit first**. Every multi-character
field in this format uses that convention.

### 5.1 The symbol stream

Bytes become thirteen-bit symbols **most significant bit first**. The
accumulator takes bits in at the bottom and gives symbols off the top:

```
acc = (acc << w) | value ;  nb += w
while nb >= 13:
    nb -= 13
    emit symbol (acc >> nb) & 8191
```

In block mode `w = 8` and `value` is the byte. In a packed base (Section 9) `w`
is 4, 5 or 6 and `value` is the byte's index in the class alphabet. Thirteen
bytes at `w = 8` are 104 bits are exactly eight symbols, so a whole group of
thirteen bytes is sixteen characters with nothing left over.

**Every field in this format uses this order**, the flush field of Section 7.2
included. There is no little-endian bit path anywhere.

### 5.2 What a pair may be

| Range of V | Meaning |
|---|---|
| 0 … 8 191 | a thirteen-bit symbol |
| 8 192 … 8 279 | a **segment signal** (Section 7) |
| 8 280 | `--`: the **escape**, extending the class space (Section 7.1) |

**No packed stream can spell 8 192 or above.** A symbol is thirteen bits, so
its greatest value is 8 191, and a pair carrying one is at most 8 191. That is
the whole basis of the format's self-description, and it is why symbols are
fixed at thirteen bits: a coder that let them float to fourteen, as basE91
does, would reach 8 280 and leave nothing free.

---

## 6. Block mode

### 6.1 State

An encoder holds a bit accumulator `b` with `n` valid bits, `0 ≤ n ≤ 12`.
Before the first byte, `b = n = 0`, so opening a segment at position 0 costs no
flush.

### 6.2 The coder

For each byte:

```
acc = (acc << 8) | byte ;  n += 8
if n >= 13:
    n -= 13
    v = (acc >> n) & 8191 ;  acc &= (1 << n) - 1
    emit ALPHABET[v mod 91], ALPHABET[v div 91]
```

`n` is never more than twelve between symbols. There is no threshold and no
branch: every symbol is thirteen bits. A stream that opens no segment is
therefore exactly the byte-synchronous packing of Section 5.1 — thirteen bytes
to sixteen characters — and an implementation MAY produce it that way.

### 6.3 The final flush

When the input ends in block mode, emit the `n` pending bits as a final group:
nothing if `n = 0`, one character of value `acc` if `1 ≤ n ≤ 6`, two characters
of value `acc` if `7 ≤ n ≤ 12`.

This makes the final group **self-delimiting**. A decoder holding `r` bits knows
the writer owed `(−r) mod 8` or eight more; one character can only carry the
first if it is six bits or fewer, two characters only the second, and the one
case where two characters could mean either — three held bits — cannot arise,
because `n + 13 ≡ 0 (mod 8)` has exactly one solution in `0 … 7`, so after three
held bits a whole symbol is the only thing that closes the stream. Section 12.3
states the rule from the decoder's side.

### 6.4 The main loop

While input remains:

1. **Candidate scan.** Run the scan of Section 11.1 at the current position.
2. **Commit.** If a candidate costs fewer characters than putting the same
   bytes through block mode — *including* the flush field the segment forces
   and the pending bits block mode would have carried on with — emit the
   segment per Section 7, consume its bytes, set `binaryRun = 0`, and repeat.
3. **Otherwise** put exactly one byte through the block coder and repeat.

At the end of the input: if the last thing emitted was a segment that ran to the
end, nothing follows it. Otherwise flush per Section 6.3.

---

## 7. Segments

A segment is:

```
signal  flush  params  length  payload
```

There is no exit signal. The length field says how long the payload is, and
block mode resumes immediately after it.

### 7.1 The signal

One pair, value `V = 8192 + 2 × class + hi`, so `class` is 0 … 43 and `hi` is
the flush flag of Section 7.2. Two characters carry both.

`V = 8280` (`--`) is the escape: one further character, value `e` in 0 … 89,
gives

```
class = 44 + (e div 2)        hi = e mod 2
```

so the escape reaches classes 44 … 88, forty-five of them, and costs one
character more than a signal. `hi` is carried inside `e` and not deferred to a
later field: the flush field's own width depends on `hi`, so a decoder that had
to read that field first could not know how wide it was. This version defines
no class above 43, so a conforming 0.4.0 decoder MUST reject the escape with
`EXTENDED_CLASS`.

### 7.2 The flush field

Block mode's accumulator holds bits belonging to bytes the encoder has already
consumed but not yet emitted. A segment's bytes come *after* those bytes, so the
pending bits have to be emitted before the segment starts. They cannot be padded
out to a full pair: thirteen bits of padding for one bit of data would make a
decoder produce a byte that does not exist.

A decoder does not have to be told `n` in full, because it can derive it modulo
eight. When the encoder holds `n` bits back, the decoder has received `8m − n`
bits for `m` consumed bytes, so it is holding `(−n) mod 8` bits itself. That
fixes `n mod 8`, leaving two candidates in `0 … 13`, and one bit — `hi` — chooses
between them:

```
n_enc = ((8 − n_dec) mod 8) + 8 × hi
```

`n_enc > 12` is malformed — twelve, not thirteen, because a thirteenth bit would
have become a symbol. The width of the field follows from `n_enc` alone: zero
characters for `n_enc = 0`, one for `1 ≤ n_enc ≤ 6` (six bits always fit in one
character, since `2⁶ − 1 = 63 ≤ 90`), two otherwise. The value is written low
digit first and its bits are read most significant first, per Section 5.1.

After the field, `b = n = 0`. **A segment that begins where the previous one
ended therefore has an empty flush field**, which is what makes a run of short
segments — a symbol table of zero runs, a column of hex fields — cost three
characters each rather than five.

### 7.3 The length field

Three tiers, low digit first throughout. `p`, `p0` and `p1` are pair values in
`0 … 8 279`; the escape value 8 280 does not occur in a length field, so the
radix of the third tier is 8 280.

| First character | Then | Length |
|---|---|---|
| `v` in 0 … 89 | — | `v` |
| `-` (90) | one pair `p` | `90 + p` |
| `-` (90) | `--`, then two pairs `p0`, `p1` | `8 370 + p0 + 8 280 × p1` |

One character for anything under 90 bytes, three for anything under 8 370, seven
beyond. An encoder MUST use the shortest tier that carries the value, and a
decoder MUST reject a value written in a longer tier than necessary with
`INVALID_LENGTH`. The third tier reaches 68 566 769, which is above any length
this format has a use for; the bounds of Section 11.4 constrain it further.

Length zero is malformed.

### 7.4 The classes

| Class | Name | Payload | `w` | Alphabet or mask |
|---|---|---|---|---|
| 0 | `PT` | passthrough | — | general; params pair follows |
| 1 | `PT0` | passthrough | — | `mask = {}` |
| 2 | `PT_S` | passthrough | — | `mask = {SP}` |
| 3 | `PT_SL` | passthrough | — | `mask = {SP, LF}` |
| 4 | `PT_SQ` | passthrough | — | `mask = {SP, "}` |
| 5 | `PT_SQL` | passthrough | — | `mask = {SP, ", LF}` |
| 6 | `PT_Z` | passthrough | — | `mask = {NUL}` |
| 7 | `DEC` | packed | 4 | `0123456789` |
| 8 | `HEXL` | packed | 4 | `0123456789abcdef` |
| 9 | `HEXU` | packed | 4 | `0123456789ABCDEF` |
| 10 | `HEXL_D` | packed | 5 | `HEXL` followed by `-` |
| 11 | `HEXU_D` | packed | 5 | `HEXU` followed by `-` |
| 12 | `ALPHA_L` | packed | 5 | `a` … `z` |
| 13 | `ALPHA_U` | packed | 5 | `A` … `Z` |
| 14 | `B32` | packed | 5 | RFC 4648 base32, `A`…`Z` then `2`…`7` |
| 15 | `B32H` | packed | 5 | RFC 4648 base32hex, `0`…`9` then `A`…`V` |
| 16 | `CROCK` | packed | 5 | Crockford, `0123456789ABCDEFGHJKMNPQRSTVWXYZ` |
| 17 | `B64` | packed | 6 | RFC 4648 base64, `A`…`Z` `a`…`z` `0`…`9` `+` `/` |
| 18 | `B64U` | packed | 6 | RFC 4648 base64url, `A`…`Z` `a`…`z` `0`…`9` `-` `_` |
| 19 | `ALNUM` | packed | 6 | `0`…`9` `A`…`Z` `a`…`z` |
| 20 | `ZSTD` | block-packed | 8 | a zstd frame (Section 10) |
| 21 | `ZRUN` | none | — | `L` zero bytes (Section 10.2) |
| 22 | `RUN` | one pair | — | `L` copies of one byte (Section 10.2) |
| 23 … 30 | `ZMIX_G1` … `ZMIX_G8` | chain | — | zero runs separated by fixed `g`-byte gaps (Section 10.3) |
| 31 … 43 | — | reserved | | MUST be rejected with `UNKNOWN_CLASS` |

Classes 1 to 6 are shorthands: they save the two parameter characters of class 0
on the masks that real data overwhelmingly has, all with `profile = 0`. They are
a table, not a mechanism; class 0 can express all of them. `PT_SL` is prose and
source without quotation marks, and `PT_Z` is the NUL-separated string table of
an object file, which Section 17.6 measures as the single largest passthrough
gain in the corpus.

Base64 **with** padding is not class 17 or 18: `=` makes 65 distinct values,
which needs seven bits and loses to passthrough. An encoder MAY split such a
string, putting the body in a packed base and the one or two `=` characters
through block mode, or MAY use passthrough for the whole of it. Whichever is
shorter wins by Section 11.

### 7.5 Parameters

Only class 0 has any. One pair, value `p = mask + 256 × profile`, so
`0 ≤ p ≤ 1 023`. The set bits of `mask` are the R-Set members occurring in the
segment; `profile` selects the donor profile of Section 8.2.

---

## 8. Passthrough

Classes 0 to 6. The payload is `L` characters, one per input byte.

### 8.1 The R-Set

Eight byte values a passthrough segment carries by substitution. The index *j*
is normative — it fixes the bit positions in `mask`.

| j | Character | Byte | | j | Character | Byte |
|---|---|---|---|---|---|---|
| 0 | space | `0x20` | | 4 | CR | `0x0D` |
| 1 | `"` | `0x22` | | 5 | `'` | `0x27` |
| 2 | LF | `0x0A` | | 6 | TAB | `0x09` |
| 3 | `\` | `0x5C` | | 7 | NUL | `0x00` |

Indices 0 to 6 are ordered by frequency in text, where they are the printable
and whitespace characters real prose is full of. **NUL is index 7 and is the
one that matters for binary**: it is the most frequent non-alphabet byte in
every object file, archive and image in the corpus, and admitting it raises the
share of the corpus that passthrough can reach at all from 60.2 % to 64.4 %
(Section 17.6). It costs text nothing, because `mask` is exact and prose does
not contain it.

The R-Set and the alphabet are disjoint. Together they cover 99 of the 256 byte
values; the other 157 are not representable in passthrough.

> **`-` is not a member.** In 0.2.0 and 0.3.0 it was, because a doubled hyphen
> was the exit signal and had to be kept out of the payload. Length delimiting
> removed that constraint, so `-` is an ordinary literal again and every segment
> containing one gets a donor back — and the slot it vacated is what NUL now
> occupies. Section 18.3.

### 8.2 Donor profiles

A **donor profile** is an ordered sequence of eight distinct alphabet
characters, none of them `-`. It is not an alphabet but a ranking: only its
first `k` entries have any effect on a segment with `k` active substitutions.

| ID | Rank → 0 1 2 3 4 5 6 7 |
|---|---|
| 0 | `$` `~` `^` `%` `#` `@` `>` `<` |
| 1 | `@` `&` `!` `~` `%` `<` `$` `^` |
| 2 | `%` `@` `#` `<` `~` `>` `$` `^` |
| 3 | `*` `$` `?` `&` `^` `\|` `~` `%` |

With NUL in the R-Set, `k` can reach eight and the eighth rank is reachable for
the first time. Section 17.5 says what that means for the derivation, which has
not been re-run.

### 8.3 Substitution

Given `profile` and `mask`:

```
rank = 0
for j in 0..7:
    if mask & (1 << j):
        donor(j) = PROFILE[profile][rank]
        rank += 1
```

Within a segment so described:

* an input byte equal to `R_CHARS[j]` for a set bit *j* SHALL be written as
  `donor(j)`;
* an input byte equal to any `donor(j)` of a **set** bit is not representable
  and cannot occur in the segment;
* every other alphabet character represents itself, `donor(j)` of a **clear**
  bit included;
* every other byte is not representable.

There are no escape characters and no escape sequences.

### 8.4 Decoding

For each of the `L` payload characters `c`:

* if `c` is `donor(j)` for a set bit *j*, emit `R_CHARS[j]`;
* otherwise `c` MUST be an alphabet character, and its own byte value is
  emitted.

---

## 9. Packed bases

Classes 7 to 19. The class names an alphabet of `b` characters and a width
`w = ⌈log₂ b⌉`.

**Encoding.** Each of the `L` input bytes is replaced by its index in the class
alphabet — the encoder has already established that every byte is a member — and
the indices are fed to the accumulator of Section 5.1 at `w` bits each. The last
symbol is padded with zero bits.

The payload therefore occupies exactly

```
2 × ⌈L × w / 13⌉   characters
```

which the decoder computes from `L` and `w` before reading. It reads that many
characters, forms the symbols, takes the first `L × w` bits, and maps each
`w`-bit group back through the class alphabet. Trailing padding bits are
discarded; an encoder MUST set them to zero and a decoder MAY reject a nonzero
value with `MALFORMED_PADDING`.

Where `b < 2^w` — classes 7, 10, 11, 12, 13 and 19 — indices from `b` to
`2^w − 1` cannot be produced. A decoder MUST reject them with `INVALID_INDEX`.
This is not a checksum, but it is a free structural check, and on `DEC` it
rejects six values in sixteen.

The ratios follow from `w` alone:

| `w` | Characters per byte | Classes |
|---|---|---|
| 4 | 0.6154 | `DEC`, `HEXL`, `HEXU` |
| 5 | 0.7692 | `HEXL_D`, `HEXU_D`, `ALPHA_L`, `ALPHA_U`, `B32`, `B32H`, `CROCK` |
| 6 | 0.9231 | `B64`, `B64U`, `ALNUM` |

A width of seven would give 1.0769, which loses to passthrough; that is why the
scale stops at six and passthrough takes over.

---

## 10. Compression and runs

### 10.1 Compression

Class 20. The payload is a **zstd frame** [RFC8878], packed at `w = 8` through
the block coder of Section 5.1.

**A compressed payload is block mode and nothing else.** The bytes of a frame
are packed at `w = 8` and an encoder MUST NOT run the candidate scan of
Section 11.1 over them: it produced them, it knows they are compressed, and a
compressor's output holds no run, no restricted alphabet and no representable
text worth looking for. Section 17.12 measures what looking anyway costs, and
it is a factor of fifteen.

The frame is unmodified and self-delimiting. It carries its own magic number,
its own window descriptor, optionally its own content size, and optionally its
own XXH64 checksum. This format specifies none of that and adds no padding byte,
no dictionary rule and no segment structure of its own — the length field of
Section 7.3 gives the frame's length in bytes, and everything else is zstd's.

**The payload is exactly one frame.** A decoder MUST reject a payload with
trailing bytes after the frame it decodes, and MUST reject a skippable frame,
with `MALFORMED_FRAME`.

**A payload longer than `MAX_SEGMENT_BYTES` is carried as several segments.**
`MAX_SEGMENT_BYTES` bounds an uncompressed segment at 64 KiB, which no zstd
frame over a large input respects; `MAX_FRAME_BYTES` bounds a compressed one
instead, and an encoder with more input than one frame may carry emits
consecutive `ZSTD` segments, each an independent frame. The payload each frame
covers is an encoder choice: Section 17.7 measures it, and 1 MiB costs 0.2 %
against a single frame where 64 KiB costs 6.3 %.

**Level is an encoder setting and is not carried.** The decoder does not need it.
A caller who wants speed uses a negative level; a caller who wants size uses a
high one; the frame is the same shape either way.

**Dictionaries are out of scope.** A frame that references a dictionary decodes
only where that dictionary is present, which would make the stream no longer
self-describing. An encoder MUST NOT emit such a frame.

**A decoder MUST bound what it allocates.** A compressor expands, and the
expansion is attacker-controlled. `MAX_FRAME_BYTES` bounds the compressed
length; the decompressed length must be bounded independently, from the frame's
content-size field where present and by a caller-supplied ceiling where not,
and that ceiling belongs on the total across all segments, not on each one.

### 10.2 Runs

Class 21, `ZRUN`, has **no payload at all**: the class is the byte value, and
the length field alone says how many zero bytes to emit. Three characters carry
up to 89 of them, five up to 8 369, nine up to `MAX_SEGMENT_BYTES`.

Class 22, `RUN`, adds one pair naming the byte, value `0 … 255`. A pair of 256
or above is malformed, and so is a pair of zero: that run is a `ZRUN`, and
Section 11.3 makes the choice canonical rather than optional.

Neither class reads any payload characters beyond that pair, so neither can
expand a stream by more than its own signal. A run longer than
`MAX_SEGMENT_BYTES` is emitted as consecutive run segments; the second and
every later one begins where its predecessor ended, so its flush field is empty
and it costs three characters.

### 10.3 Runs with gaps

Structured binary is not one long run. A symbol table, a relocation table, a
glyph table is short zero runs separated by a few bytes that are not zero, over
and over, and each of those gaps ends a `ZRUN` and starts another — two signal
pairs for two bytes of content.

Classes 23 to 30 carry the whole alternation in one segment. The class fixes
the gap width `g = class − 22`, from one byte to eight, and the segment is:

```
count  len(0)  gap(0)  len(1)  gap(1)  ...  len(c-1)  gap(c-1)  len(c)
```

`count` is the length field of Section 7.3 read as a number of gaps, `1 ≤ c`;
each `len(i)` is a length field giving a run of `1 …` zero bytes; each `gap(i)`
is exactly `g` bytes, packed at `w = 8` into `2 × ⌈8g / 13⌉` characters of its
own. A segment carries `c + 1` zero runs and `c` gaps, and the total number of
bytes it emits MUST NOT exceed `MAX_SEGMENT_BYTES`.

**The gap width is in the class, not in a field, and that is the whole design.**
What one of these segments saves over the `ZRUN`–block–`ZRUN` it replaces is a
signal pair, less whatever rounding the gap to whole symbols costs against block
mode's amortised thirteen bits per symbol — one or two characters. A field
naming the gap width would cost one of those characters back, which Section 18.7
measures as roughly two thirds of the gain. Eight classes and no field is the
cheaper arrangement, and eight covers the gap widths that occur.

Chaining is what makes it pay rather than the pairing alone: a class that
carried exactly two runs and one gap could swallow only every other boundary in
an alternating sequence, and Section 18.7 measures both.

---

## 11. Encoder rules

### 11.1 The candidate scan

At a position where a segment may open, the encoder determines, for each class
it implements, the longest prefix of the remaining input that class can carry,
subject to `MAX_SEGMENT_BYTES`; computes the exact character count of the
resulting segment, **flush field included**; and compares it against what those
same bytes would cost in block mode from the encoder's current pending-bit
state.

For a run class the prefix is the run of identical bytes. For a packed base it
ends at the first byte outside the class alphabet. For passthrough it ends at
the first byte that is neither an alphabet character nor an R-Set member, and
additionally at any byte that no viable profile can accommodate:

```
for each byte c:
    if c is R_CHARS[j]:
        new_mask = mask | (1 << j) ;  new_k = k + (bit j was clear)
        new_min  = min_donor
    else if c is an alphabet character:
        r        = the per-profile rank vector of c   (8 where absent)
        new_min  = elementwise min(min_donor, r)
        new_mask = mask ;  new_k = k
    else:
        STOP
    new_profile = the smallest p with new_min[p] >= new_k
    if no such p exists:  STOP
    commit
```

`min_donor[p]` is the lowest rank any literal in the segment holds in profile
`p`, and a profile is viable exactly while that is at least `k`. On STOP, the
values describing the emitted segment are those in effect **before** the byte
that ended the scan was examined.

**The run break.** A passthrough or packed prefix SHALL also stop at the first
position `f` such that the `MIN_RUN_IN_SEGMENT` bytes at `f` are identical; the
prefix ends at `f`, and the run is then carried by a class of Section 10.2 at
the next decision point. Without this the prefix scans are greedy and swallow
the runs those classes exist for: passthrough carries a zero byte at one
character each, since NUL is an R-Set member, where `ZRUN` carries eighty-nine
of them in three. This was not in the first draft of this version and cost
2.3 % of the corpus; Section 17.3 measures the threshold and Section 18.11 says
why it is not simply "any run at all".

The thresholds below are the shortest segment that wins, and the shortest from
which a class wins at every longer length. They are not monotone — a packed
base at `w = 5` wins at five bytes, loses at six and wins again at seven,
because both sides round up to a whole symbol at different rates — so an
encoder MUST compare rather than consult a threshold. The two columns assume an
empty flush field; one that costs a character moves both by four to eight bytes.

| Class group | First length that wins | Wins from |
|---|---|---|
| `ZRUN` | 2 | 2 |
| `RUN` | 4 | 5 |
| `ZMIX_Gg` | one gap, against two `ZRUN`s | always |
| `w = 4` | 5 | 5 |
| `w = 5` | 5 | 9 |
| `w = 6` | 10 | 12 |
| passthrough, classes 1–6 | 10 | 14 |
| passthrough, class 0 | 18 | 22 |

### 11.2 Compression

`ZSTD` is not part of the scan. An encoder that has been asked to compress
builds the whole-input candidate — one or more `ZSTD` segments per Section 10.1
— computes its character count, and takes it only if it beats what the scan and
block mode produce together.

Building both is what makes this rule expensive. On the corpus the compressed
path encodes at 120 to 415 MB/s and the same encoder weighing both candidates
at 25 to 85, because the uncompressed candidate is the slow one -- it is the
scan of Section 11.1 over data the scan has something to find in. An encoder
told outright to compress MAY skip the comparison; one left to decide MUST
make it, and Section 17.14 says what it costs and what it saves, which over
the corpus is one part in thirty thousand.

Since block mode is always a candidate and no other candidate is committed
unless it is strictly shorter, **encoder output never exceeds block mode**:
`2 × ⌈8L / 13⌉` characters, which is 1.2308 per byte plus at most two for the
final group. That holds whatever the level and whatever the input, and it is
why a compressed segment cannot make an incompressible payload worse.

### 11.3 Canonicity

Encoder output is deterministic:

1. **Smallest output.** Among all candidates the encoder built, the one with the
   fewest characters.
2. **Lowest class on a tie.** Where two candidates are the same length, the one
   with the lower class number; block mode counts as lower than every class.
3. **Maximal prefix.** Within a class, the longest prefix the scan accepts,
   subject to `MAX_SEGMENT_BYTES` and to the run break of Section 11.1.
4. **Smallest viable profile**, and `mask` set for exactly the R-Set characters
   occurring in the segment.
5. **Shortest length tier** that carries the value, and shortest flush field
   that carries the pending bits.
6. **Shorthand before general.** Where a passthrough segment's mask and profile
   match one of classes 1 to 6, that class MUST be used rather than class 0.
7. **`ZRUN` before `RUN`.** A run of zero bytes MUST be class 21, never class 22
   with a zero payload pair.
8. **Maximal chain.** A `ZMIX` segment MUST extend while the next gap is exactly
   `g` bytes and the next zero run is at least one byte — subject to
   `MAX_SEGMENT_BYTES` and to `count` fitting its tier. Every zero run in the
   stream is maximal, so a gap byte adjacent to a run boundary is never zero.

Two encoders that implement the same set of classes therefore produce identical
output. An encoder that implements fewer produces valid but larger output, which
Section 15 treats as conforming.

### 11.4 Constants

| Constant | Value | Notes |
|---|---|---|
| `SYMBOL_BITS` | 13 | Fixed; no fourteen-bit branch |
| `MIN_RUN_IN_SEGMENT` | 8 | Shortest run that ends a passthrough or packed prefix (Sections 11.1, 17.3) |
| `MAX_SEGMENT_BYTES` | 65 536 | Bound on every class but `ZSTD`; makes output canonical and encoder memory finite |
| `MAX_FRAME_BYTES` | 16 777 216 | Bound on one `ZSTD` payload (Section 10.1) |
| `NUM_PROFILES` | 4 | Donor profiles (Section 17.5) |
| `R_LEN` | 8 | R-Set size, and the width of `mask` |
| `PARALLEL_ALIGN` | 13 | Bytes per whole symbol group; a block-mode split here needs no seam (Section 14.5) |

`MIN_BINARY_RUN`, which 0.3.0 set to 4, is **gone**. It answered a question the
0.3.0 structure raised -- after passthrough had been broken, how many bytes must
go through block mode before it may resume -- and with typed segments the answer
is none: Section 17.3 measures every value and zero is the best of them.

### 11.5 Deciding not to scan

The scan of Section 11.1 is what encoding costs on data no class can carry. A
reference implementation encoding high-entropy input spends five sixths of its
time there, entering the scan once per byte and finding nothing once per byte:
531 MB/s for the block coder alone against 31 for the encoder around it.

An encoder MAY therefore decide, for a stretch of input, that no scan is worth
running, and put the whole stretch through block mode. Two signals carry that
decision, and Section 17.12 measures both:

* **The stream opens with a magic number** whose format is already compressed —
  a zstd frame, a gzip or zlib stream, a JPEG, a PNG, a zip. This is not a
  guess.
* **The entropy of a sample is high.** Above roughly 7.4 bits per byte nothing
  the format offers can reach: passthrough needs ten consecutive representable
  bytes and a packed base five of one alphabet, and at that entropy neither
  occurs often enough to pay for looking. This is what catches a raw deflate
  stream, which has no magic number at all.

**A wrong decision costs size, never correctness**, and the cost is bounded:
block mode is the ceiling of Section 11.2, so a false positive can do no worse
than 1.2308 characters per byte on data that would have done better. That bound
is what makes the decision safe to take on a guess.

**It SHOULD be taken per window rather than once per stream.** A `tar`
alternates text headers, compressed members and zero padding every few hundred
bytes, and one decision at its head would be wrong for most of its length.
Where an encoder aligns its windows to absolute offsets in the stream, the
decision is a pure function of the bytes in the window and the parallel
arrangement of Section 14.5 still produces byte-identical output.

This is an encoder-side choice and it changes what is emitted, so an encoder
that takes it does not satisfy Section 11.3 against one that does not. Like the
class subsetting of Section 15.5, it MUST be documented rather than assumed.

---

## 12. Decoding

### 12.1 The logical stream

The decoder operates on a **logical stream** obtained by removing ASCII space
(`0x20`), TAB (`0x09`), LF (`0x0A`) and CR (`0x0D`) from the physical input.
None of the four is in the alphabet, so line-wrapped output decodes unchanged.
Removal MUST be incremental, or a padded stream costs quadratic time.

The decoder holds a bit accumulator `b` with `n` valid bits, `0 ≤ n ≤ 7`, and is
in block mode at the start.

### 12.2 Block mode

Let `left` be the number of characters remaining and `owed = (8 − n) mod 8`.

* `left = 0`: the stream ends. It is an error if `n ≠ 0`.
* `left = 1`: this is the final group; see Section 12.3.
* `left = 2` with `n ≠ 3`: form `V`. If `V ≥ 8192` the stream ends on a signal
  whose fields never arrived — reject with `UNEXPECTED_EOS`. Otherwise this is
  the final group; see Section 12.3.
* Otherwise read two characters and form `V = d0 + d1 × 91`:
  * `V ≥ 8192`: a signal; go to Section 12.4.
  * Otherwise `V` is a thirteen-bit symbol:

    ```
    b = (b << 13) | V ;  n += 13
    while n >= 8:  n -= 8 ;  emit (b >> n) & 0xFF
    b &= (1 << n) - 1
    ```

### 12.3 The final group

With `owed` as above:

| `left` | `n_end` |
|---|---|
| 1 | `owed`, which MUST be in `1 … 6` |
| 2, `n ≠ 3` | `owed` if `owed ≥ 7`, else `owed + 8`; MUST be in `7 … 12` |
| 2, `n = 3` | — this is a whole symbol, not a final group |

Read one character if `n_end ≤ 6` and two otherwise, forming `w` low digit first.
It is an error if `w ≥ 2^n_end`. Then

```
b = (b << n_end) | w ;  n += n_end
while n >= 8:  n -= 8 ;  emit (b >> n) & 0xFF
```

and the stream ends, with `n = 0` by construction.

### 12.4 A signal

```
if V == 8280:
    read one character -> e                  # 0 .. 89
    class = 44 + (e div 2) ;  hi = e mod 2
    error EXTENDED_CLASS                     # no class above 43 in 0.4.0
s     = V - 8192
hi    = s & 1
class = s >> 1
if class > 30:  error UNKNOWN_CLASS

n_enc = ((8 - n) % 8) + 8 * hi
if n_enc > 12:  error INVALID_FLUSH
if n_enc > 0:
    read one character -> f
    if n_enc > 6:  read a second -> f += d * 91
    if f >= 2^n_enc:  error INVALID_FLUSH
    b = (b << n_enc) | f ;  n += n_enc       # a multiple of 8 by construction
    while n >= 8:  n -= 8 ;  emit (b >> n) & 0xFF
b = n = 0

if class == 0:  read one pair -> p ;  if p > 1023: error INVALID_PARAMS
                mask = p & 255 ;  profile = p >> 8
read the length field per Section 7.3       -> L
if L == 0:  error INVALID_LENGTH
if class == 20:  if L > MAX_FRAME_BYTES:    error INVALID_LENGTH
else:            if L > MAX_SEGMENT_BYTES:  error INVALID_LENGTH
```

Then the payload: Section 8.4 for classes 0–6, Section 9 for 7–19, Section 10.1
for 20, Section 10.2 for 21 and 22, Section 10.3 for 23–30, whose length field
counts gaps rather than bytes and whose own bound is on the bytes it emits.
Block mode resumes immediately afterwards, with `b = n = 0`.

### 12.5 Conversion

```
digits_to_value(d0, d1)  :  V = d0 + d1 × 91
value_to_digits(V)       :  d0 = V mod 91 ;  d1 = V div 91
```

---

## 13. Error handling

Every rejection carries a code, and every one is the same error type, whichever
layer refused. A caller catches one type and switches on the code.

| Code | Condition |
|---|---|
| `INVALID_CHARACTER` | A significant character outside the alphabet |
| `UNEXPECTED_EOS` | The input ends while a field or payload is still required |
| `UNKNOWN_CLASS` | A class this version does not define |
| `EXTENDED_CLASS` | The escape, which this version cannot read |
| `INVALID_FLUSH` | `n_enc > 12`, or a flush field carrying more bits than `n_enc` |
| `INVALID_PARAMS` | A class-0 parameter pair above 1 023 |
| `INVALID_LENGTH` | Length zero, above the class's bound, or in a longer tier than necessary |
| `INVALID_FINAL_BLOCK` | A final group whose width the character count forbids, or one carrying more bits than are owed |
| `INVALID_INDEX` | A packed-base index at or above `b` |
| `INVALID_RUN_VALUE` | A `RUN` pair at or above 256, or a `RUN` pair of zero |
| `INVALID_CHAIN` | A `ZMIX` segment whose count is zero, or which emits more than `MAX_SEGMENT_BYTES` |
| `MALFORMED_PADDING` | Nonzero padding bits in a packed base (optional check) |
| `MALFORMED_FRAME` | A zstd frame the decompressor rejects, a skippable frame, or trailing bytes after one |

An implementation MUST NOT read outside its input buffer, and MUST NOT terminate
the process, on malformed input.

---

## 14. Implementation guidance (informative)

### 14.1 The decision point

For short inputs the encoder does not have to guess. If fewer than a few hundred
bytes arrive before the input ends, the encoder has seen all of it, and the class
choice is exact rather than heuristic — there is no threshold constant to tune
and no window that can be wrong. A useful shape:

```
read up to ~200 bytes
├─ input ended    → classify exactly, emit the best segment, done
└─ input continues → compression path if the caller asked for it,
                     otherwise the scan of Section 11.1
```

The number is an implementation choice, not a format constant: it never appears
in the stream and two encoders that pick differently both produce valid output.

### 14.2 Skipping binary stretches

On high-entropy input almost every position takes step 3 of Section 6.4 and
consumes one byte, so the scan is re-entered for nothing thousands of times per
kilobyte. Cheapen the test rather than skipping positions: a segment can open at
a position only if the class threshold's worth of members do, and on random input
that fails within the first two or three bytes for every class. Membership is a
256-entry bitmap per class, or one 256-entry byte table holding a bitmask of all
classes at once. A run test is one comparison against the previous byte and needs
no table at all.

### 14.3 Tracking the profiles at once

The passthrough scan keeps, per profile, the lowest rank any literal has held in
it. Four such numbers fit in one 32-bit word, one per byte lane, and both
operations the scan needs are then branch-free — see Base85N's specification,
section 11.2, for the lane arithmetic; it applies here unchanged.

### 14.4 Streaming

An encoder needs `MAX_SEGMENT_BYTES` of lookahead, because the length field
precedes the payload. A decoder needs none: every field says how long it is
before it starts. Neither holds state across a segment boundary — Section 7.2
flushes the accumulator explicitly and Section 12.4 resets it.

### 14.5 Encoding in parallel

Thirteen bytes are 104 bits are exactly eight symbols, so **block mode returns
to `n = 0` at every thirteenth byte and nowhere else**. That is the whole basis
of a parallel encoder: cut the input at multiples of `PARALLEL_ALIGN`, encode
each piece as though it were a stream of its own, and concatenate the results.
No piece needs to know what the piece before it held back, because a piece that
begins on a group boundary begins with an empty accumulator.

A piece must also *end* with one, and segments are what can break that: a
segment consumes bytes without passing them through the accumulator, so the
block-mode bytes after the last segment in a piece are what have to be a
multiple of thirteen, not the piece. Two ways to hold that:

* **Repair the seam.** Each worker encodes speculatively and reports where its
  last segment ended; a sequential pass then re-encodes the bytes from there to
  the boundary. The repair is bounded by one segment plus twelve bytes, and the
  output is **byte-identical to a serial encode**, so Section 11.3 still holds
  across thread counts.
* **Stop early.** A worker may decline to open a segment that would leave a tail
  its boundary cannot absorb. This needs no second pass and produces valid
  output that is *not* canonical, so an encoder that does it MUST say so.

The first is what a conforming parallel encoder should do, because an encoding
that changes with the number of threads is one that cannot be diffed, cached by
content or tested against a fixture.

Decoding does not parallelise the same way: a signal can begin at any pair, so
nothing but a sequential pass can say where segment boundaries are. A decoder
with a known-good split — its own earlier encode, or a stream it has already
walked once — can decode the pieces independently, and that is the only case.

---

## 15. Conformance

### 15.1 Structural

* The alphabet has 91 distinct characters, none of them `"`, `\`, `'` or below
  `0x20`, and `-` is at value 90.
* `R_CHARS` has eight distinct entries, none of them in the alphabet.
* Each profile has eight distinct alphabet characters and does not contain `-`.
* No pair value at or above 8 192 is in the range of the block coder. This is a
  proof, not a sample: a symbol is thirteen bits, its greatest value is 8 191,
  and a pair carrying a symbol is that value. An implementation SHOULD assert it
  anyway, because a counterexample would mean the whole signalling mechanism
  needs an escape clause.

### 15.2 Round trip

* Random binary at every length 0–300, plus 1 023, 1 024, 1 025, 65 535,
  65 536, 65 537.
* Text exercising every one of the 256 masks over the R-Set, and each of the
  four profiles.
* For every packed class: the empty-adjacent lengths 1, 2, 3, the lengths where
  `L × w` is and is not a multiple of 13, and a segment of `MAX_SEGMENT_BYTES`.
* Zero runs of every length 1–100, and at 8 369, 8 370, 65 536; the same for
  `RUN` with byte values 1, 127, 128, 255.
* For every `ZMIX` class: chains of 1, 2 and 89 gaps, a chain that ends because
  the gap width changes, one that ends on `MAX_SEGMENT_BYTES`, and a gap whose
  bytes include a zero that is not adjacent to a run.
* Hyphens — `-`, `--`, `---` and longer runs — at the start, in the middle and
  at the end of a passthrough payload, and immediately before and after a
  segment boundary.
* NUL inside a passthrough segment, alone and together with each of the other
  seven R-Set members.
* Mixed content exercising every block↔segment transition, with the pending bit
  count `n` taking each of its thirteen values at a transition.
* Every length tier of Section 7.3, at its boundaries: 89/90, 8 369/8 370.

### 15.3 Canonicity

* No active donor occurs as a literal inside an emitted passthrough segment.
* The emitted `profile` is the smallest viable one, and `mask` has a set bit for
  exactly the R-Set characters present.
* Classes 1–6 are used wherever they apply; class 0 never duplicates them.
* A zero run is never emitted as `RUN`, and a `ZMIX` chain is always maximal.
* The flush and length fields take the fewest characters that carry them.
* Two encoders implementing the same class set produce byte-identical output.

### 15.4 Adversarial decode

* Classes 31–43 and the escape, which MUST be rejected, not skipped.
* Every `hi` and pending-bit combination, valid and not, including `n_enc = 14`
  and `n_enc = 15`, which no encoder can produce.
* Length zero, lengths above the class bound, and a value written in a longer
  tier than necessary, in every tier.
* Packed indices at and above `b` for each class where `b < 2^w`.
* A `RUN` pair of zero and of 256.
* A `ZMIX` chain whose runs sum past `MAX_SEGMENT_BYTES`.
* A signal at the very end of the input, with each field truncated in turn, and
  a two-character stream whose pair is a signal.
* A `ZSTD` segment whose frame declares a content size larger than any ceiling
  the decoder will allocate, one with trailing bytes, and a skippable frame.
* Class-0 parameter pairs above 1 023.

### 15.5 Class subsetting

An encoder MAY implement any subset of the classes and remains conforming; its
output is valid and merely larger. A **decoder MUST implement all of them**,
because it cannot choose what it receives. An implementation that ships without
zstd MUST reject class 20 with `UNKNOWN_CLASS` and MUST say so in its
documentation — and it should be understood that this makes it a different
format in practice, not a smaller one.

---

## 16. Security considerations

base91-jdp is an encoding, not a cryptographic transform, and it makes no
integrity claim (Section 2.3). The decoder is the security-relevant surface.

* **A compressed segment expands.** Sections 7.3 and 10.1 bound the input; the
  output must be bounded by the caller. A stream of many small `ZSTD` segments
  can multiply a modest input into an arbitrary one, so the ceiling belongs on
  the total, not per segment.
* **So does a run.** Nine characters emit 65 536 bytes, and a stream of them
  emits that many again each time. The same total ceiling covers this; a decoder
  that bounds only the compressed classes has missed the cheaper amplifier.
* **Lengths are attacker-controlled.** Every length field must be range-checked
  before anything is allocated or indexed.
* **Whitespace skipping must be incremental**, or a padded stream costs
  quadratic time.
* **Output is arbitrary binary.** Callers MUST NOT assume it is printable,
  NUL-terminated or text, whatever the input looked like.
* **Truncation is undetectable.** A stream cut at a segment boundary decodes as
  a shorter valid stream. Callers who need to know that they have all of it
  carry a length or a digest outside the format.
* **A zstd frame's checksum covers that frame only.** It is not a checksum over
  the stream, and it does not protect passthrough, packed, run or block regions.

---

## 17. Measurements

Full method, per-file figures and every sweep: `bench/results/RESULTS.md`. The
corpus is fetched by `bench/corpus.py`; the projections below are produced by
`bench/uncompressed.js` and `bench/zstdprojection.js`.

> **What is and is not measured.** Everything below is the prototype in `rust/`
> encoding the corpus and decoding it again, except: Section 17.2, which is the
> 0.3.0 JavaScript codec; the ratios in Section 9, which are arithmetic; and
> the packed classes, which neither corpus exercises. The donor profiles are
> still 0.3.0's and were
> derived for an R-Set that held `-` rather than NUL (Section 17.5). Neither
> corpus contains a hex dump, a column of digits or a base64 blob, so nothing
> here measures the packed classes either (Section 17.15).

### 17.1 Corpus

Two groups, both Base85N's, unchanged.

The **core** group is 13 real files, 6.52 MB, fetched from pinned upstream
archives — three binary container formats, an uncompressed source tar, a JSON
dataset in both pretty-printed and minified form, JavaScript, CSS and Python
source, the CommonMark specification, a Markdown changelog, a JPEG and a PNG.

The **Silesia** group is the twelve files, 202 MiB, that compression work has
been reported against since 2003. It is here because thirteen files picked by a
codec's own author are a weak basis for a claim about real data: Silesia was
assembled by somebody else, for somebody else's benchmark, before this encoding
existed, and it contains input classes the core group has none of — a star
catalogue, two medical images, a chemical database, a dictionary.

The **short** group is different in kind: 55 field-level samples under 200
bytes each — identifiers, digests, tokens, timestamps, one record of JSON —
authored in `bench/wire_samples.py` from invented values and needing no
download. It exists because neither of the other two contains a hex dump, a
column of digits or a base64 blob, which is exactly what Section 9 is for, and
because three characters of segment overhead are invisible at a megabyte and
decisive at forty bytes. Until it existed, thirteen of the format's classes had
never been exercised by a benchmark at all.

The donor profiles are derived on a separate 2.37 MB training corpus
(`tools/traincorpus.py`) that shares no file and no upstream project with any
of the three.

### 17.2 What the fixed symbol costs

basE91's adaptive coder takes fourteen bits when the low thirteen are small, so
its density depends on the data. Against the fixed thirteen-bit symbol:

| file | adaptive | fixed | cost |
|---|---|---|---|
| bootstrap.css, countries.json, lodash.js and the other text | — | — | 0.000 % |
| minduka_present.png | 1.22869 | 1.23082 | 0.173 % |
| grace_hopper.jpg | 1.22879 | 1.23055 | 0.143 % |
| sql-wasm.wasm | 1.20838 | 1.22777 | 1.605 % |
| requests-2.32.3.tar | 1.04385 | 1.06491 | 2.018 % |
| _cffi_backend.so | 1.18713 | 1.22765 | 3.413 % |
| **core corpus** | 1.08442 | 1.09650 | **1.114 %** |

The fourteen-bit branch fires when a symbol's low thirteen bits are at most 88,
which zero-heavy binaries hit constantly: `_cffi_backend.so` was averaging 13.478
bits per pair, a 47.8 % branch rate against the 1.086 % of uniform data.

This is the price of the eighty-nine free pair values, and there is no version of
this format that pays less for them. It is also the cost the run classes of
Section 10.2 exist to take back, since the files that pay it most are the files
that are full of zeros.

### 17.3 The thresholds, re-swept

0.3.0's `MIN_BINARY_RUN` was measured against a structure this version does not
have, and the run break of Section 11.1 is new. Both were swept with the
prototype encoder over the core corpus, one at a time and then together, by
`rust/examples/sweep.rs`.

| `MIN_BINARY_RUN` | **0** | 1 | 2 | 3 | 4 | 6 | 8 | 16 |
|---|---|---|---|---|---|---|---|---|
| ratio | **1.00817** | 1.00892 | 1.00929 | 1.00983 | 1.01113 | 1.01529 | 1.01938 | 1.03114 |

Zero is the best value, which is to say the constant should not exist. In 0.3.0
it stopped passthrough from resuming too eagerly after it had been broken; with
typed segments there is nothing to stop, and forcing four bytes through block
mode after every segment costs 0.3 %.

| `MIN_RUN_IN_SEGMENT` | 2 | 4 | 6 | **8** | 10 | 13 | 20 | 40 |
|---|---|---|---|---|---|---|---|---|
| ratio | 1.01132 | 1.01126 | 1.01120 | **1.01113** | 1.01113 | 1.01113 | 1.01118 | 1.01167 |

The curve is flat from 6 to 20 and the differences there are in the fifth
decimal, so 8 is chosen on the plateau rather than at a fitted optimum.

The two do not move independently -- how long a run has to be before breaking
out pays depends on what happens after the segment that follows -- so the pair
was also gridded:

| `MIN_BINARY_RUN` | `MIN_RUN_IN_SEGMENT` | ratio |
|---|---|---|
| 4 (0.3.0's value) | 13 | 1.01113 |
| 0 | 13 | 1.00817 |
| **0** | **8** | **0.97944** |

Three percent of the corpus sits in that grid, which is more than any other
constant in this document is worth, and it is the reason Section 11.1 states
the run break normatively rather than leaving it to an encoder.

### 17.4 Without a compressor on either side

Characters per input byte once the output sits in a JSON string. `jdp 0.3.0` is
the previous version's headerless codec — passthrough and the block coder, and
nothing else. `jdp 0.4.0` is the projection of Sections 8, 9 and 10.2.

| | Base85N | jdp 0.3.0 | 0.4.0 projected | **0.4.0 measured** |
|---|---|---|---|---|
| core, 6.52 MB | 1.00698 | 1.09650 | 1.00464 | **0.97831** |
| Silesia, 202 MiB | 1.05114 | 1.09861 | 1.03434 | **1.03635** |
| both, 218 MB | 1.04982 | 1.09855 | 1.03345 | **1.03462** |

The last column is the prototype in `rust/`, encoding the corpus and decoding
it again. The one before it is what this document projected before that
prototype existed, and the two agree closely enough to say the arithmetic was
right -- but only after the prototype found what the arithmetic had missed. Its
first run over the corpus produced 1.03809 on the core group, not 1.00464,
because the passthrough scan was swallowing the zero runs that the run classes
exist to carry; the run break of Section 11.1 is that finding, and it is worth
2.3 percentage points of this table.

Read across a row. 0.3.0, which has passthrough and the block coder and nothing
else, is 8.9 % behind Base85N on the core corpus. The passthrough of Section 8
with NUL admitted, the packed bases of Section 9 and the run classes of
Sections 10.2 and 10.3 put it **2.85 % ahead on the core corpus, 1.41 % ahead
on Silesia and 1.45 % ahead over both**, with no compressor on either side.

Where the format stands on its own arithmetic rather than on a corpus: the block
coder is **1.2308 characters per byte against Base85N's 1.25**, 1.56 % denser,
on any input at all. Everything above is what the two formats' segment machinery
does on top of that.

### 17.5 The donor profiles

Derived greedily by `tools/deriveprofiles.js` on the training corpus:

| profiles | 1 | 2 | 3 | **4** | 5 | 6 |
|---|---|---|---|---|---|---|
| gain | — | 0.245 % | 0.067 % | **0.050 %** | 0.013 % | 0.019 % |

Four is where the curve flattens. Letters and digits are kept out of the
candidate pool on principle: a rare capital is rare across all text and common in
the one file that uses it, so it breaks segments in bursts.

These were derived with an eight-member R-Set that contained `-`. The membership
changed in 0.4.0 — `-` out, NUL in — so the derivation MUST be re-run before
this version leaves draft. The eighth rank is now reachable for the first time
and has never been fitted.

### 17.6 NUL in the R-Set, and why there is only one R-Set

Frequency is not the test; passthrough needs *contiguous* representable bytes.
The measurement is therefore the share of each corpus that lies in runs of at
least `MIN_DP_BYTES` representable bytes:

| R-Set | share of both corpora reachable by passthrough |
|---|---|
| the seven text characters | 60.2 % |
| **+ NUL** | **64.4 %** |
| + NUL + `0xFF` | 64.4 % |
| + NUL + `0x01` … `0x06` | 65.7 % |
| + NUL + `0x01` … `0x06` + `0xFF` | 65.8 % |

NUL alone is worth 4.2 points and costs text nothing: every text file in both
corpora is at 100 % before and after, because `mask` is exact and prose does not
contain it. Per file the gains are where the losses were — `requests-2.32.3.tar`
73.3 % to 99.9 %, `mr` 0 % to 28.9 %, `mozilla` 10.3 % to 18.9 %,
`_cffi_backend.so` 2.3 % to 9.0 %.

The six low control bytes add 1.3 points more and are **rejected**: they occur in
high-entropy regions where no run reaches `MIN_DP_BYTES` anyway, and admitting
them would take `k` to fourteen, burn six more donor slots in every segment that
touches one, and require profiles twice as long. `0xFF` is worth nothing at all.

That is also the answer to whether the format needs more than one R-Set, chosen
per segment the way the profile is. It does not. One eight-member set reaches
what is reachable, and a second set would have to be paid for by every segment
that names it, to buy the 1.3 points that the low control bytes are worth.

### 17.7 What the chained gaps are worth

Two designs were measured for Section 10.3, over both corpora, as the ratio they
would produce and its margin over Base85N:

| design | core | Silesia | both |
|---|---|---|---|
| no gap class | 1.02281 (−1.6 %) | 1.05013 (+0.1 %) | 1.04931 (+0.05 %) |
| two runs, one gap, `g ≤ 2` | 1.01988 (−1.3 %) | 1.04719 (+0.4 %) | 1.04637 (+0.33 %) |
| two runs, one gap, `g ≤ 8` | 1.01306 (−0.6 %) | 1.04128 (+0.9 %) | 1.04044 (+0.89 %) |
| **chained, `g ≤ 8`** | **1.00464 (+0.2 %)** | **1.03434 (+1.6 %)** | **1.03345 (+1.56 %)** |
| chained, `g ≤ 64` | 1.00061 (+0.6 %) | 1.02909 (+2.1 %) | 1.02824 (+2.06 %) |
| chained, `g ≤ 8`, width in a field | 1.01595 (−0.9 %) | 1.04403 (+0.7 %) | 1.04319 (+0.63 %) |

Three things follow, and each of them is a decision in Section 10.3.

**Chaining, not pairing.** A class carrying exactly two runs and one gap can
only swallow every other boundary of an alternating sequence, which costs half
the gain.

**The width belongs in the class.** The last row is the same design with a field
naming the gap width instead of eight classes: the field costs a character back
out of the one or two the merged signal saved, and two thirds of the gain with
it.

**Eight is where the curve is still cheap.** `g ≤ 64` is better again, but it
would take 64 classes of the 44 that exist and would need the escape. Eight
costs eight classes and reaches the widths the corpus actually has: in
`_cffi_backend.so`, gaps of three are the mode.

The `count` field of one character per segment is not modelled above and is
charged against these numbers, not for them.

### 17.8 With a compressor, on both sides

Every contender runs the same zstd frame; what differs is the container.

| | Base64 + zstd | Base85N + zstd | **base91-jdp** | margin |
|---|---|---|---|---|
| core, level 1 | 0.40620 | 0.38072 | **0.37497** | 1.51 % |
| core, level 3 | 0.37304 | 0.34954 | **0.34436** | 1.48 % |
| core, level 9 | 0.34035 | 0.31893 | **0.31419** | 1.49 % |
| Silesia, level 1 | 0.46076 | 0.43189 | **0.42532** | 1.52 % |
| Silesia, level 3 | 0.41824 | 0.39205 | **0.38607** | 1.53 % |
| Silesia, level 9 | 0.37270 | 0.34937 | **0.34403** | 1.53 % |

Those were computed from the frame lengths before an implementation of
Section 10 existed. There is one now, and it agrees: the prototype encodes the
core corpus at level 3 to **0.34445** against the 0.34436 projected here, a
difference of two parts in ten thousand, which is the length fields of the
frames the projection counted slightly differently.

The margin is the 1.56 % of the block coder, less what the segment framing
costs, and it does not move with the level or the corpus because nothing in it
depends on the data. base91-jdp is the smaller of the two on **all 25 files of
both corpora, at every level**, including the two that do not compress at all.

### 17.9 What a compressed segment's size costs

Section 10.1 leaves the payload per frame to the encoder. Over the core corpus,
at level 3, against a single frame over the whole input:

| payload per frame | ratio | cost |
|---|---|---|
| one frame | 0.34436 | — |
| 1 MiB | 0.34511 | +0.2 % |
| 256 KiB | 0.35181 | +2.2 % |
| 64 KiB | 0.36594 | +6.3 % |

A caller who wants a bound on what one damaged frame destroys can have it for
0.2 %; one who segments at `MAX_SEGMENT_BYTES` pays 6.3 %, which is why
Section 10.1 gives compressed segments a bound of their own.

### 17.10 Throughput, and what parallel encoding is worth

The prototype in `rust/`, release build, on a shared four-core virtual machine.
Run-to-run spread on this host is wide; read a factor as real and ten percent as
noise. These are the encoder without compression, since the prototype carries
no zstd.

| sample | serial | four threads | chunks spliced or rejoined |
|---|---|---|---|
| nci (32 MB) | 114 MB/s | 309 MB/s | 100 % |
| webster (40 MB) | 91 MB/s | 271 MB/s | 80 % |
| mr (9.5 MB) | 71 MB/s | 227 MB/s | 100 % |
| samba (21 MB) | 78 MB/s | 213 MB/s | 100 % |
| requests-2.32.3.tar | 75 MB/s | 169 MB/s | 100 % |
| _cffi_backend.so | 60 MB/s | 127 MB/s | 100 % |
| countries.json | 28 MB/s | 100 MB/s | 100 % |
| mozilla (49 MB) | 44 MB/s | 70 MB/s | 100 % |
| bootstrap.css | 99 MB/s | 101 MB/s | too small to chunk |

Section 14.5's arrangement holds. Most chunks were used as their worker wrote
them or met the sequential pass again at a shared segment boundary; the rest
were re-encoded outright, and the output is byte-identical to a serial encode
in every case, which the test suite asserts at four chunk sizes down to a
single symbol group.

It is worth recording what the first version of that join did, because the
shape is easy to get wrong. Splicing only where a worker's assumption held
outright -- an empty accumulator at its first byte -- fired on a fifth to a half
of chunks, and repairing a whole chunk when it did not left the parallel encoder
*slower* than the serial one. Bounding the repair at the first segment boundary
both paths reach is the difference between that and the table above.

**What the scan costs, and what a vector unit can do about it.** On input no
class can carry -- which is what a compressed payload is -- the block coder
alone runs at 323 MB/s and the whole encoder at 31. Five sixths of the time is
the candidate scan of Section 11.1, entered once per byte and finding nothing
once per byte.

The question the scan answers per byte can be answered per *window*: no run of
two equal bytes, no four bytes of one packed alphabet, no eight carriable bytes
in the next thirty-two means nothing can open at any of the first twenty-two
positions. That is three comparisons over a vector register, and it is
conservative by construction -- it can only ever say "scan here after all".

| input | scalar | with the probe | |
|---|---|---|---|
| high-entropy synthetic | 31 MB/s | 64 MB/s | 2.06× |
| `grace_hopper.jpg` | 32 MB/s | 62 MB/s | 1.94× |
| `sql-wasm.wasm` | 36 MB/s | 62 MB/s | 1.72× |
| `minduka_present.png` | 37 MB/s | 59 MB/s | 1.59× |
| `commonmark-spec.txt` | 87 MB/s | 103 MB/s | 1.18× |
| `DejaVuSans.ttf` | 31 MB/s | 30 MB/s | 0.97× |

This is a property of the format, not of one implementation: the three
thresholds the probe tests against are Sections 10.2, 9 and 11.1, and any
encoder can batch the same question. It is also the answer to why the scan is
not simply run once at the start of a stream — Section 14.1's decision point
holds only while the input is one kind of thing, and a `tar`, whose text
headers, binary members and zero padding alternate every few hundred bytes, is
the standing counterexample. Its ratio of 0.7528 is what switching classes
mid-stream buys; one decision at the head would give it 1.0 or 1.2308.

The same probe applied to the passthrough prefix scan loses, in four
arrangements; `rust/src/simd.rs` records each with its number. The bytes that
stop *that* skip are the R-Set members and the donors, which are the frequent
characters of the text it runs on, so it settles two or three bytes per call
where the dead-span probe settles thirty-two.

### 17.12 Not scanning, and what it is worth

Section 11.5 lets an encoder decide that a stretch needs no scan. Measured in
the prototype on four MB of high-entropy bytes:

| | stable | with `simd` |
|---|---|---|
| encoder, deciding per window | **2 030 MB/s** | **2 010 MB/s** |
| encoder, scanning everything | 31 MB/s | 125 MB/s |
| block coder alone | 3 090 MB/s | 3 090 MB/s |

Sixty times, and it needs no vector unit: the decision is a byte histogram over
a kilobyte per sixteen-kilobyte window. The vector mask of
Section 17.10 is what carries the cases the decision does not fire on, and the
two are complements rather than alternatives.

What it does to real compressed payloads -- which is the case the format is for,
since Section 10 puts a zstd frame inside a segment:

| payload | windows called block | ratio, deciding | scanning | speed |
|---|---|---|---|---|
| `countries.json` at zstd −3 | 11 of 11 | 1.2308 | 1.2304 | 30 → 2 276 MB/s |
| `lodash.js` at zstd −9 | 6 of 6 | 1.2308 | 1.2308 | 33 → 2 202 MB/s |
| the source tar, gzipped | 8 of 9 | 1.2308 | 1.2307 | 33 → 1 961 MB/s |
| `sql-wasm.wasm`, raw deflate | 20 of 20 | 1.2308 | 1.2308 | 33 → 2 126 MB/s |

Raw deflate is the row that justifies the entropy test on its own: it carries no
magic number, and nothing but its entropy says what it is.

And the false positives, over the whole core corpus: **none.** No window of any
of the eleven files that are not already compressed was called block mode, and
the corpus ratio moves from 0.97944 to 0.97945 — one part in a hundred thousand,
which is the JPEG and the PNG giving up the 0.1 % their last few windows would
have saved.

### 17.13 What the block coder costs

Thirteen bytes to sixteen characters is the floor under every other number
here, so it is worth saying what it is made of. Measured in the prototype, in
order of what each change was worth:

| | MB/s |
|---|---|
| a byte at a time through the accumulator | 256 |
| thirteen bytes at a time, division by 91 as a multiply and a shift | 549 |
| one big-endian load per group, output written through a pointer | 1 289 |
| **both digits from one 16 KiB table** | **3 090** |

The last step is the one that matters, and it is the one that removes
arithmetic rather than adding cleverness. A pair value is at most 8 191, so
8 192 entries of two bytes each -- sixteen kilobytes, half a typical L1 data
cache -- give both characters in one aligned load. There is then no division,
no reciprocal, and no alphabet lookup left in the coder at all.

**A vector unit does not help here**, which is worth recording because it is
not obvious. Extracting the eight symbols with two byte shuffles and a variable
shift, instead of eight shifts of a `u128`, measures at 1 180 MB/s against
3 050: the symbols have to leave the vector registers again for the table
lookup, and moving eight lanes out costs more than the shifts saved.
Assembling the sixteen characters into one register to store once rather than
eight times costs a further 9 %. Both are implemented and verified in the
prototype and neither is used; `rust/src/simd.rs` says what a vector path would
need instead, which is a fully vectorised digit conversion where nothing leaves
the registers until the store.

### 17.14 What compression costs in throughput

The container encodes at gigabytes per second. zstd does not, at any level
anyone would choose for size, and everything below follows from that.
`countries.json`, 1 408 911 bytes:

| level | chars/byte | whole encode | zstd alone | the container alone | decode |
|---|---|---|---|---|---|
| −5 | 0.2518 | 487 MB/s | 515 MB/s | 3 334 MB/s | 263 MB/s |
| −1 | 0.1842 | 457 MB/s | 471 MB/s | 3 342 MB/s | 296 MB/s |
| 1 | 0.1635 | 430 MB/s | 462 MB/s | 3 342 MB/s | 271 MB/s |
| 3 | 0.1511 | 325 MB/s | 365 MB/s | 3 336 MB/s | 277 MB/s |
| 9 | 0.1206 | 61 MB/s | 59 MB/s | 3 335 MB/s | 661 MB/s |
| 15 | 0.1065 | 10 MB/s | 8 MB/s | 3 338 MB/s | 677 MB/s |
| 19 | 0.0986 | 2 MB/s | 2 MB/s | 3 313 MB/s | 737 MB/s |

**Read the third and fourth columns together: they are the same number.** At
every level the whole encode runs at the compressor's speed, within the noise
of this machine, and the container's own contribution is a constant that does
not move — 3.3 GB/s from level −5 to level 19. Between the two there is a
factor of six at the fastest level and sixteen hundred at the slowest.

That is the shape of the control this format hands the caller. Choosing a level
chooses a point on zstd's curve; the format adds a fixed 1.2308 characters per
byte of frame to whatever comes out, and nothing else. **Any throughput claim
about a compressing encoder is a claim about zstd**, and the container is not
where the time goes.

Decoding is not symmetric, and the asymmetry is zstd's too: decompression is
roughly level-independent, so the high levels decode faster than the low ones
only because there are fewer bytes to unpack before handing them over.

Over the whole core corpus at level 3, with a frame per mebibyte
(Section 17.9):

| | ratio |
|---|---|
| no compressor | 0.97945 |
| always compress | 0.34445 |
| weighing both, Section 11.2 | 0.34444 |

The last row is worth its own sentence, because it is the rule the
specification states and it buys one part in thirty thousand. Compression wins
on eleven of the thirteen files; the two it loses on are the JPEG and the PNG,
where it loses by 0.0003 and 0.0016 characters per byte. It costs three to six
times the throughput, because building the uncompressed candidate means running
the scan of Section 11.1 over data the scan has plenty to find in: 331 MB/s
becomes 26 on `countries.json`, 382 becomes 85 on `bootstrap.css`.

### 17.16 Short payloads, and the classes only they reach

Fifty-five field-level samples, 2 381 bytes in all, none over 155. Against
Base64, which is what these fields are encoded with today:

| what the sample is | bytes | Base64 | base91-jdp | |
|---|---|---|---|---|
| hex digests and keys | 408 | 1.3725 | **0.6838** | −50.2 % |
| decimal identifiers | 130 | 1.4154 | **0.7462** | −47.3 % |
| binary, runs included | 216 | 1.3889 | **0.8287** | −40.3 % |
| Crockford base32 (ULIDs) | 78 | 1.3846 | **0.8462** | −38.9 % |
| base32 secrets | 96 | 1.3750 | **0.8542** | −37.9 % |
| UUIDs, hex with separators | 145 | 1.3517 | **0.8483** | −37.2 % |
| alphanumeric identifiers | 141 | 1.3901 | **1.0000** | −28.1 % |
| base64 and base64url | 327 | 1.3579 | **1.0230** | −24.7 % |
| protocol text | 840 | 1.3619 | **1.0750** | −21.1 % |
| **all of them** | **2 381** | **1.3709** | **0.9252** | **−32.5 %** |

Every packed class of Section 7.4 is chosen by something: `DEC` by an account
number, `HEXL` and `HEXU` by digests, `HEXL_D` and `HEXU_D` by UUIDs, `B32` by
a TOTP secret, `CROCK` by a ULID, `B64` and `B64U` by tokens, `ALPHA_L` and
`ALPHA_U` by slugs and codes. `ZRUN` takes thirty-two zero bytes in three
characters, and `ZMIX` takes a zero-padded record at 0.438.

Where the format does not win is where it should not: four bytes of digits stay
in block mode because `DEC` cannot pay for a signal there, and a name with
umlauts runs at 1.240 because a multi-byte character is not representable in
passthrough (Section 17.15).

**Two encoder faults were found by this group and by nothing else.**

The first was in the comparison of Section 11.1. Block mode emits only whole
symbols, so counting the characters it writes understates it — the remainder is
input it has consumed and has not yet paid for. Comparing written characters
against written characters favoured block mode by up to two characters, and on
a short payload two characters is the whole decision: six digits went to block
mode at eight characters where `DEC` takes seven. Weighing the deferred bits as
well is worth 0.12 % on the core corpus too, where it had been invisible.

The second is still open. The ranking of candidates is greedy, and it compares
candidates of different lengths by what each saves in total, which favours the
longer one — a JWT is three base64url runs separated by dots, passthrough
reaches all of it and a packed base reaches only the first run, so the token
goes to passthrough at 1.032 where three packed segments would be cheaper.
Ranking by saving *per byte consumed* instead is worse, not better: 0.98013
against 0.97831 on the core corpus, 0.9261 against 0.9252 here, and the JWT
itself goes to 1.039. That neither criterion dominates is what says the problem
is not the criterion but the greediness. Section 14.1 already observes that a
short input can be classified exactly rather than heuristically, and this group
is small enough to settle what that would be worth.

### 17.15 What is left on the table

* **UTF-8 above U+007F breaks passthrough.** A multi-byte character is not
  representable, so prose in a language that uses accents runs through block mode
  at 1.2308. `commonmark-spec.txt` and `requests-history.md` both pay for this.
  A codepoint-level class would fix it and is the most valuable unassigned class
  in Section 7.4.
* **The greedy ranking is not optimal**, and Section 17.16 shows a case where
  it costs: a JWT lands in passthrough where three packed segments would be
  cheaper. An exact segmentation over the pending-bit state is affordable for
  short inputs and nobody has measured what it would buy.
* **The short group is authored, not collected.** Its samples are invented, and
  chosen by someone who knew which classes existed — which is the right way to
  exercise the classes and the wrong way to estimate how often they occur. What
  fraction of real traffic is a hex digest is not a question this corpus can
  answer.
* **No speed claim is made against another implementation.** Base85N publishes C
  throughput; this repository has JavaScript. A comparison needs a C
  implementation of this format, and until there is one, size is the only axis
  on which the two have been compared.
* **A custom packed base**, carrying its alphabet explicitly rather than by
  class, would cover restricted alphabets the table does not name. It costs
  roughly `b` characters to declare, so it only pays on long segments.
* **Layout-aware classes** — a UUID as 32 hex digits with the four hyphens
  implied — would take a UUID from `w = 5` to `w = 4`, about 8 characters on 36.
  That requires a normative statement about where the hyphens go, which is a
  different kind of assumption from "these bytes are hex".

---

## 18. What was considered and left out (informative)

### 18.1 Reed-Solomon, the check pattern, and CRC

0.3.0 carried Reed-Solomon over GF(2¹³) at 0.098 % overhead, a check pattern
riding in the free pair values at no cost in characters, a 256 KiB segment
structure with `--` separators, and a bound of two segments on the damage one
flipped bit could do. All of it is gone.

The reasoning, in order:

* **The channel does not ask for it.** The four contexts in Section 1 — API
  payload, log field, database column, document inside a document — all have
  integrity from the layer beneath, and the damage they actually produce is
  truncation, transcoding and hand-editing, none of which Reed-Solomon repairs.
  Its own measurement showed a four-character burst already costing a whole
  segment.
* **Sender-side memory faults, the one gap the lower layers leave, are not
  covered either.** Corruption before the parity is computed is protected *into*
  the codeword and decodes as sound. An encoder that cares about this should
  decode its own output and compare, which covers the whole pipeline rather than
  its last few microseconds.
* **The check pattern had no floor.** Its capacity is a function of the symbol
  distribution, and on highly compressible data a whole segment can carry a
  dozen check bits or none at all.
* **A CRC was considered as the floor and rejected on scope.** It costs six
  characters per segment, which is nothing, but it only buys back an integrity
  guarantee this format had no business making. Base64 makes none either.

What remains of the concern is real and is answered differently: a single zstd
frame over a large input means one flipped bit destroys all of it. Section 10.1
lets an encoder bound that by choosing the payload per frame, and Section 17.9
prices the choice — 0.2 % for a megabyte. That is a caller's decision about
their channel, not a property the format imposes on everyone.

### 18.2 A fill mode

Earlier drafts of this version left it out, on the grounds that a fill mode and
a compressor address the same redundancy — a run of identical bytes — and that
wherever a run is long enough for a fill mode to pay, compression pays more.

That argument is true and was beside the point. The comparison that decides
whether this format is worth using is frequently made **without a compressor on
either side**, because the caller does not want one, cannot link one, or is
encoding something too short for one to help. In that comparison the run
classes are not redundant with anything: they are 4.7 percentage points of the
core corpus and the entire distance between losing to Base85N and beating it
(Section 17.4).

They also cost nothing where they do not apply. Every text file in both corpora
emits not one run segment, and the scan that establishes this is one comparison
against the previous byte.

### 18.3 `-` in the R-Set

0.2.0 and 0.3.0 substituted `-` like an R-Set member, because a doubled hyphen
was the segment exit signal and could not be allowed into a payload. It was worth
0.29 % over the corpus at the cost of one donor character in every segment
containing a hyphen.

Length delimiting makes the exit signal unnecessary, so both the constraint and
its cost are gone: `-` is a plain literal, `--` inside a payload means nothing,
and every segment that contains a hyphen keeps a donor it used to spend. The
slot it vacated is what NUL now occupies (Section 17.6), so the R-Set stayed at
eight members and gained the one byte that binary data is made of.

### 18.4 Head-of-stream mode markers

0.3.0 had two signalling mechanisms: a marker pair valid only at the head of a
stream, choosing between four framed modes, and a `--`-plus-header signal for
passthrough anywhere else. 0.4.0 has one. A segment signal is a segment signal
wherever it occurs, compression is a class like any other, and the distinction
between "framed" and "headerless" streams is gone.

The cost is one to three characters where the whole stream is one compressed
segment. The saving is one signalling mechanism, one grammar, and one set of
conformance tests.

### 18.5 LZ4, and deflate

0.3.0 used the LZ4 block format, and justified it on the grounds that a
specification demanding deflate demands a library while LZ4 can be written in a
few hundred lines. That argument does not survive contact with the actual
constraint: an implementer who wants compression will link a library whatever
this document says, and one who does not want it omits class 20.

zstd replaces it, and replaces deflate as the alternative too, because its level
range is the size-against-speed control this format wanted to give the caller
and was otherwise going to have to invent (Section 17.10). Its frame format also
carries the length, the content size and the checksum that 0.3.0 specified by
hand.

### 18.6 A shared dictionary, and an entropy coder

Both were considered for the 40-to-100-byte range, where LZ77-family compressors
have nothing to work with because their window is empty.

Both work. Both require the decoder to hold something the stream does not carry —
a dictionary, or a frequency table — and that ends the stream's ability to
describe itself, which is the property everything else here is built on. A caller
with millions of similar short records is better served by zstd with a dictionary
they trained on their own data, outside this format, than by any generic table
this document could freeze.

The packed bases of Section 9 are what remains once that constraint is applied:
they exploit the only redundancy that needs no shared knowledge, which is that
the input alphabet is smaller than the output alphabet.

### 18.7 A gap width in a field

Section 10.3 spends eight classes on eight gap widths rather than one class and a
field. The field is the obvious design and it is the worse one, because the whole
gain of merging two run segments is the signal pair it saves — one or two
characters after rounding — and a field naming the width costs one of them
straight back. Section 17.7 measures both: the field version keeps a third of the
gain, and leaves the core corpus behind Base85N where the class version passes it.

### 18.8 More than one R-Set

Section 17.6 measures it and the answer is no: one eight-member set reaches
64.4 % of both corpora, and the six low control bytes that would make a second,
binary-oriented set worth naming are worth 1.3 points, in regions where no run
is long enough for passthrough to open anyway.

### 18.9 The adaptive thirteen-or-fourteen-bit coder

basE91's own block coder is denser than the fixed one by up to 3.4 % on
structured binary (Section 17.2). It reaches every one of the 8 281 pair values,
which would leave the format nothing to signal with. Making a header mandatory at
the head of every stream would buy the adaptive coder back at a cost of two to
three characters — but it would also remove the free pair values that make
Section 7 work at all, and every segment would need a different mechanism. It is
kept in the reference implementation as a benchmark and is not part of the
format.

### 18.10 Skippable unknown segments

An earlier design let a decoder step over a class it did not implement and report
the gap, so that new classes could be deployed without a flag day. It requires
the core to know a segment's character count without knowing its class, which the
length field cannot give — it is in bytes for most classes, gaps for `ZMIX`, and
the bytes-to-characters ratio is the class's own business. Unknown classes are
therefore a hard error (Section 15.5), which is at least honest: a decoder says
what it cannot read rather than silently returning less than it was given.

---

## 19. References

* Joachim Henke, *basE91*, 2005. <http://base91.sourceforge.net/>
* Keywan Ghadami, *Base85N v0.5.0*, 2026. <https://base85n.ghadami.de/> — the
  passthrough design, the R-Set and donor-profile mechanism, the fill mode that
  Section 10.2 answers, and both benchmark corpora are taken from it.
* RFC 8259, *The JavaScript Object Notation (JSON) Data Interchange Format*.
* RFC 8878, *Zstandard Compression and the application/zstd Media Type*.
* RFC 4648, *The Base16, Base32, and Base64 Data Encodings*.

---

## 20. Review and feedback are welcome

This document is a draft, and it is more useful to us reviewed than admired.
Corrections, objections, "this cannot be implemented as written" and "you have
measured the wrong thing" are all equally welcome — as issues, pull requests or
mail. Nothing here is settled by seniority; a counterexample settles it.

Some parts would benefit from a second reader more than others, so if you have
an hour rather than a week, these are where it is best spent:

* the pair-space argument in Section 5.2, on which all signalling rests;
* the flush derivation in Section 7.2, the escape's `hi` in 7.1, and the length
  tiers in 7.3;
* the padding and index rules in Section 9, and the chain structure in 10.3,
  which are the only new decode paths;
* the never-worse-than-block-mode guarantee in Section 11.2;
* the allocation bounds in Section 16, runs included and not only frames.

Two things we already know are open, and would rather hear about early than
late. The measurement caveat at the head of Section 17 is a release blocker
rather than a footnote: Sections 9, 10.2 and 10.3 carry projections rather than
measurements, the donor profiles must be re-derived for the changed R-Set
(Section 17.5), and neither corpus can say anything about the packed classes
(Section 17.15). And no implementation has yet encoded a byte against this
document, so every claim in it is an argument rather than a result.
