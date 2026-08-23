# Does Reed-Solomon pay for itself here, and where is the sweet spot?

Reproduce with `node bench/rsstudy.js` (each section runs on its own:
`m1` … `m6`). The Base85N sections need Go on the path.

## The question

The planned rebuild — passthrough out, deflate always on, Reed-Solomon, a small
header — rests on two requirements that pull against each other:

1. **Containment, not primarily repair.** 2 GB with one flipped bit: at best
   nothing is lost, at worst one megabyte. No checksum, one fixed code strength.
2. **Beat Base85N on ratio**, with a risk you can put a number on.

Measured over the benchmark corpus, both sides deflated at level 6:

| | characters per input byte |
|---|---|
| Base85N | 0.34039 |
| base91-jd, byte-synchronous packer, no protection | **0.33515** — 1.54 % ahead |

Every layer of protection is paid for out of that 1.54 %.

> **The reference has to be measured on the same bytes.** An earlier comparison
> put Base85N at 0.33741, which was its output on a corpus deflated at level 9
> while ours was deflated at level 6. That flattered the opponent by 0.9 % —
> enough to reverse which pipelines look like wins. Deflated data is
> incompressible, so Base85N spends a flat 1.25 characters per byte on it
> against the byte-synchronous packer's 1.23077.

## Two decisions, not one

They are independent, combinable, and priced completely differently.

| | without Reed-Solomon | with it |
|---|---|---|
| **without byte-sync** | 5.9 % of flips destroy everything after them | the code cannot help: a desync moves its own codeword boundaries |
| **with byte-sync** | damage ≤ 1 deflate segment, guaranteed | damage 0 for isolated flips, ≤ 1 segment otherwise |

---

## M1 — what one flipped character costs each packer

| packer | chars per byte | wrong bytes: median / p95 / max | reach: median / max | total losses | decoder threw |
|---|---|---|---|---|---|
| A  adaptive (basE91 as today) | 1.22982 | 2 / 120833 / 398033 | 2 / 399597 | 5.9 % | 0.0 % |
| B  adaptive, realigned every 64 kB | 1.22984 | 2 / 19726 / 64930 | 2 / 65166 | 0.0 % | 0.0 % |
| C  byte-synchronous, 13 bytes to 16 characters | 1.23077 | 2 / 3 / 3 | 2 / 3 | 0.0 % | 0.0 % |

3000 trials each, one flipped bit per trial, on 400000 bytes of deflated corpus.
"total losses" counts flips that damaged more than 65536 bytes — the containment bound the format is supposed to promise.

Of 20000 single-bit flips, 13.8 % produced a character outside the alphabet and a further 0.5 % a pair value above 8191. Both localise the damaged symbol, which is what erasure decoding needs.

**Reading it.** The adaptive coder that basE91 uses today has no upper bound:
in 5.9 % of single-bit flips a whole 400 kB body is lost, because the flip
crossed the value threshold that decides whether a pair carries 13 bits or 14,
and every bit after it shifted. Realigning at segment boundaries (B) caps that
at the segment, for free. Going byte-synchronous (C) caps it at **three bytes**,
for 0.080 %.

That 0.080 % is the whole price of the first decision, and it is also what makes
the second one possible: only C has a symbol layer a code can sit on, because
only in C is one character pair one independent unit.

---

## M2 — which field, which codeword length, which strength

