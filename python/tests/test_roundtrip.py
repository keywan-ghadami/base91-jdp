# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""What the bindings promise: bytes in, the same bytes out, through a string
that a JSON document will carry without escaping any of it."""

import json

import pytest

import base91z

CASES = [
    b"",
    b"a",
    b"\x00",
    bytes(300),
    b'{"user":"ada","id":42,"role":"admin"}',
    bytes(range(256)),
    b"\xff" * 1000,
    "grüße, welt".encode(),
]


@pytest.mark.parametrize("data", CASES)
def test_round_trip(data):
    assert base91z.decode(base91z.encode(data)) == data
    assert base91z.decode(base91z.encode_plain(data)) == data


@pytest.mark.parametrize("data", CASES)
def test_the_output_needs_no_escaping(data):
    # The property the format exists for, asserted against a real JSON encoder
    # rather than against a list of characters we believe are the dangerous
    # ones: the quoted form is the text plus its two quotation marks.
    text = base91z.encode(data)
    assert len(json.dumps(text)) == len(text) + 2
    assert json.loads(json.dumps(text)) == text


@pytest.mark.parametrize("data", CASES)
def test_the_alphabet_is_the_alphabet(data):
    assert set(base91z.encode(data)) <= set(base91z.ALPHABET)


def test_bytearray_is_accepted_and_a_list_is_not():
    assert base91z.decode(base91z.encode(bytearray(b"abc"))) == b"abc"
    with pytest.raises(TypeError):
        base91z.encode([97, 98, 99])
    with pytest.raises(TypeError):
        base91z.encode("a string is not bytes")


def test_decode_accepts_str_bytes_and_bytearray():
    text = base91z.encode(b"hello")
    assert base91z.decode(text) == b"hello"
    assert base91z.decode(text.encode("ascii")) == b"hello"
    assert base91z.decode(bytearray(text.encode("ascii"))) == b"hello"


def test_threads_do_not_change_the_output():
    data = bytes(range(256)) * 4000
    serial = base91z.encode_plain(data, threads=1)
    assert base91z.encode_plain(data, threads=4) == serial
    assert base91z.encode_plain(data, threads=0) == serial
    assert base91z.decode(serial) == data
