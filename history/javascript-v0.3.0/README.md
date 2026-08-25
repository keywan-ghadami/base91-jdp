# base91-jdp v0.3.0, in JavaScript

The implementation of [specification
v0.3.0](../../spec/history/base91-jdp-v0.3.0.md), kept as it was published.
Zero dependencies, ESM, Node 18.11 or newer.

```sh
cd history/javascript-v0.3.0
npm test          # 76 tests: round trip, adversarial decode, damage bound
node bench/bench.js
```

The benchmarks read the corpus from `bench/corpus/` at the repository root,
which `python3 bench/corpus.py` fetches; that corpus is shared with the current
version and did not move.

## What it is, and what it is not

It is a complete and tested codec: basE91 on the JSON-safe alphabet, dynamic
passthrough, LZ4 inside a framed body, and Reed-Solomon over GF(2¹³) at 0.098 %
that repairs two damaged symbols per codeword.

It does not implement the current format. 0.4.0 replaced the head-of-stream
mode markers with typed segments, LZ4 with zstd, and the error-correction layer
with nothing at all — the reasoning is in Sections 18.1, 18.4 and 18.5 of the
current specification. A 0.3.0 stream and a 0.4.0 stream are not
interchangeable and neither decoder reads the other's output.

`results/RESULTS.md` and `results/RS.md` are its measurements. They are correct
about 0.3.0 and say nothing about 0.4.0, whose numbers are in Section 17 of the
current specification.
