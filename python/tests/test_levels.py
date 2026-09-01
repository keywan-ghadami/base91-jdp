# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""The level is part of the encoding and not of the payload, and the README
and the docstrings both say so. This is what holds them to it."""

import pytest

import base91z

PAYLOAD = (b'{"event":"login","user":"ada","ok":true}' * 400)

# Something with more than one kind of structure in it, so the levels have
# something to disagree about. On a payload as repetitive as PAYLOAD alone,
# levels 1, 9 and 19 all find the same thing and produce the same string --
# which is why the claim below is "can differ", not "always differ".
MIXED = PAYLOAD + bytes(range(256)) * 40 + b"x" * 5000


@pytest.mark.skipif(not base91z.HAS_COMPRESSOR, reason="built without zstd")
def test_a_level_can_change_the_text_without_changing_the_bytes():
    texts = {level: base91z.encode(MIXED, level=level) for level in (-5, 1, 9, 19)}
    # The point of the caveat: the text is not a function of the payload alone.
    # Two levels agreeing is fine and common -- all four agreeing would mean
    # the caveat had nothing behind it.
    assert len(set(texts.values())) > 1, "every level produced the same text"
    for text in texts.values():
        assert base91z.decode(text) == MIXED


def test_the_same_call_twice_is_the_same_string():
    # Determinism within a level, which is the half that does hold.
    assert base91z.encode(PAYLOAD) == base91z.encode(PAYLOAD)
    assert base91z.encode_plain(PAYLOAD) == base91z.encode_plain(PAYLOAD)


@pytest.mark.skipif(not base91z.HAS_COMPRESSOR, reason="built without zstd")
def test_the_default_level_is_the_documented_one():
    assert base91z.encode(PAYLOAD) == base91z.encode(PAYLOAD, level=base91z.DEFAULT_LEVEL)


def test_compression_actually_happens():
    if base91z.HAS_COMPRESSOR:
        assert len(base91z.encode(PAYLOAD)) < len(base91z.encode_plain(PAYLOAD))
    else:
        assert len(base91z.encode(PAYLOAD)) == len(base91z.encode_plain(PAYLOAD))
