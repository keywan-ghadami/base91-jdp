# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""The stubs describe the module that is actually there.

A `.pyi` that has drifted is worse than none: a type checker reports errors
about a function that no longer has that signature, and everyone learns to
ignore it.
"""

import ast
import pathlib

import base91z

STUB = pathlib.Path(base91z.__file__).with_name("__init__.pyi")


def stub_names():
    tree = ast.parse(STUB.read_text(encoding="utf-8"))
    names = set()
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            names.add(node.name)
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names.add(node.target.id)
    return names


def test_the_stub_ships_beside_the_extension():
    assert STUB.is_file(), "no __init__.pyi next to the installed package"
    assert STUB.with_name("py.typed").is_file(), "no PEP 561 marker"


def test_everything_exported_is_in_the_stub():
    missing = sorted(set(base91z.__all__) - stub_names())
    assert not missing, f"exported but not in __init__.pyi: {missing}"


def test_the_stub_invents_nothing():
    extra = sorted(stub_names() - set(base91z.__all__) - {"__all__"})
    assert not extra, f"in __init__.pyi but not exported: {extra}"


def test_all_is_importable():
    for name in base91z.__all__:
        assert hasattr(base91z, name), f"__all__ names {name}, which is not there"