| code | nsym | overhead | chars per byte | repaired: 1 flip | 2 flips | 4 flips | 16 flips |
|---|---|---|---|---|---|---|---|
| GF(2^8)  n=255 | 2 | 0.8 % | 1.24050 | 31.0 % | 12.5 % | 1.3 % | 0.0 % |
| GF(2^8)  n=255 | 4 | 1.6 % | 1.25040 | 90.8 % | 82.0 % | 66.5 % | 21.3 % |
| GF(2^8)  n=255 | 6 | 2.4 % | 1.26043 | 100.0 % | 100.0 % | 99.8 % | 95.5 % |
| GF(2^8)  n=255 | 8 | 3.2 % | 1.27065 | 100.0 % | 100.0 % | 100.0 % | 99.3 % |
| GF(2^13) n=255 | 2 | 0.8 % | 1.24051 | 100.0 % | 100.0 % | 98.8 % | 85.8 % |
| GF(2^13) n=255 | 4 | 1.6 % | 1.25040 | 100.0 % | 100.0 % | 100.0 % | 100.0 % |
| GF(2^13) n=1024 | 2 | 0.2 % | 1.23319 | 100.0 % | 99.5 % | 95.3 % | 52.3 % |
| GF(2^13) n=1024 | 4 | 0.4 % | 1.23560 | 100.0 % | 100.0 % | 99.8 % | 99.0 % |
| GF(2^13) n=4096 | 2 | 0.1 % | 1.23139 | 100.0 % | 97.0 % | 87.0 % | 5.0 % |
| GF(2^13) n=4096 | 4 | 0.1 % | 1.23200 | 100.0 % | 100.0 % | 99.3 % | 81.0 % |
| GF(2^13) n=8191 | 2 | 0.0 % | 1.23108 | 100.0 % | 95.3 % | 72.8 % | 0.0 % |
| GF(2^13) n=8191 | 4 | 0.1 % | 1.23139 | 100.0 % | 100.0 % | 99.3 % | 41.3 % |

400 trials per cell, on 300000 bytes of deflated corpus (293 kB). "repaired" means the payload came back byte-identical.
Overhead is measured against the same bytes packed without any parity.

**Reading it.** The field choice decides everything, and the reason is a
mismatch of units. The channel damages **characters**; a 13-bit pair straddles
two or three **byte** boundaries. So byte-level parity has to be strong enough
for three byte errors before it can repair one character error — `nsym = 6`,
confirmed exactly: 31 %, then 90.8 %, then 100 %.

Over character pairs, one damaged character is one damaged symbol, and `nsym = 2`
repairs 100 % of single flips. Then the second lever: the overhead is `nsym / n`,
so the longer the codeword the cheaper the same protection. At the field maximum
of 8191 symbols — 13.3 kB of payload — two parity symbols cost **0.02 %**.

The last two columns are the caveat. Long codewords are cheaper per byte and
*worse* under many errors, because more of them land in the same codeword.
Sixteen flips in a 293 kB body is a far harsher channel than the one this format
is being designed for, but it is what separates the candidates.

---

## M3 — the ratio line against Base85N

| pipeline | chars per input byte | vs Base85N | repairs |
|---|---|---|---|
| Base85N + deflate (the opponent) | 0.34039 | 0.00 % | no |
| gzip -6 + adaptive | **0.33492** | -1.61 % | no |
| raw deflate + adaptive (packer A) | **0.33488** | -1.62 % | no |
| raw deflate + byte-synchronous (packer C) | **0.33515** | -1.54 % | no |
| raw deflate + C + RS GF(2^8) n=255 nsym=2 | **0.33780** | -0.76 % | yes |
| raw deflate + C + RS GF(2^8) n=255 nsym=4 | 0.34050 | +0.03 % | yes |
| raw deflate + C + RS GF(2^8) n=255 nsym=6 | 0.34323 | +0.84 % | yes |
| raw deflate + C + RS GF(2^13) n=255 nsym=2 | **0.33781** | -0.76 % | yes |
| raw deflate + C + RS GF(2^13) n=255 nsym=4 | 0.34050 | +0.03 % | yes |
| raw deflate + C + RS GF(2^13) n=1024 nsym=2 | **0.33581** | -1.34 % | yes |
| raw deflate + C + RS GF(2^13) n=1024 nsym=4 | **0.33647** | -1.15 % | yes |
| raw deflate + C + RS GF(2^13) n=4096 nsym=2 | **0.33532** | -1.49 % | yes |
| raw deflate + C + RS GF(2^13) n=4096 nsym=4 | **0.33549** | -1.44 % | yes |
| raw deflate + C + RS GF(2^13) n=8191 nsym=2 | **0.33524** | -1.51 % | yes |
| raw deflate + C + RS GF(2^13) n=8191 nsym=4 | **0.33532** | -1.49 % | yes |

