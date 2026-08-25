# base91-jdp: basE91 on a JSON-safe alphabet, with Dynamic Passthrough

| Field | Value |
|---|---|
| Version | 0.3.0 |
| Status | Draft |
| Date | 2026-08-23 |
| License | MPL-2.0 |

> **Draft.** The wire format described here is complete, implemented and
> measured, but it has not been in the field. Eighty-four of the eighty-eight
> mode markers are unassigned, and one of them is reserved to say "a longer
> header follows", so the format has room without spending any of it today.

---

## 1. Abstract

base91-jdp represents arbitrary data as text for the case where the result has
to be embedded in **JSON** -- an API payload, a log field, a database text
column, a document inside a document -- and where the size of that result
matters.

Its block coder is basE91 (Joachim Henke, 2005) with one substitution in the
alphabet: `"` is dropped and `-` takes its place. basE91 already omits `\` and
`'`; with `"` gone as well, the alphabet contains none of the characters a JSON
string has to escape, so encoded output can be pasted between quotation marks
verbatim and the encoded size *is* the final size.

The substitution decides the rest of the format. `-` lands on the alphabet's
last value, 90, so the pair `--` is worth 8 280 -- above everything a
thirteen-bit symbol can spell. base91-jdp fixes symbols at thirteen bits rather
than letting them float between thirteen and fourteen as basE91 does, which
leaves **eighty-nine pair values that no encoded stream can contain**. Those
eighty-nine values carry everything the format says about itself:

* `--` opens and closes a **passthrough** segment, in which text-like input is
  written one character per byte instead of being expanded 1.23 times, and
  divides one segment of a framed stream from the next;
* the eighty-eight values below it are **mode markers**, two characters at the
  head of a stream that say it carries LZ4 compression, error correction, or
  both.

A stream that wants neither pays nothing: no marker, no header, no padding.

---

## 2. Introduction

### 2.1 Design summary

* **Symbols are thirteen bits, always.** Two characters are one pair, worth
  `d0 + d1 x 91`; thirteen bytes are eight symbols in sixteen characters,
  exactly. This costs 0.08 % against basE91 on high-entropy data and up to
  3.4 % on structured binary (Section 18.2), and it buys three things nothing
  else in the design could have: eighty-nine free pair values, a symbol layer
  a Reed-Solomon code can sit on, and a bound of three bytes on what one
  damaged character can reach.

* **Passthrough** carries text at one character per byte. Seven byte values
  that real text is full of and the alphabet does not contain -- space, `"`,
  newline, `\`, carriage return, `'`, tab -- are carried as stand-ins borrowed
  from the alphabet's rarest characters, and `-` is carried the same way so
  that the exit signal can never occur inside a segment.

* **A marker** of two characters, drawn from values no packer can write, says
  what a stream is. Detection is total: there is no escape clause and no
  probability attached.

* **LZ4** compresses, in its block format, so that an implementation needs no
  dependency. The dictionary is reset every 256 KiB so that damage stays local.

* **Reed-Solomon** over GF(2^13) repairs two damaged symbols per 4 096-symbol
  codeword for 0.098 % of the stream, and a **check pattern** rides in the
  free pair values at no cost in characters at all.

* **The encoder decides by measuring.** It builds the framed candidate, works
  out what it would cost, and compares that against the headerless one. The
  size at which a marker starts paying for itself is a measurement (Section
  14.5), not a constant.

### 2.2 Key properties

| Property | Value |
|---|---|
| Alphabet | 91 characters, none of which JSON escapes |
| Expansion, incompressible input | 1.2308 characters per byte |
| Expansion, text without compression | ~1.00 characters per byte |
| Ratio over the benchmark corpus | 0.50264 characters per byte |
| Overhead of a headerless stream | zero characters |
| Overhead of a marker | two characters |
| Overhead of error correction | 0.098 % |
| Overhead of the check pattern | zero characters |
| One flipped bit, protected stream | repaired |
| Worst damage from one flipped bit | two segments, 512 KiB of payload |

### 2.3 What this format is not

It is not a general-purpose archiver and it is not the smallest thing you can
do to bytes. Deflate compresses better than LZ4: over the benchmark corpus,
Base85N applied to deflated bytes reaches 0.340 where base91-jdp reaches 0.503
(Section 18.1). What base91-jdp offers against that is one tool rather than
two, a stream that says what it is, error correction, and a damage bound --
and never expanding data that will not compress, where a deflate pipeline goes
to 1.334 characters per byte on a JPEG and base91-jdp stays at 1.231.

