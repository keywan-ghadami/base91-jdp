# Security

## Reporting

Mail **keywan.ghadami@gmail.com**, or open a GitHub security advisory on
[the repository](https://github.com/keywan-ghadami/base91z/security/advisories).
A public issue is fine too: this is a draft-stage prototype with no users to
protect, and a bug found in the open gets fixed faster than one found in
private. There is no bounty and no embargo policy to negotiate.

Nothing here is deployed anywhere, nothing is published to crates.io, and the
implementation is a prototype. What follows is what has been done to it, not a
claim that it is finished.

## What the threat model is

A decoder is the attack surface. **An encoder is handed bytes by its caller; a
decoder is handed a string by whoever sent it** -- a JSON field, a log line, a
database column -- and specification Section 15.4 allows it exactly two
outcomes on a stream it did not write: the bytes, or an error. Not a panic, not
a read past the end, not an allocation the caller did not sanction.

Three things in this format make that harder than it sounds, and all three are
attacker-controlled:

* **Length fields.** A segment declares how long it is, in a three-tier field
  whose widest form expresses about 68.5 million against a class bound of
  65 536. A decoder that reserves what a length claims is a denial of service
  in four characters.
* **Runs.** Class 18 and class 19 are a length and, at most, a byte: four
  characters produce 65 536. The expansion ratio is the point of the classes
  and the reason the output ceiling exists.
* **The compressed classes.** A zstd frame's expansion is not bounded by
  anything in the frame. Section 16 is the rule that what is *allocated* is
  bounded by what a payload of that size could physically produce, and not by
  what it says it produces.

The **encoder** is a smaller surface but not a zero one: it writes into buffers
it has sized itself, through bulk paths that carry no per-byte bounds check.
That is what Miri and the sanitizers below are pointed at.

## One property this format does not have

**The encoded text is not a stable identity for the payload.** The compression
level is a parameter, so the same bytes encode to different strings at
different levels — and the zstd build underneath, and the set of classes an
encoder implements, can move the text too. Specification Section 11.3 makes the
encoder's own choices canonical, which is a narrower promise than it sounds:
two conforming encoders at the same level with the same compressor agree
character for character, and nothing guarantees more than that.

So a protocol that **signs, hashes or compares the encoded string** — a
signature over the text, a content-addressed key, a dedup key, an ETag — is
building on something this format does not promise, and will break on a level
change, a dependency bump, or a second implementation. Sign the payload:
`decode(encode(x)) == x` is the guarantee, and the bytes either side of it are
where an identity belongs. Base64 and Base85N have no parameter and are stable
this way; where the encoded form itself must be reproducible, they are the
better choice and this is not a gap to work around.

## What is run against it

Everything here runs in CI on every push, except where it says otherwise.

| | what it catches |
|---|---|
| `cargo test`, with and without the `zstd` feature | Round trip over every class, the guarantees the specification states as guarantees, and `rust/tests/adversarial.rs` -- streams no encoder would write, asserted down to the error code. |
| `cargo test --features simd` on nightly | The vector paths answer the same questions as the scalar ones. The whole suite runs under both and must agree character for character. |
| `cargo clippy -- -D warnings`, stable and nightly | Including `clippy::undocumented_unsafe_blocks`, denied at the crate root: every `unsafe` block carries a `// SAFETY:` comment saying what makes it sound, and a new one without it does not compile. |
| [`cargo fuzz`](rust/fuzz/README.md), five targets | Round trip, the decoder on arbitrary bytes, on alphabet-folded bytes, on streams built field by field, and the parallel encoder against the serial one. Built with AddressSanitizer. CI runs a short smoke of each; long runs are manual. |
| `cargo miri test` | Undefined behaviour in the raw-pointer bulk paths -- the sixteen-byte store that advances thirteen, the `set_len` after it. Miri cannot cross an FFI boundary, so this is the container-only build, which is the build that contains all of the raw pointers. |
| `cargo audit` | RustSec advisories against the committed `Cargo.lock`. Yanked versions are a failure, not a warning. |
| `cargo deny check` | Licences, sources and bans. The allow-list is permissive plus this crate's own MPL-2.0; crates come from crates.io and nowhere else; a wildcard version is refused, because accepting whatever is published next is how dependency confusion works. |

## What the code does about it

* **One decoder ceiling, checked before anything is reserved.** `decode_bounded`
  takes a budget and nothing exceeds it: a run emits byte by byte against it, a
  passthrough segment is refused if it would cross it, and a compressed segment
  is refused on its *declared* plaintext length before the decompressor is
  handed the frame. `decode` uses a default ceiling rather than none.
* **A declared length is not evidence.** For the compressed classes the
  allocation is the smaller of what was declared, what a payload of that size
  could physically produce, and a fixed cap; the decompressor is then read one
  byte past what was declared, so a frame producing more is caught rather than
  silently truncated.
* **Every length field is bounds-checked against its class**, and a length of
  zero is refused: a class that produced nothing would be a loop that consumed
  nothing.
* **Thirteen `unsafe` blocks, each with a `SAFETY` comment**, and a lint that
  fails the build for a fourteenth without one. They are the two bulk paths --
  which establish their bounds once per group rather than once per byte, and
  say so -- and the three places an alphabet-only buffer becomes a `String`
  without a second validation pass.
* **No dependency in the container.** `zstd` is the only dependency, it is
  optional, and without it the crate has none; `rand` is for tests.

## What has not been done

* **No second reader.** The specification's Section 20 says which parts would
  most repay one. Nothing here has been reviewed by anyone who did not write it.
* **The fuzzing is short.** Millions of executions per target, not billions, and
  no continuous fuzzing service. The corpora are not kept.
* **Miri does not see the compressed classes**, because it cannot run zstd's C
  code. Those paths are covered by tests, fuzzing and ASan, not by Miri.
* **No formal verification of the arithmetic**, and no constant-time claim:
  this is a data format, not a cipher, and nothing here is written to be free
  of data-dependent timing.

## What it has found

Nothing in the decoder yet: the adversarial suite passed as written, and the
fuzz targets have run roughly ten million executions between them without a
crash in it.

One thing in the encoder's public surface. `encode_with_chunk` asserts that a
chunk is a whole number of symbol groups -- correctly; a chunk ending mid-group
could not be spliced, which is the basis of the parallel encoder -- and
declared that nowhere. The `parallel` fuzz target found it on the empty input
within four minutes of its first run. The assertion stayed, the documentation
changed, and `tests/adversarial.rs` holds both halves of the contract now: the
three inputs that must panic, and the aligned sizes that must agree with the
serial encoder.

The rest of what the exercise turned up was in the tooling rather than the
crate -- a fuzz target whose compressed class had compiled out unnoticed,
eight nightly-clippy findings on the paths nothing linted, and two site checks
that had gone dead. Each has a regression test.
