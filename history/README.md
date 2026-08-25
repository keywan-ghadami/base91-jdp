# Superseded work

What is kept here implemented or measured a version of the format that no
longer is the format. It is not maintained, it is not built by CI, and nothing
in the current version depends on it. It is kept because a decision is easier
to trust when the thing it was made from is still readable.

The current specification is [`spec/`](../spec/README.md) and the current
implementation is [`rust/`](../rust/README.md).

## [`javascript-v0.3.0/`](javascript-v0.3.0/README.md)

The complete v0.3.0 implementation in JavaScript — the codec, LZ4, the
Reed-Solomon layer, the CLI, seventy-six tests, and the benchmarks and reports
that produced every number 0.3.0 published. It is the package that was on npm
as `base91-jdp@0.3.0`.

Its wire format is not 0.4.0's and the two do not interoperate: 0.3.0 puts a
mode marker at the head of a stream and 0.4.0 has typed segments, 0.3.0 carries
LZ4 and 0.4.0 carries zstd, and 0.3.0's Reed-Solomon layer has no successor.

## [`projections/`](projections/README.md)

Two benchmarks that estimated what 0.4.0 would cost before there was anything
to run. They were right within a few parts in ten thousand on compression and
wrong by a factor of ten on the chained-gap classes, which is roughly what a
projection is worth and why the specification now says so in its Section 18.7.