It is also not a security mechanism. The check pattern detects accident, not
an adversary; see Section 17.

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
by frequency in the corpus of Section 18.

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
* which character sits on value 90 stops being a size decision (Section 18.7).

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

Derivation and the reason there are four of them are in Section 18.5.

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

A pair's value is `d0 + d1 x 91`, **low digit first**. Every multi-character
field in this format -- the passthrough header of Section 6.4, the pending-bit
field of Section 6.5, the mode marker of Section 5.2 -- uses that same
convention.

### 5.1 The symbol stream

Bytes become thirteen-bit symbols **most significant bit first**. The
accumulator takes bytes in at the bottom and gives symbols off the top:

```
acc = (acc << 8) | byte ;  nb += 8
while nb >= 13:
    nb -= 13
    emit symbol (acc >> nb) & 8191
```

Thirteen bytes are 104 bits are exactly eight symbols, so a whole group of
thirteen bytes is sixteen characters with nothing left over. A remainder of one
to twelve bits becomes a final short symbol, and how wide it is follows from
the character count alone (Section 6.8).

### 5.2 What a pair may be

| Range of V | Meaning |
|---|---|
| 0 ... 8 191 | a thirteen-bit symbol |
| 8 192 ... 8 279 | at the head of a stream, a mode marker (Section 9); inside a framed body, a symbol carrying a side-channel bit (Section 11) |
| 8 280 | `--`: the passthrough signal, and the framed-segment separator |

**No packed stream can spell 8 192 or above.** That is the whole basis of the
format's self-description, and it is why symbols are fixed at thirteen bits: a
coder that let them float to fourteen, as basE91 does, would reach 8 280 and
leave nothing free.

---

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
acc = (acc << 8) | byte ;  n += 8
if n >= 13:
    n -= 13
    v = (acc >> n) & 8191 ;  acc &= (1 << n) - 1
    emit ALPHABET[v mod 91], ALPHABET[v div 91]