Bold marks every pipeline that stays under the opponent. Negative is a win.
Each file is deflated on its own, so the per-file codeword count -- and with it
the parity cost -- is what a real payload of that size would pay.

**Reading it.** Byte-level Reed-Solomon at the strength it actually needs
(`nsym = 6`) lands at 0.34323 and **loses the race**. Symbol-level parity at a
long codeword costs almost nothing: `n = 8191, nsym = 2` keeps 1.51 of the
1.54 points, and `n = 4096, nsym = 4` — twice the correction strength — still
keeps 1.44.

---

## M4 — what a dictionary reset costs

| segment | compressed bytes | vs one stream | segments |
|---|---|---|---|
| 64 kB | 2,466,173 | 3.480 % | 136 |
| 256 kB | 2,405,957 | 0.954 % | 34 |
| 1024 kB | 2,387,191 | 0.166 % | 9 |
| 4096 kB | 2,384,147 | 0.039 % | 3 |
| one stream, no reset | 2,383,232 | 0.000 % | 1 |

8,885,182 bytes of benchmark plus training corpus, all distinct files.
The segment size is what sets the damage bound: a codeword the code cannot repair
ruins the rest of its segment and nothing beyond it.

**Reading it.** This is the expensive layer, and it is the one that actually
sets the promised bound. One megabyte per segment costs 0.166 % — twice what the
packer costs and more than the parity. Four megabytes would cost 0.039 %, and
64 kB would cost 3.5 % and lose the race on its own.

---

## M5 — the promise, end to end

8.5 MiB payload, 2.3 MiB after deflate in 9 segments of 1024 kB, RS GF(2^13) n=4096 nsym=4, 2,940,962 characters.

| flipped bits | payload intact | wrong bytes: median / p95 / max | over one segment |
|---|---|---|---|
| 1 | 100.0 % | 0 / 0 / 0 | 0.0 % |
| 2 | 100.0 % | 0 / 0 / 0 | 0.0 % |
| 4 | 100.0 % | 0 / 0 / 0 | 0.0 % |
| 16 | 100.0 % | 0 / 0 / 0 | 0.0 % |

The promise under test: one flipped bit costs nothing, and no number of them costs
more than one segment. The last column is the one that can fail it.

### The one place the bound can break

Aiming 40 flips at the segment length fields: wrong bytes median 0, p95 0, max 0.
Length fields are 36 bytes out of 2,387,191, so a random flip finds one about once in 66,311.
Zero damage because the length field sits inside the code's protection: the
flip is repaired before the framing layer ever sees it.

### When the code is overwhelmed

| mangled characters | codeword survives | wrong bytes: median / p95 / max | over one segment |
|---|---|---|---|
| 4 | 45.0 % | 14312 / 839104 / 927553 | 0.0 % |
| 8 | 17.5 % | 456925 / 910982 / 1048576 | 0.0 % |
| 32 | 5.0 % | 440729 / 874106 / 1047667 | 0.0 % |
| 256 | 10.0 % | 298551 / 911436 / 916137 | 0.0 % |

A run of 8 mangled characters is already more than a codeword of nsym=4 can
carry. What matters from there on is not whether the payload survives -- it does not --
but whether the loss stays inside one segment.

**Reading it.** The first table is the code doing its job: every flip repaired,
nothing lost, including flips aimed deliberately at the segment length fields —
those sit inside the protection, so the framing layer never sees the damage.

The second table is the one that tests the promise, because a bound only means
something once the code has given up. From four mangled characters upward the
payload is gone, and the maximum damage across every burst size is
**1,048,576 bytes — exactly one segment, never more.**

---

## M6 — Base85N under the same bit flips

