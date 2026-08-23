#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Regenerate test/lz4-fixtures.js from the reference LZ4 implementation.

    pip install lz4 && python3 tools/lz4fixtures.py > test/lz4-fixtures.js

The fixture exists so that src/lz4.js is checked against the block format and
not merely against itself. Inputs are formulas, not data: what matters is the
blocks, which upstream liblz4 produced and which our decompressor must read.
"""

import base64

import lz4.block

CASES = [
    ("empty", "zeros(0)", b""),
    ("one byte", "run(1, 65)", b"A"),
    ("shorter than the minimum match", "count(11)", bytes(range(11))),
    ("exactly one group", "count(13)", bytes(range(13))),
    ("a literal run of fifteen", "count(15)", bytes(range(15))),
    (
        "a literal run needing a continuation byte",
        "count(300)",
        bytes(i & 0xFF for i in range(300)),
    ),
    ("five thousand zeros", "zeros(5000)", b"\x00" * 5000),
    ("a three byte period", "period(2100, 3)", bytes(97 + (i % 3) for i in range(2100))),
    (
        "a match at the far edge of the offset field",
        "edge()",
        bytes(
            (97 + (i % 7)) if i < 32 or i >= 65535 else ((i * 37 + 11) & 0xFF)
            for i in range(65567)
        ),
    ),
    ("text", "text(40)", b"the quick brown fox jumps over the lazy dog. " * 40),
]

HEADER = """// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Blocks produced by the reference LZ4 implementation (python-lz4 4.x over
// upstream liblz4, high compression), so that src/lz4.js is checked against
// the format itself and not only against its own output. The inputs are
// formulas rather than data: the point of the fixture is the blocks.
//
// Regenerate with tools/lz4fixtures.py if the set ever needs to grow.

const enc = new TextEncoder();
const count = (n) => Uint8Array.from({ length: n }, (_, i) => i & 0xff);
const zeros = (n) => new Uint8Array(n);
const run = (n, b) => new Uint8Array(n).fill(b);
const period = (n, p) => Uint8Array.from({ length: n }, (_, i) => 97 + (i % p));
const text = (n) => enc.encode('the quick brown fox jumps over the lazy dog. '.repeat(n));
// A match 65535 bytes back is the furthest the two-byte offset field reaches.
const edge = () =>
  Uint8Array.from({ length: 65567 }, (_, i) =>
    i < 32 || i >= 65535 ? 97 + (i % 7) : (i * 37 + 11) & 0xff,
  );

export const REFERENCE_BLOCKS = ["""


def main():
    print(HEADER)
    for name, formula, src in CASES:
        block = lz4.block.compress(
            src, store_size=False, mode="high_compression", compression=12
        )
        assert lz4.block.decompress(block, uncompressed_size=max(len(src), 1)) == src
        b64 = base64.b64encode(block).decode()
        print("  {")
        print("    name: %r," % name)
        print("    plain: () => %s," % formula)
        if len(b64) <= 72:
            print("    block: '%s'," % b64)
        else:
            chunks = [b64[i : i + 68] for i in range(0, len(b64), 68)]
            print("    block:")
            print("      " + "\n      ".join("'%s' +" % c for c in chunks[:-1]))
            print("      '%s'," % chunks[-1])
        print("  },")
    print("];")


if __name__ == "__main__":
    main()