```

`n` is never more than twelve between symbols, because a thirteenth bit becomes
one. There is no threshold and no branch: every symbol is thirteen bits.

A stream that never enters passthrough is therefore exactly the byte-synchronous
packing of Section 5.1 -- thirteen bytes to sixteen characters -- and an
implementation MAY produce it that way.

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

`n_enc > 12` is malformed -- twelve, not thirteen, because a thirteenth bit
would have become a symbol. The number of characters the pending bits occupy
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

When the input ends in block mode, emit the `n` pending bits as a final group:
nothing if `n = 0`, one character of value `acc` if `1 ≤ n ≤ 6`, two characters
of value `acc` if `7 ≤ n ≤ 12`.

This is the same rule as Section 6.5's pending-bit field, and it makes the
final group **self-delimiting**. A decoder holding `r` bits knows the writer
owed `(−r) mod 8` or eight more; one character can only carry the first if it
is six bits or fewer, two characters only the second, and the one case where
two characters could mean either -- three held bits -- cannot arise, because
after three held bits a whole symbol is the only thing that closes the stream.
Section 7.5 states the rule from the decoder's side.

### 6.9 Constants

| Constant | Value | Notes |
|---|---|---|
| `SYMBOL_BITS` | 13 | Fixed; there is no threshold and no fourteen-bit branch |
| `MIN_DP_BYTES` | 26 | Shortest segment at which passthrough is never larger than block mode |
| `MIN_BINARY_RUN` | 4 | Block-mode bytes before passthrough may resume (Section 18.4) |
| `MAX_DP_BYTES` | 65 536 | Encoder lookahead bound; makes the output canonical and the encoder's memory finite |
| `HEADER_CHARS` | 2 | Passthrough header width |
| `NUM_PROFILES` | 4 | Donor profiles (Section 18.5) |
| `R_LEN` | 8 | R-Set size, and the width of `mask` |
| `SEGMENT_BYTES` | 262 144 | Payload per framed segment (Section 14) |
| `RS_DATA` | 4 092 | Data symbols per codeword (Section 16) |
| `RS_PARITY` | 4 | Parity symbols per codeword |
| `SIDE_COUNT` | 88 | Symbol values carrying a side-channel bit (Section 11) |
| `SIDE_MIX` | 8 179 | Which 88, as `v x 8179 mod 8192 < 88` |
| `SIDE_UNMIX` | 4 411 | Its inverse modulo 8 192 |

`MIN_DP_BYTES` is derived rather than fitted. A segment of `L` bytes costs
`L + 6` characters -- two for the entry signal, two for the header, two for the
exit -- while block mode charges `16/13` characters per byte. `L + 6 ≤ 16L/13`
gives `L ≥ 26`.

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

Let `left` be the number of characters remaining and `owed = (8 - n) mod 8`,
the bits the writer still owed on the byte in hand, modulo eight.

* `left = 0`: the stream ends. It is an error if `n ≠ 0`.
* `left = 1`, or `left = 2` with `n ≠ 3`: this is the final group. See
  Section 7.5.
* Otherwise read two characters and form `V = d0 + d1 x 91`:
  * `V = 8280` -- the pair `--` -- is the mode signal; go to Section 7.4.
  * `V ≥ 8192` is an error, `RESERVED_PAIR`: no encoder can write such a pair,
    so the stream is damaged, or it is a framed stream being read as a
    headerless one.
  * Otherwise `V` is a thirteen-bit symbol:

    ```
    b = (b << 13) | V ;  n += 13
    while n >= 8:  n -= 8 ;  emit (b >> n) & 0xFF
    b &= (1 << n) - 1
    ```

### 7.5 The final group

With `owed` as above:

| `left` | `n_end` |
|---|---|
| 1 | `owed`, which MUST be in `1..6` |
| 2, `n ≠ 3` | `owed` if `owed ≥ 7`, else `owed + 8`; MUST be in `7..12` |
| 2, `n = 3` | -- this is a whole symbol, not a final group |

Read one character if `n_end ≤ 6` and two otherwise, forming `w` low digit
first. It is an error if `w ≥ 2^n_end`. Then

```
b = (b << n_end) | w ;  n += n_end
while n >= 8:  n -= 8 ;  emit (b >> n) & 0xFF
```

and the stream ends, with `n = 0` by construction.

The three-held-bits case is the only one where two characters could mean either
a final group or a whole symbol, and it resolves because `3 + 13 = 16`: after
three held bits a whole symbol closes the stream exactly, while the final-group
reading would need thirteen bits, which no writer can owe.

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
if n_enc > 12:  error INVALID_FLUSH

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

## 9. Modes and the marker

### 9.1 Reading the head of a stream

A decoder forms the value of the **first pair** and dispatches on it:

| First pair | The stream is |
|---|---|
| 0 ... 8 191 | **headerless**: Sections 6 and 7, and nothing else |
| 8 280 (`--`) | **headerless**, opening in passthrough |
| 8 192 ... 8 279 | **framed**: the value names the mode |

The rule is total. A headerless stream cannot begin with a marker by accident,
because Section 5.2 says no packer can write one. There is no escape clause,
no exclusion rule in the encoder, and no probability attached -- which is what
the fixed thirteen-bit symbol was bought for.

Every marker's second character is `-`, since `91 x 90 = 8190` puts the high
digit at 90 for everything from there up. Classic basE91 carries `"` on value
90 and cannot produce `-` at all, so a `-` in second place also answers "is
this base91-jdp or is it classic basE91?", and classic basE91 needs no flag
anywhere to stay out of band.

### 9.2 The modes

| Marker | Value | Compression | Error correction | Check pattern |
|---|---|---|---|---|
| `~-` | 8 279 | none | Reed-Solomon | yes |
| `}-` | 8 278 | LZ4 | Reed-Solomon | yes |
| `\|-` | 8 277 | none | none | yes |
| `{-` | 8 276 | LZ4 | none | yes |
| `<-` | 8 266 | -- | -- | escape: a longer header follows |

The check pattern is in every framed mode, protected or not, because it costs
no characters. The difference between a Reed-Solomon mode and a checked one is
whether damage can be repaired or only reported.

Passthrough appears in none of them. It cannot coexist with either: compressed
bytes have nothing a passthrough segment could carry, and passthrough writes
one character per byte, which destroys the pair grid the error correction is
counted on. Passthrough is what a *headerless* stream does.

The other eighty-three marker values are unassigned and MUST be rejected with
`UNKNOWN_MODE`. The escape MUST be rejected with `EXTENDED_HEADER`: a decoder
that does not implement it must say what it cannot do, not read the stream
wrongly.

### 9.3 The passthrough header

The header that follows an entering `--` has its own 8 281 values:

| Range of h | Interpretation |
|---|---|
| 0 ... 2 047 | Passthrough segment: `hi`, 8-bit `mask`, `profile` 0-3 |
| 2 048 ... 8 280 | `FUTURE_SIGNAL_SPACE`. MUST be rejected. |

