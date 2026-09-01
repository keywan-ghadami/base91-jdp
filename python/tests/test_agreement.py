# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""The bindings are the Rust library and not a second implementation.

There is no Python encoder to disagree with the Rust one, which is the whole
design -- so there is no cross-implementation test to write here, and nothing
below pretends otherwise. What is worth checking is that the module reports the
crate's own constants rather than a transcribed copy, and that the flags it
publishes match what it actually does.
"""

import base91z


def test_the_constants_come_from_the_crate():
    assert base91z.SPEC_VERSION == "0.4.0"
    assert len(base91z.ALPHABET) == 91
    assert len(set(base91z.ALPHABET)) == 91, "the alphabet has a repeated character"
    for forbidden in '"\\':
        assert forbidden not in base91z.ALPHABET
    assert base91z.MAX_SEGMENT_BYTES == 65536
    assert base91z.MAX_BLOCK_BYTES == 131072
    assert base91z.PARALLEL_ALIGN == 13


def test_the_version_matches_the_distribution():
    from importlib.metadata import version

    assert base91z.__version__ == version("base91z")


def test_the_compressor_flag_is_not_stuck_off():
    # `HAS_COMPRESSOR` is computed with `cfg!(feature = "zstd")` inside the
    # *binding* crate, which has that feature only because Cargo.toml forwards
    # it. Forget the forwarding and the flag reads false while everything still
    # builds -- so the flag is checked against behaviour, not trusted.
    payload = b"compress me " * 500
    compressed = len(base91z.encode(payload)) < len(base91z.encode_plain(payload))
    assert base91z.HAS_COMPRESSOR == compressed
