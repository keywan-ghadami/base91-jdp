# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Training corpus for the donor-profile derivation (tools/deriveprofiles.js).

Disjoint from the benchmark corpus of binary2textbench: no archive, no file
and no upstream project is shared between the two, so the profile table in
the specification is not fitted to the files it is later measured on.

Like the benchmark corpus, nothing is vendored. Every archive is pulled from
a pinned URL and verified against a recorded SHA-256, so a rerun either
reproduces the same bytes or fails loudly.
"""

from __future__ import annotations

import hashlib
import shutil
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRAIN_DIR = ROOT / "bench" / "train"
CACHE_DIR = TRAIN_DIR / "_archives"

# (key, url, sha256, kind)
ARCHIVES = [
    (
        "jquery",
        "https://registry.npmjs.org/jquery/-/jquery-3.7.1.tgz",
        "68a9f787516da47c680e09c187bcbac4536b6f85d90eb882844e12919e583f53",
        "tar.gz",
    ),
    (
        "marked",
        "https://registry.npmjs.org/marked/-/marked-12.0.2.tgz",
        "6cfd2d09c6bce2541558a1547e5f3b9895ed743f0d287536ff2280e318e8a074",
        "tar.gz",
    ),
    (
        "handlebars",
        "https://registry.npmjs.org/handlebars/-/handlebars-4.7.8.tgz",
        "2dadfd6743a1e0d876e55d5c5622450a99c1316848048bc6c131a5fc42e776a6",
        "tar.gz",
    ),
    (
        "click",
        "https://files.pythonhosted.org/packages/96/d3/"
        "f04c7bfcf5c1862a2a5b845c6b2b360488cf47af55dfa79c98f6a6bf98b5/"
        "click-8.1.7.tar.gz",
        "ca9853ad459e787e2192211578cc907e7594e294c7ccc834310722b41b9ca6de",
        "tar.gz",
    ),
    (
        "jinja2",
        "https://files.pythonhosted.org/packages/ed/55/"
        "39036716d19cab0747a5020fc7e907f362fbf48c984b14e62127f7e68e5d/"
        "jinja2-3.1.4.tar.gz",
        "4a3aee7acbbe7303aede8e9648d13b8bf88a429282aa6122a993f0ac800cb369",
        "tar.gz",
    ),
    (
        "rich",
        "https://files.pythonhosted.org/packages/b3/01/"
        "c954e134dc440ab5f96952fe52b4fdc64225530320a910473c1fe270d9aa/"
        "rich-13.7.1.tar.gz",
        "9be308cb1fe2f1f57d67ce99e95af38a1e2bc71ad9813b0e247cf7ffbcc3a432",
        "tar.gz",
    ),
]

# Text extensions worth training on. Binary members are skipped: a donor
# profile only ever applies to passthrough segments, which are text.
TEXT_SUFFIXES = {
    ".js", ".mjs", ".ts", ".py", ".go", ".c", ".h", ".rs", ".java",
    ".md", ".rst", ".txt", ".json", ".yml", ".yaml", ".toml", ".cfg",
    ".ini", ".css", ".html", ".htm", ".xml", ".svg", ".sh", ".csv",
}

MAX_BYTES = 2_200_000
MAX_MEMBER = 400_000
MIN_MEMBER = 500


def _fetch(key: str, url: str, sha: str, kind: str) -> Path:
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    dest = CACHE_DIR / f"{key}{'.zip' if kind == 'zip' else '.tar.gz'}"
    if dest.exists() and _sha256(dest.read_bytes()) == sha:
        return dest
    print(f"  downloading {key} ...", file=sys.stderr)
    with urllib.request.urlopen(url, timeout=180) as resp:
        blob = resp.read()
    got = _sha256(blob)
    if sha and got != sha:
        raise SystemExit(f"SHA-256 mismatch for {url}\n  expected {sha}\n  got      {got}")
    dest.write_bytes(blob)
    return dest


def _sha256(blob: bytes) -> str:
    return hashlib.sha256(blob).hexdigest()


def _members(path: Path, kind: str):
    if kind == "zip":
        with zipfile.ZipFile(path) as zf:
            for info in zf.infolist():
                if info.is_dir():
                    continue
                yield info.filename, zf.read(info)
    else:
        with tarfile.open(path, "r:gz") as tf:
            for info in tf:
                if not info.isfile():
                    continue
                fh = tf.extractfile(info)
                if fh is None:
                    continue
                yield info.name, fh.read()


def build() -> None:
    TRAIN_DIR.mkdir(parents=True, exist_ok=True)
    for old in TRAIN_DIR.glob("*.train"):
        old.unlink()

    total = 0
    kept = 0
    # A per-archive budget: without it the first archive in the list, which is
    # also the largest, would fill the whole corpus and the profile table would
    # be derived from one project's JavaScript.
    budget = MAX_BYTES // len(ARCHIVES)
    for key, url, sha, kind in ARCHIVES:
        path = _fetch(key, url, sha, kind)
        spent = 0
        for name, blob in sorted(_members(path, kind)):
            if spent >= budget:
                break
            if Path(name).suffix.lower() not in TEXT_SUFFIXES:
                continue
            if not (MIN_MEMBER <= len(blob) <= MAX_MEMBER):
                continue
            flat = name.replace("/", "_")
            (TRAIN_DIR / f"{key}_{flat}.train").write_bytes(blob)
            total += len(blob)
            spent += len(blob)
            kept += 1
    print(f"{kept} files, {total} bytes in {TRAIN_DIR}")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "clean":
        shutil.rmtree(TRAIN_DIR, ignore_errors=True)
        print("training corpus removed")
    else:
        build()