1,775,377 bytes of deflated corpus (6,519,688 bytes of input), 3000 single-bit flips each.

| | Base85N | base91-jd, packer C alone |
|---|---|---|
| characters | 2,219,208 | 2,185,080 |
| characters per input byte | 0.34039 | 0.33515 |
| decoder refused | 16.6 % | 0.0 % |
| wrong bytes when it did not: median / p95 | 3 / 4 | 2 / 3 |
| worst case | 1,707,683 | 3 |
| silently wrong by over 1 MB | 0.6 % (19 of 3000) | 0.0 % |
| output longer than the input | 49 | 0 |

Base85N's block mode is byte-synchronous too -- five characters carry exactly
four bytes -- which is why its median damage is as small as ours. The tail is
the difference: its signals are not, and a damaged Fill signal invents bytes
that were never sent. The 19 runs in the last-but-one row
returned over a megabyte of wrong data without reporting anything.

**Reading it.** Base85N is better here than one might guess: its block mode is
byte-synchronous too, so its median damage is three bytes, as small as ours. Two
things separate them. It refuses one flip in six outright — an honest failure,
but the payload is gone either way. And in **0.6 % of single flips it returns
over a megabyte of wrong data and reports success**, because a damaged Fill
signal invents bytes that were never sent; 49 of 3000 runs produced output
*longer* than the input.

That is the difference the phrase "calculable risk" is about. Both formats have
a small median. Only one has an upper bound.

---

## Recommendation

Build all three layers. Together they cost **0.35 %** of a 1.54 % lead.

| layer | setting | cost | what it buys |
|---|---|---|---|
| packer | byte-synchronous, 13 bytes ↔ 16 characters | 0.080 % | damage per flipped character drops from unbounded to 3 bytes |
| parity | RS over GF(2¹³), `n = 4096`, `nsym = 4` | 0.101 % | 100 % repair at 1–2 flips, 99.3 % at 4 |
| segments | raw deflate level 6, reset every 1 MiB | 0.166 % | the hard bound, proven at exactly 1,048,576 bytes |

**Result: 0.33605 characters per input byte against Base85N's 0.34039 — 1.28 %
ahead, with single-error correction and a hard damage bound.** Both requirements
are met at once, which is not what the first sketch of this design suggested.

### Why `n = 4096, nsym = 4` and not the cheapest option

`n = 8191, nsym = 2` costs 0.02 % instead of 0.10 % and repairs every single
flip just as well — it is the right answer to the requirement exactly as
written. It is not the right answer to the requirement as it will be used: it
repairs 95.3 % of double flips against 100 %, and collapses to 0 % at sixteen
where the recommendation still manages 41 %. The extra 0.08 % buys a much
gentler slope, and there are 1.4 points of margin to spend it from.

### What is deliberately not in this

* **No checksum.** As specified. The cost is that Reed-Solomon beyond its
  capacity miscorrects silently in about 3 % of cases over GF(2¹³) — measured,
  and five times better than GF(2⁸)'s 17 %, because a larger field makes a
  random syndrome far less likely to look like a valid correction. A four-byte
  CRC32 would close it, still cost 14 bytes less than gzip's wrapper, and is the
  one thing worth revisiting.
* **No erasure decoding.** 13.8 % of flips leave the alphabet and a further
  0.5 % push a pair above 8191, so a decoder can point at the damaged symbol for
  free and a code correcting erasures gets twice the mileage from the same
  parity. At `nsym = 4` repairing everything the requirement asks for, it would
  be complexity without a job.

### The one loose end

Segment framing. Each segment carries a four-byte compressed length, and if the
codeword holding that length is beyond repair, the framing after it is gone and
the bound breaks. The measurement above shows zero damage from flips aimed at
those bytes — but only because the code repaired them first, which is not the
same as the bound holding without it. A segment table at the end, more heavily
protected, would cost 8 kB on a 2 GB stream and make the format seekable and
parallel-decodable as a side effect. That belongs in the specification work, not
in this study.
