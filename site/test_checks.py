#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Regression tests for the generator, one per bug that reached the published
site.

Every test here is a bug that was live: a link scheme rewritten into a
repository path, a footer naming a repository that had been renamed, an index
check that had gone dead and stopped checking anything, an index date that had
drifted from the document. `check_links.py` verifies the *output*; these verify
the parts of the generator that decide what the output says, including the
parts whose failure mode is to silently pass.

Standard library only, no test runner to install::

    python3 site/test_checks.py
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import build  # noqa: E402  -- after the path insert, by necessity


SPECS = [{"path": "spec/base91z-v9.9.9.md", "date": "2026-01-02"}]


def index(rows, header="| Version | Status | Date |"):
    return header + "\n|---|---|---|\n" + "\n".join(rows) + "\n"


class LinkSchemes(unittest.TestCase):
    """A `tel:` link in IMPRESSUM.md was rewritten to

    ``https://github.com/keywan-ghadami/base91z/blob/main/tel:+49...``

    because only http, https and mailto were recognised as schemes and
    everything else was taken for a repository path. The published page had a
    telephone number that opened GitHub.
    """

    def test_schemes_are_left_alone(self):
        for target in ("tel:+4917620785913", "mailto:a@b.example",
                       "https://example.org/x", "http://example.org/x",
                       "#anchor"):
            self.assertEqual(
                build.rewrite_link(target, "", "impressum.html"), target,
                "%s was rewritten" % target,
            )

    def test_a_repository_path_is_still_rewritten(self):
        # The other half of the rule: a real relative path must still become a
        # link that resolves, or this test would pass on a function that gave
        # up entirely.
        self.assertEqual(
            build.rewrite_link("README.md", "", "impressum.html"), "index.html"
        )


class FooterIdentity(unittest.TestCase):
    """Every page's footer read "Source: github.com/keywan-ghadami/base91-jdp"
    while linking to `base91z`: the repository had been renamed and the link
    text had not. It is derived now, and this is what keeps it derived.
    """

    def test_the_label_is_the_url(self):
        self.assertTrue(build.GITHUB_REPO.endswith(build.GITHUB_LABEL))
        self.assertNotIn("://", build.GITHUB_LABEL)

    def test_the_template_does_not_spell_a_repository_name(self):
        # A literal owner/repository in the template is how the last one got
        # stale, so the template may not contain one at all.
        self.assertNotIn("keywan-ghadami/", build.TEMPLATE)


class SpecIndexChecks(unittest.TestCase):
    """`check_spec_index` had two checks that could not fail.

    The stale-entry check matched `Base91z-v*.md` while every file had been
    renamed to lowercase `base91z-`, so no entry was ever stale. And nothing
    compared the index's date column to the document's own date field, which
    is how `spec/README.md` came to publish 2026-08-25 for a document dated
    2026-08-31.
    """

    def test_a_listed_document_that_does_not_exist_is_caught(self):
        text = index([
            "| [v9.9.9](base91z-v9.9.9.md) | Final | 2026-01-02 |",
            "| [v8.8.8](base91z-v8.8.8.md) | Final | 2026-01-01 |",
        ])
        problems = build.spec_index_problems(text, "spec", SPECS)
        self.assertTrue(
            any("base91z-v8.8.8.md" in p for p in problems),
            "an entry pointing at no file passed: %r" % problems,
        )

    def test_a_document_that_is_not_listed_is_caught(self):
        text = index(["| nothing here | | |"])
        problems = build.spec_index_problems(text, "spec", SPECS)
        self.assertTrue(
            any("not listed" in p for p in problems),
            "an unlisted document passed: %r" % problems,
        )

    def test_a_date_that_disagrees_with_the_document_is_caught(self):
        text = index(["| [v9.9.9](base91z-v9.9.9.md) | Final | 2026-01-09 |"])
        problems = build.spec_index_problems(text, "spec", SPECS)
        self.assertTrue(
            any("2026-01-09" in p for p in problems),
            "a drifted date passed: %r" % problems,
        )

    def test_an_index_that_agrees_has_no_problems(self):
        text = index(["| [v9.9.9](base91z-v9.9.9.md) | Final | 2026-01-02 |"])
        self.assertEqual(build.spec_index_problems(text, "spec", SPECS), [])

    def test_an_index_without_a_date_column_is_not_asked_for_one(self):
        # `spec/history/README.md` publishes "Superseded by" in that column,
        # which is not a date and must not be read as one.
        text = index(
            ["| [v9.9.9](base91z-v9.9.9.md) | Superseded | 0.4.0 |"],
            header="| Version | Status | Superseded by |",
        )
        self.assertEqual(build.spec_index_problems(text, "spec", SPECS), [])

    def test_a_link_out_of_the_directory_is_not_an_entry(self):
        # The history index links up to the current specification with `../`,
        # and that is not a claim about what is in the history directory.
        text = index([
            "| [v9.9.9](base91z-v9.9.9.md) | Final | 2026-01-02 |",
            "see also [current](../base91z-v7.7.7.md)",
        ])
        self.assertEqual(build.spec_index_problems(text, "spec", SPECS), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
