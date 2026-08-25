# Projections that a prototype replaced

Two benchmarks, written while v0.4.0 was a document and not yet an
implementation. Both compute what the format would cost from its own
arithmetic over the real corpus, with the v0.3.0 encoder standing in for the
parts that did not exist.

- `zstdprojection.js` — what a zstd segment would cost, from the frame lengths
  and the packing rule. It projected 0.34436 for the core corpus at level 3;
  the implementation encodes it to 0.34445.
- `uncompressed.js` — what the passthrough, packed and run classes would cost.
  It projected 1.00464 for the core corpus; the first run of the real encoder
  produced 1.03809, because the projection did not know that a greedy prefix
  scan swallows the runs the run classes exist for.

Kept because the difference between the two is the argument for building
prototypes: one projection was right to four decimal places and the other was
wrong by enough to hide a design fault, and nothing about the two said in
advance which would be which.
