# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Base91z: a binary-to-text encoding for bytes inside text protocols.

Everything in this package comes from `base91z.base91z`, the extension module
built from Rust -- see `src/lib.rs`. The package exists so that the type stubs
and the PEP 561 marker have somewhere a type checker recognises; it adds no
behaviour of its own, and re-exports exactly what the extension lists in its
`__all__`.
"""

from . import base91z as _extension
from .base91z import *  # noqa: F401,F403

# The extension's own docstring is the one worth showing, and its `__all__` is
# the single list of what this package exports -- kept next to the code that
# defines the names rather than transcribed here.
__doc__ = _extension.__doc__
__all__ = list(_extension.__all__)