---

## 10. The framed body

A framed stream is the marker pair of Section 9, then:

```
body    = segment ( "--" segment )*
segment = codeword+
```

The separator is the point of the arrangement. No packed symbol can spell
8 280, so a reader that has lost its place finds the next separator and carries
on. Segment boundaries are therefore **not a chain**: nothing has to survive in
segment *i* for segment *i+1* to be found, and there is no length field anywhere
in the stream that one damaged symbol could take out.

### 10.1 What a segment holds

A segment carries at most `SEGMENT_BYTES` of payload. Its data symbols spell:

```
pad ‖ block ‖ 0 x pad
```

where `block` is the LZ4 block of Section 13 in a compressing mode and the
payload bytes themselves otherwise, and `pad` is a single byte, `0 ≤ pad < 13`,
saying how many bytes were appended to bring the segment up to a whole number
of thirteen-bit symbols:

```
pad = (13 - ((1 + len(block)) mod 13)) mod 13
```

That one byte is the entire framing overhead. It is what tells the decompressor
where the block stops, and it is the reason a framed body never has a short
final symbol: every segment is a whole number of symbols by construction, so
every pair in a framed body is a full pair and the character count is even.

A `pad` of 13 or more is malformed, as is a segment shorter than its own
padding.

### 10.2 The damage bound

* A codeword that error correction cannot repair costs **its segment**. The
  LZ4 dictionary is reset at every segment, so nothing after the damage inside
  that segment can be trusted, and nothing outside it is affected.
* A separator destroyed outright costs **two** segments, because the ones on
  either side of it merge and neither then parses.
* Nothing costs a third.

With `SEGMENT_BYTES` at 256 KiB that is at most **512 KiB of payload for one
flipped bit**, whatever the size of the stream. Section 18.5 measures it.

A decoder MUST report which segments it could not recover. It MAY return the
ones it could, which is the reason segments exist; a caller that wants all or
nothing checks the report and refuses.

---

## 11. The side channel and the check pattern

Section 5.2 leaves 8 192 ... 8 279 unused inside a framed body. Eighty-eight
symbol values may therefore be written as one of those instead, which carries
one bit **without moving a single character**.

### 11.1 Which symbols

```
slot(v)      = (v x SIDE_MIX) mod 8192            # SIDE_MIX = 8179
carries(v)   = slot(v) < SIDE_COUNT               # SIDE_COUNT = 88
raise(v)     = 8192 + slot(v)                     # the "and a one bit" value
lower(u)     = ((u - 8192) x SIDE_UNMIX) mod 8192 # SIDE_UNMIX = 4411
```

`lower` is defined for 8 280 as well, so a damaged separator lands on a symbol
the field has rather than outside it.

Which eighty-eight values is a free choice, and it is the choice that decides
how much the channel carries. Thirteen-bit symbols are nothing like uniform, so
a contiguous window collapses: measured over forty distributions, the top
eighty-eight values carry 0.000 % in the worst case and the bottom eighty-eight
carry 0.000 % as well, though they average 11 %. A scattered window carries
0.5 % in the worst case and 4.4 % on average. `SIDE_MIX` is 8 179, which is
-13 modulo 8 192; thirteen is the symbol width, so multiplying by it steps the
window across bit-alignment classes rather than along the grain of the data.
Section 18.7 has the table.

Nothing in the format is load-bearing on this channel. A stream whose data
happens to fill none of the window decodes exactly as well; it simply has no
check pattern.

### 11.2 The check pattern

For each codeword, sixty-four bits are derived from its **data symbols** and
its **index within its own segment**, and written into that codeword's side
channel slots in order, cycling if there are more than sixty-four.

The index is per segment and never per stream. A stream-wide counter puts every
later segment out of step the moment a separator is lost and two segments
merge -- exactly the coupling between segments that the separator exists to
remove. This was measured before the index was made local: one burst of 256
characters cost eleven segments of sixteen.

The order of operations is fixed and matters:

* the encoder computes parity (Section 12) **first**, then finds the slots in
  the encoded codeword, parity included, then writes the bits. Nothing is
  circular: both are settled before a single bit is written.
* the decoder strips the side channel **before** the parity check, because
  8 280 is not a value GF(2^13) has, then repairs, then finds the slots from
  the **corrected** symbols and reads the bit values from the **wire**.

That last split is what keeps a repaired symbol from shifting every bit after
it. A symbol error correction put back has a wire value that is neither of the
two its corrected value allows, so it shows up as one slot the reader knows it
cannot trust, and mismatches at untrusted slots are ignored.

