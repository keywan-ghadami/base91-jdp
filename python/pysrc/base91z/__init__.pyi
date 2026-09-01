# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Type stubs for the compiled `base91z` extension module.

The module is built from Rust (see `src/lib.rs`), so nothing in it carries a
signature a type checker can read: without this file `encode` is `Any` and a
typo in an argument name is a runtime error rather than a red squiggle. It
ships in the wheel next to the extension, with the PEP 561 marker.

Keep it in step with `src/lib.rs`; `tests/test_stubs.py` checks that the two
still describe the same module.
"""

from typing import Final, Optional, Union

__version__: str
__all__: list[str]

SPEC_VERSION: Final[str]
HAS_COMPRESSOR: Final[bool]
DEFAULT_LEVEL: Final[int]
ALPHABET: Final[str]
MAX_SEGMENT_BYTES: Final[int]
MAX_FRAME_BYTES: Final[int]
MAX_BLOCK_BYTES: Final[int]
MAX_FRAME_PLAIN_BYTES: Final[int]
PARALLEL_ALIGN: Final[int]

class Base91zDecodeError(ValueError):
    """Raised by `decode` on malformed input.

    `code` is one of the specification's section 13 conditions as a lower-case
    string -- `"invalid_character"`, `"unexpected_end_of_stream"`,
    `"unknown_class"`, `"extended_class"`, `"invalid_flush"`,
    `"invalid_params"`, `"invalid_length"`, `"invalid_final_block"`,
    `"invalid_index"`, `"invalid_run_value"`, `"malformed_padding"` or
    `"malformed_frame"`. `position` is the character offset at which the
    condition was detected.
    """

    code: str
    position: int

def encode(data: Union[bytes, bytearray], /, level: Optional[int] = None) -> str:
    """Encode bytes, compressing where that is smaller. Always succeeds.

    `level` is part of the encoding, not of the payload: the same bytes at two
    levels give two different strings. Do not sign or key on the text.
    """

def encode_plain(data: Union[bytes, bytearray], /, threads: int = 1) -> str:
    """Encode without compressing. No level, so it is stable within a version.

    `threads` changes only the speed; 0 means one per available core.
    """

def decode(
    s: Union[str, bytes, bytearray], /, max_bytes: Optional[int] = None
) -> bytes:
    """Decode back to bytes, raising `Base91zDecodeError` on malformed input.

    Set `max_bytes` for anything you did not encode yourself.
    """
