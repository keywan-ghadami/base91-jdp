# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Malformed input raises, with the specification's name for the condition and
the offset it was found at -- and the ceiling is reachable from Python."""

import pytest

import base91z

# Nine characters that declare a 65 536-byte run of zeroes: a class-18 signal
# and a tier-three length field. The shape a ceiling exists for.
BOMB = "m----Y]GA"


def test_a_character_outside_the_alphabet_is_refused():
    with pytest.raises(base91z.Base91zDecodeError) as excinfo:
        base91z.decode('"not in the alphabet')
    assert excinfo.value.code == "invalid_character"
    assert excinfo.value.position == 0


def test_the_exception_is_a_value_error():
    # So that `except ValueError` catches it, which is what a caller who has
    # not read this far will write.
    assert issubclass(base91z.Base91zDecodeError, ValueError)
    with pytest.raises(ValueError):
        base91z.decode('"')


def test_the_bomb_is_refused_under_a_budget_and_produced_without_one():
    with pytest.raises(base91z.Base91zDecodeError) as excinfo:
        base91z.decode(BOMB, max_bytes=1000)
    assert excinfo.value.code == "invalid_length"

    out = base91z.decode(BOMB, max_bytes=1 << 20)
    assert out == bytes(65536)


def test_a_budget_that_fits_exactly_is_allowed():
    text = base91z.encode_plain(b"x" * 100)
    assert base91z.decode(text, max_bytes=100) == b"x" * 100
    with pytest.raises(base91z.Base91zDecodeError):
        base91z.decode(text, max_bytes=99)


def test_every_documented_code_is_a_string():
    # A caller branches on `code`, so it must be the name and not a number.
    with pytest.raises(base91z.Base91zDecodeError) as excinfo:
        base91z.decode("m-" + chr(0x41))  # a signal with its fields cut off
    assert isinstance(excinfo.value.code, str)
    assert excinfo.value.code.islower()
    assert isinstance(excinfo.value.position, int)