A codeword whose trusted slots disagree with the pattern MUST be treated as
damaged even if the parity check passed. That is what closes the hole where
Reed-Solomon, overwhelmed, lands on a different valid codeword.

The parity itself cannot ride in the side channel: the slot positions come from
the symbol values one would need the parity to correct. Circular.

---

## 12. Error correction

Reed-Solomon over GF(2^13), generator polynomial `x^13 + x^4 + x^3 + x + 1`
(0x201B), systematic, with shortened codewords. Each segment's data symbols are
cut into codewords of `RS_DATA = 4092` symbols, the last one short, and each
gets `RS_PARITY = 4` parity symbols appended -- enough to repair **two damaged
symbols per codeword**.

The overhead is `RS_PARITY / (RS_DATA + RS_PARITY)` = **0.098 %**.

The field is chosen, not inherited. The channel damages *characters*, and a
character is half a pair; byte-level parity counts *bytes*, and one damaged
character reaches two or three of them. Over GF(2^8) the same protection needs
six parity bytes and costs 2.4 %. Over GF(2^13) one pair is exactly one symbol,
one damaged character is exactly one damaged symbol, and the cost is a
twenty-fifth of that. This is the reason symbols are a fixed thirteen bits.

---

## 13. Compression

The **LZ4 block format**, unchanged: a token byte whose high nibble is the
literal length and whose low nibble is the match length less four, then any
continuation bytes of 255, then the literals, then a two-byte little-endian
offset, then any match-length continuation. The final sequence is literals and
stops.

Two rules of the format constrain an encoder: the last five bytes of a block
are always literals, and no match may begin within the last twelve. A block
shorter than thirteen bytes is therefore all literals.

LZ4 rather than deflate because a specification may demand it without demanding
a library: a complete implementation is a few hundred lines. It compresses less
well, and Section 18.1 says by how much.

The dictionary does not survive a segment. That is what makes the damage bound
of Section 10.2 hold, and it costs almost nothing, since LZ4's match offset is
sixteen bits and cannot reach further back than 64 KiB in any case.

---

## 14. Error handling

Every rejection carries a code, and every one is the same error type, whichever
layer refused. A caller catches one type and switches on the code.

| Code | Condition |
|---|---|
| `INVALID_CHARACTER` | A significant character outside the alphabet |
| `UNEXPECTED_EOS` | The input ends while a pair, a header or a pending-bit field is still required |
| `UNDEFINED_SIGNAL` | A passthrough header value in `FUTURE_SIGNAL_SPACE` |
| `INVALID_FLUSH` | `n_enc > 12`, or a pending-bit field carrying more bits than `n_enc` |
| `INVALID_FINAL_BLOCK` | A final group whose width the character count forbids, or one carrying more bits than are owed |
| `RESERVED_PAIR` | A pair in 8 192 ... 8 279 inside a headerless stream |
| `UNKNOWN_MODE` | A marker no mode claims |
| `EXTENDED_HEADER` | The escape marker, which this version cannot read |
| `MALFORMED_FRAME` | A segment whose padding, or whose LZ4 block, does not add up |
| `MALFORMED_PAIRS` | An odd number of characters where pairs were required |
| `DAMAGED_SEGMENT` | A segment error correction could not recover |

An implementation MUST NOT read outside its input buffer, and MUST NOT
terminate the process, on malformed input. A decoder MUST NOT return bytes
from a damaged segment as though they were sound; it either omits that segment
and says so, or refuses the stream.

---

## 15. Implementation guidance (informative)

### 15.1 Skipping binary stretches

On high-entropy input almost every position takes step 3 and consumes one byte,
so the scan is re-entered for nothing 8 000 times per 8 kB. An encoder may bail
out of the scan early instead: passthrough can begin at a position only if
`MIN_DP_BYTES` representable bytes do, and 158 of the 256 byte values are not
representable, so on random input that test fails within the first two or three
bytes.

Unlike a block-aligned format, there is nothing to be gained by *skipping*
positions: block mode here consumes one byte at a time, and every position is a
decision point. Cheapening the test is the whole optimisation.

### 15.2 Tracking the profiles at once

The scan keeps, per profile, the lowest rank any literal has held in it. Four
such numbers fit in one 32-bit word, one per byte lane, and both operations the
scan needs are then branch-free — see Base85N's specification, section 11.2,
for the lane arithmetic; it applies here unchanged with four lanes instead of
eight.

