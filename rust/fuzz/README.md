# Fuzzing

Five [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) targets.
The decoder reads streams it did not write -- specification Section 15.4 -- and
the crate reaches those streams through thirteen `unsafe` blocks, so this is
where the two facts are put together.

```sh
cargo install cargo-fuzz            # needs a nightly toolchain
cd rust
cargo +nightly fuzz list
cargo +nightly fuzz run decode_structured -- -max_total_time=300
```

`cargo fuzz` builds with AddressSanitizer by default, so a target that finds an
out-of-bounds read reports it as one rather than as whatever the read happened
to return.

## The targets

| | what it asserts |
|---|---|
| `roundtrip` | Anything encoded decodes back to itself, through an alphabet with nothing a JSON string escapes, and never longer than block mode alone (Section 11.2). Runs the default entry point, the plain one, and two compression levels. |
| `decode_any` | Arbitrary bytes into the decoder: bytes out or an error, never a panic. A bounded decode never returns more than the budget. |
| `decode_alphabet` | The same, with every input byte folded onto the alphabet first, so the run is spent past the character check rather than on it. |
| `decode_structured` | Streams built field by field -- a class the fuzzer picks, a length from a table of nothing but boundaries, a run value, a parameters pair. This is the one aimed at the decoder's arithmetic. |
| `parallel` | The parallel encoder is the serial one character for character, with the chunk boundary anywhere the fuzzer likes, and the result still decodes. |

## Why `decode_structured` exists

A signal is a pair value of 8 192 or above, which is 88 of the 8 281 a pair can
hold. A fuzzer mutating alphabet characters reaches one about once in a hundred
pairs and reaches a *specific class with a specific length* far less often than
that, so a blind target spends most of its budget decoding well-formed block
data. `decode_structured` writes the fields directly: the class, the length,
the run value. Lengths come from a table of boundaries -- 0, 89, 90, 8 369,
8 370, each class bound and one past it, and the largest value the three-tier
field can express, about 68.5 million against a class bound of 65 536 -- because
no amount of mutating a four-byte integer finds `MAX_SEGMENT_BYTES + 1`, and
choosing an index into a table does.

The emitter is [`../tests/support/stream.rs`](../tests/support/stream.rs),
shared with `../tests/adversarial.rs`. One emitter on purpose: a case the
fuzzer finds is written down as a named test in the same vocabulary, without
translating it first.

## What the targets do not cover

Neither `decode_alphabet` nor `decode_structured` asserts anything about the
*result* of a decode, because an arbitrary stream is not one anybody encoded --
bytes and an error are equally correct answers, and asserting either would be
asserting a thing that is not true. What they assert is that one of the two
happens, and that a bounded decode stays inside its bound. `roundtrip` and
`parallel` are where output is checked against a known answer, and they only
ever see streams the encoder wrote.

`corpus/` and `artifacts/` are git-ignored. A crash worth keeping becomes a
case in `../tests/adversarial.rs`, which CI runs on every push; a corpus is not
a test.
