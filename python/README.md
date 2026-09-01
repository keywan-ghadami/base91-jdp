# Base91z for Python

**A binary-to-text encoding: arbitrary bytes written as characters, so they can
travel inside a text protocol. When that protocol is JSON, this is the smallest
one there is.**

```python
import base91z, json

text = base91z.encode(payload)          # compresses where compression pays
json.dumps({"blob": text})              # nothing to escape; the size you see is the size you pay
assert base91z.decode(text) == payload
```

Base64 costs a third more size on every byte, forever. Base91z encodes the
[benchmark corpus](https://bench.ghadami.de) to 0.374 characters a byte against
Base64's 1.333, and it does that with the compressor *inside* the format rather
than as a stage in front of it: it compresses where compression pays, carries
the payload with a typed class where it does not, and the same `decode` reads
either.

There is no Python implementation of the format. This package is a
[PyO3](https://pyo3.rs) layer over the Rust library, so what runs here is the
same encoder and decoder every other caller gets — see
[the repository](https://github.com/keywan-ghadami/base91z).

## Install

```sh
pip install base91z
```

Wheels are abi3, so one wheel serves every CPython from 3.9 up.

## The API

```python
encode(data, /, level=None)        -> str    # compressing; always succeeds
encode_plain(data, /, threads=1)   -> str    # no compressor, no level
decode(s, /, max_bytes=None)       -> bytes  # raises Base91zDecodeError
```

`data` is `bytes` or `bytearray`; `decode` also takes `str`. Type stubs and the
PEP 561 marker ship in the wheel.

### Set `max_bytes` on anything you did not encode yourself

A length field is a few characters and can declare far more output than the
stream is long. `decode` without a ceiling will do what the stream asks; with
one, it refuses before reserving the memory, and raises rather than truncating.

```python
base91z.decode(untrusted, max_bytes=1 << 20)
```

### Do not sign or key on the text

`encode`'s level is part of the *encoding*, not of the payload: the same bytes
at two levels give two different strings, both valid, both decoding back to
those bytes. A signature over the string, a cache key, an ETag or a test
fixture will break on a level change or a version bump.

```python
digest = hashlib.sha256(payload).hexdigest()      # yes
digest = hashlib.sha256(text.encode()).hexdigest()  # no
```

`decode(encode(x)) == x` is the guarantee. If the encoded form itself has to be
reproducible, Base64 has no parameters and is the better choice.

### Errors

```python
try:
    data = base91z.decode(text, max_bytes=1 << 20)
except base91z.Base91zDecodeError as e:
    e.code       # "invalid_character", "invalid_length", ... (spec section 13)
    e.position   # the character offset it was found at
```

`Base91zDecodeError` is a `ValueError`, so `except ValueError` catches it too.

## Licence

Mozilla Public License 2.0.