### 15.3 Streaming

An encoder needs `MAX_DP_BYTES` of lookahead and nothing else; a decoder needs
no lookahead at all beyond one character, to see whether a `-` in passthrough
begins the exit signal. Neither holds state across a segment boundary except
the block accumulator, which Section 6.4 flushes explicitly.

---

## 16. Conformance testing

### 16.1 Structural

* The alphabet has 91 distinct characters, none of them `"`, `\`, `'` or below
  `0x20`, and `-` is at value 90.
* `R_CHARS` has eight distinct entries; the first seven are not in the
  alphabet, and the eighth is `-`.
* Each profile has eight distinct alphabet characters and does not contain `-`.
* `8280` is not in the range of the block coder, for any input.

### 16.2 Round trip

* Random binary at every length 0–300, plus 1 023, 1 024, 1 025, 65 535,
  65 536, 65 537.
* Text with every one of the 256 masks over the R-Set.
* `-`, `--`, `---` and longer runs, at the start, in the middle and at the end
  of the input, and immediately before and after a segment boundary; and a
  segment in which no profile can lend a donor for `-`, so that the scan has to
  stop at one.
* Mixed text and binary, exercising every block↔passthrough transition, with
  the pending bit count `n` taking each of its 14 values at a transition.

### 16.3 Canonicity

* No active donor occurs as a literal inside an emitted segment.
* The emitted `profile` is the smallest viable one for the accepted prefix.
* `mask = 0` is emitted only with `profile = 0`.
* `mask` has a set bit for exactly the R-Set characters in the segment.
* No emitted segment contains `-` at all.
* The pending bits take 0, 1 or 2 characters exactly as Section 6.5 requires.

### 16.4 Adversarial decode

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

### 16.5 The marker rule

An implementation MUST verify, over a large sample of inputs, that no
headerless stream it produces begins with a pair in 8 192 ... 8 279. This is
the property the whole self-description rests on; a counterexample would mean
an encoder somewhere needs an escape clause.

### 16.6 The damage bound

With a stream of at least six segments and bursts of mangled characters wide
enough to overwhelm a codeword, the payload damage MUST NOT exceed two
segments, and no run may return altered bytes without reporting a damaged
segment.

---

## 17. Security considerations

base91-jdp is an encoding, not a cryptographic transform. The decoder is the
security-relevant surface.

* **A framed stream carries a compressor, and a compressor expands.** A
  headerless stream cannot: passthrough is one character per byte and a symbol
  yields at most two bytes, so its output is bounded by its input. A framed one
  has no such bound -- expansion is what compression is -- and a decoder MUST
  therefore bound what it is willing to allocate. `SEGMENT_BYTES` is the bound
  per segment, and the number of segments is bounded by the input, so a
  conforming decoder can compute a ceiling before it starts.
* **The check pattern is not a MAC.** It detects accident. Anyone who can
  rewrite the stream can rewrite the pattern with it, since it is a plain
  function of the data. Authentication belongs outside this format.
* **Error correction can be made to work against you.** Reed-Solomon that is
  overwhelmed sometimes lands on a different valid codeword, which is the hole
  the check pattern narrows and does not close. A stream that must not be
  silently altered needs a real checksum or signature outside it.
* **Lengths are not attacker-controlled in a headerless stream**, because no
  length is carried. In a framed one the single `pad` byte per segment is, and
  a decoder MUST range-check it rather than trusting it.
* **Whitespace skipping must be incremental**, or a padded stream costs
  quadratic time.
* **Output is arbitrary binary.** Callers MUST NOT assume it is printable,
  NUL-terminated or text, whatever the input looked like.
* **A decoder MUST NOT return bytes from a damaged segment** as though they
  were sound. Reporting the loss is the contract.

---

## 18. Measurements

Full method, per-file numbers and every sweep: `bench/results/RESULTS.md`.

### 18.1 Corpus

The benchmark corpus is Base85N's, unchanged: 6.52 MB across 13 real files,
fetched from pinned upstream archives by `bench/corpus.py` — three binary
container formats, an uncompressed source tar, a JSON dataset in both
pretty-printed and minified form, JavaScript, CSS and Python source, the
CommonMark specification, a Markdown changelog, a JPEG and a PNG. Using it
unchanged is what makes the comparison a comparison.

The donor profiles are derived on a separate 2.37 MB training corpus
(`tools/traincorpus.py`) that shares no file and no upstream project with it.

### 18.2 `MIN_BINARY_RUN`

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

### 18.3 What the fixed symbol costs

basE91's adaptive coder takes fourteen bits when the low thirteen are small, so
its density depends on the data. Against the fixed thirteen-bit symbol, per
file:

| file | adaptive | fixed | cost |
|---|---|---|---|
| bootstrap.css, countries.json, lodash.js and the other text | -- | -- | 0.000 % |
| minduka_present.png | 1.22869 | 1.23082 | 0.173 % |
| grace_hopper.jpg | 1.22879 | 1.23055 | 0.143 % |
| sql-wasm.wasm | 1.20838 | 1.22777 | 1.605 % |
| requests-2.32.3.tar | 1.04385 | 1.06491 | 2.018 % |
| _cffi_backend.so | 1.18713 | 1.22765 | 3.413 % |
| **whole corpus** | 1.08442 | 1.09650 | **1.114 %** |

An earlier estimate of 0.08 % was taken from deflated input and does not hold
for raw binary. The fourteen-bit branch fires when a symbol's low thirteen bits
are at most 88, which zero-heavy binaries hit constantly: `_cffi_backend.so`
was averaging 13.478 bits per pair, a 47.8 % branch rate against the 1.086 % of
uniform data.

Structured binary is exactly what the compressed mode exists for, so the cost
appears in a headerless stream and almost never in a framed one.

### 18.4 Overall

Characters per input byte once the output sits in a JSON string, where `"` and
`\` have to be escaped. `jdp, no LZ4` is the headerless codec on its own;
`base91-jdp` is `encode` with its defaults, choosing per file.

| | Base64 | Ascii85 | basE91 | Base85N | Base64 +deflate | Base85N +deflate | jdp, no LZ4 | base91-jdp |
|---|---|---|---|---|---|---|---|---|
| text | 1.33333 | 1.26249 | 1.25189 | 0.96474 | 0.19672 | **0.18442** | 1.00081 | 0.33388 |
| binary | 1.33334 | 1.16265 | 1.22770 | 1.05041 | 0.53412 | **0.50073** | 1.19487 | 0.67613 |
| whole corpus | 1.33333 | 1.21326 | 1.23996 | 1.00698 | 0.36308 | **0.34039** | 1.09650 | 0.50264 |

Three readings, and they say different things.

**Against the plain binary-to-text codecs**, base91-jdp is twice as good: 0.503
against Base85N's 1.007 and Base64's 1.333.

**Against deflate-then-encode**, which is what people actually do when size
matters, it is 48 % worse than Base85N+deflate. That is the cost of LZ4 over
deflate, and it is the price of a format an implementer can write in an
afternoon (Section 19).

**On data that will not compress**, it is the only column that does not lose:

| sample | Base64 +deflate | Base85N | base91-jdp |
|---|---|---|---|
| `grace_hopper.jpg` | 1.330 | 1.249 | **1.231** |
| `minduka_present.png` | 1.334 | 1.250 | **1.231** |

A deflate pipeline expands an already-compressed file past plain Base64.
base91-jdp measures both candidates and keeps the headerless one, so it never
does.

### 18.5 Where the marker starts paying for itself

Two characters is not free on a small payload, and the format sets no threshold
for it: `encode` builds the framed candidate, computes its exact size, and
compares. The crossover is therefore a measurement of the data, not a constant.

| payload | repetitive text | JSON | source | random bytes |
|---|---|---|---|---|
| 32 B | headerless | headerless | headerless | headerless |
| 64 B | headerless | framed | framed | headerless |
| 128 B and up | framed | framed | framed | headerless |

Random bytes stay headerless at every size, which is the right answer: nothing
compresses them, and the marker would be pure loss.

### 18.6 The damage bound

4 MiB of payload in sixteen segments, protected, with bursts of mangled
characters placed at random. 200 trials per width:

| burst | worst damage | in segments | silently wrong |
|---|---|---|---|
| 4 characters | 262 144 B | 1.00 | 0 |
| 64 | 524 288 B | 2.00 | 0 |
| 256 | 262 144 B | 1.00 | 0 |
| 1 024 | 524 288 B | 2.00 | 0 |
| 4 096 | 524 288 B | 2.00 | 0 |

Two segments is the bound of Section 10.2 and it is reached, never exceeded.
No run in 1 200 returned altered bytes without reporting a damaged segment.

Scattered damage is a different measurement, and the answer there is that there
is no damage at all: over 3 MiB with one to eight flipped bits, every trial was
repaired exactly, in 240 runs.

An earlier arrangement failed this test at 256-character bursts with eleven
segments of sixteen lost, because the check pattern mixed in a stream-wide
codeword counter. Section 11.2 says why the counter is per segment.

### 18.7 The side-channel window

Forty distributions -- raw and LZ4-compressed text, source, JSON, CSV, XML,
images, uniform bytes and zeros -- against four ways of choosing the
eighty-eight values:

| window | worst case | mean |
|---|---|---|
| the top 88 values | 0.000 % | 0.5 % |
| the bottom 88 | 0.000 % | 11 % |
| every 91st value | 0.55 % | 4.4 % |
| `v x 8179 mod 8192` | **0.52 %** | 4.4 % |

The bottom window carries the most where it works -- LZ4 writes two-byte
offsets whose high byte is zero whenever a match is near, and those make small
symbols -- and nothing at all on repeated raw text. The choice went on the
worst case, since a check that can vanish is not a check.

Every scattered window performs within noise of every other, so the multiplier
was chosen on synthetic shapes and then checked against the corpus, which the
search never saw. On the corpus the channel carries 2.3 % to 4.3 % of symbols,
which is 793 to 3 460 check bits per segment.

### 18.8 Throughput

`countries.json`, 1 408 911 bytes, per layer:

| layer | encode | decode |
|---|---|---|
| LZ4 alone | 237 MB/s | 249 MB/s |
| bytes to symbols | 719 MB/s | -- |
| frame, check only | 453 MB/s | 125 MB/s |
| frame with Reed-Solomon | 185 MB/s | 108 MB/s |
| symbols to characters | 332 MB/s | 387 MB/s |
| whole pipeline | 90 MB/s | 76 MB/s |
| whole pipeline, protected | 85 MB/s | 42 MB/s |

The encoder does not build the headerless candidate when the framed one is
already shorter than the input, which a headerless stream can never be. Before
that shortcut the passthrough scan was 84 % of encoding time on binary input.

### 18.9 The donor profiles

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

### 18.10 `-` in the R-Set

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

### 18.11 The signal character

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

### 18.12 What is not measured, and what is left on the table

* **The corpus is 13 files.** Every constant in Section 6.9 sits on a plateau
  rather than at a fitted optimum, for that reason.
* **How much a stronger compressor would be worth is measured but not
  addressed.** Base85N over deflated bytes reaches 0.340 where this format
  reaches 0.503 (Section 18.4). Nothing here closes that; the escape marker is
  what would make it possible to.
* **The side channel is measured on thirteen files and a handful of synthetic
  shapes.** A distribution that fills none of the window is possible, and the
  format still decodes -- it simply has no check pattern, silently.
* **UTF-8 above U+007F breaks passthrough.** A multi-byte character is not
  representable, so prose in a language that uses accents runs through block
  mode. `commonmark-spec.txt` and `requests-history.md` both pay for this.

---

## 19. What was considered and left out (informative)

**A run-length or fill mode.** Earlier drafts held header space for one. It was
measured against compression and lost: a construct that repeats a byte is a
special case of what a compressor does anyway, and LZ4 turns a megabyte of
zeros into just over four thousand bytes without any help from the format.
The space it would have occupied is now the eighty-three unassigned markers.

**Deflate.** It compresses better than LZ4 by a wide margin -- Section 18.1
puts the corpus at 0.340 against 0.503 -- and it was rejected on the grounds
that a specification which demands deflate demands a library. That is a
judgement about who can implement this format, not about compression, and the
escape marker of Section 9.2 is what keeps the decision reversible.

**The adaptive thirteen-or-fourteen-bit coder.** basE91's own block coder is
denser than the fixed one by 0.08 % on high-entropy data and by up to 3.4 % on
structured binary. It reaches every one of the 8 281 pair values, which would
leave the format nothing to describe itself with, no symbol layer for error
correction, and no bound on what one damaged character reaches. It is kept in
the reference implementation as a benchmark and is not part of the format.

**Reed-Solomon parity in the side channel.** The capacity is nearly enough. The
positions are not: they come from symbol values one would need the parity to
correct.

---

## 20. References

* Joachim Henke, *basE91*, 2005. <http://base91.sourceforge.net/>
* Keywan Ghadami, *Base85N v0.5.0*, 2026. <https://base85n.ghadami.de/> — the
  passthrough design, the R-Set and donor-profile mechanism, and the benchmark
  corpus are taken from it.
* RFC 8259, *The JavaScript Object Notation (JSON) Data Interchange Format*.
