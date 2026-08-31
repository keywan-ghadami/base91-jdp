#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Static site generator for the Base91z project website.

The site has no content of its own: every page, the landing page included, is
rendered from a Markdown file the repository already ships -- the README, the
specification, the benchmark report, the changelog -- so the website cannot
drift from the repository. Repository-relative links in those files are
rewritten either to the corresponding generated page or to an absolute
github.com URL.

This generator is Base85N's, adapted: the page list, the identity and the
footer are this project's, and the machinery -- specification discovery, the
index checks, GitHub-compatible heading anchors, link rewriting -- is
unchanged, because both repositories publish the same shape of document.

Where a source carries something that only makes sense on GitHub -- the badge
row at the top of the README, its link *to* this site -- a filter in
``SOURCE_FILTERS`` removes it. Filters only ever remove; nothing on this site
is written twice.

Usage::

    pip install -r site/requirements.txt
    python3 site/build.py [--output DIR] [--serve]

The default output directory is ``site/_build``, which is git-ignored.
"""

from __future__ import annotations

import argparse
import html
import os
import re
import shutil
import sys

try:
    import markdown
except ImportError:  # pragma: no cover - developer convenience
    sys.exit(
        "python-markdown is required: pip install -r site/requirements.txt"
    )

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SITE_DIR = os.path.join(REPO_ROOT, "site")

GITHUB_REPO = "https://github.com/keywan-ghadami/base91z"
# The same thing without the scheme, for link text. Derived, because the
# repository was renamed once and the footer went on naming the old one.
GITHUB_LABEL = GITHUB_REPO.split("://", 1)[1]
GITHUB_BLOB = GITHUB_REPO + "/blob/main/"
GITHUB_TREE = GITHUB_REPO + "/tree/main/"

SITE_TITLE = "base91z"
SITE_TAGLINE = (
    "basE91 on an alphabet that JSON never has to escape. The encoded size is "
    "the final size, and a stream says what it carries."
)

# The current documents are `base91z-vX.Y.Z.md`. Superseded ones under
# `spec/history/` keep the name the format had when they were written, which
# up to v0.3.0 was base91z -- renaming a published document would make it
# a different document.
SPEC_FILE_RE = re.compile(r"^(?:base91z)-v(\d+)\.(\d+)\.(\d+)\.md$")
# The metadata table at the top of a specification. Older versions bold the
# field name, newer ones do not, so the asterisks are optional here.
SPEC_FIELD_RE = re.compile(
    r"^\|\s*\*{0,2}(Version|Status|Date)\*{0,2}\s*\|\s*([^|]+?)\s*\|", re.M
)


def discover_specs(subdir):
    """Every specification in one directory, newest first.

    The specification directories are the source of truth: a version is a
    file, and the file carries its own version, status and date in the
    metadata table at the top. Deriving the page list from that means adding
    or retiring a version is one commit to `spec/` and nothing here -- and it
    means the site cannot disagree with the document about what version it is.

    `spec/` holds exactly one document, the current specification, because a
    first-time reader should meet one. Superseded versions live in
    `spec/history/` and are discovered the same way.

    Returns a list of dicts with `path`, `version` (a sortable tuple), `label`,
    `status` and `date`.
    """
    spec_dir = os.path.join(REPO_ROOT, *subdir.split("/"))
    found = []
    for name in os.listdir(spec_dir):
        m = SPEC_FILE_RE.match(name)
        if not m:
            continue
        with open(os.path.join(spec_dir, name), encoding="utf-8") as fh:
            head = fh.read(2048)
        fields = dict(SPEC_FIELD_RE.findall(head))
        label = fields.get("Version", ".".join(m.groups()))
        if label != ".".join(m.groups()):
            raise SystemExit(
                "spec/%s says it is version %s; the file name says %s"
                % (name, label, ".".join(m.groups()))
            )
        found.append({
            "path": subdir + "/" + name,
            "version": tuple(int(g) for g in m.groups()),
            "label": label,
            "status": fields.get("Status", "Draft"),
            "date": fields.get("Date", ""),
        })
    if not found:
        raise SystemExit("no specification documents found in " + subdir)
    found.sort(key=lambda s: s["version"], reverse=True)
    return found


# What a specification document is called, loosely enough to survive another
# rename of the format: a name, a version, `.md`.
SPEC_LINK_RE = re.compile(r"^[A-Za-z0-9-]+-v\d+\.\d+\.\d+\.md$")

# A row of a specification index: the cells between the outer pipes.
INDEX_ROW_RE = re.compile(r"^\|(.+)\|\s*$", re.M)


def index_rows(index):
    """Every table row in an index, as a list of stripped cells."""
    return [[cell.strip() for cell in m.group(1).split("|")]
            for m in INDEX_ROW_RE.finditer(index)]


def index_date_column(index):
    """Which column of the index holds a document's date, if any.

    `spec/README.md` has one; `spec/history/README.md` has "Superseded by"
    there instead, because what a reader of that page wants is which version
    replaced this one. So this returns None rather than guessing, and the date
    check below applies only where there is a date to check.
    """
    for cells in index_rows(index):
        for i, cell in enumerate(cells):
            if cell.lower() == "date":
                return i
        # Only the header row can declare it, and it is the first row.
        return None
    return None


def spec_index_problems(index, directory, specs):
    """What is wrong between one index and the documents beside it.

    Returns a list of one-line problems, empty when the two agree. Separated
    from the file reading so it can be tested against an index that is wrong
    on purpose -- see `site/test_checks.py`, which exists because two of the
    checks below were silently dead.
    """
    problems = []
    # Only links that stay inside this directory count. An index may point at
    # a document in the other one -- the history index links up to the current
    # specification, and should -- and a `../` link is exactly that, not a
    # claim about what is here.
    names = {os.path.basename(s["path"]) for s in specs}
    for spec in specs:
        name = os.path.basename(spec["path"])
        if not re.search(r"(?<!\.\./)" + re.escape(name), index):
            problems.append("not listed: " + spec["path"])

    # An entry is a link, and only a link whose target stays in this
    # directory: the history index points up at the current specification with
    # `../`, which is not a claim about what is here.
    #
    # Matched case-insensitively against actual link targets rather than
    # against the text. The pattern this replaces looked for `Base91z-v*.md`
    # in the prose while every document had been renamed to lowercase
    # `base91z-`, so nothing was ever stale and the check was dead for every
    # commit after the rename; and a name pattern loose enough to survive
    # another rename will happily start matching in the middle of `../name`,
    # which a lookbehind does not prevent.
    linked = set()
    for target in re.findall(r"\]\(\s*<?([^)<>\s]+)>?\s*\)", index):
        if "/" in target or "://" in target:
            continue
        if SPEC_LINK_RE.match(target):
            linked.add(target.lower())
    for name in sorted(linked - {n.lower() for n in names}):
        problems.append("listed but missing: %s/%s" % (directory, name))

    # And the date, where the index publishes one. The build already refuses a
    # document whose file name and version field disagree; this is the same
    # rule for the date, which had drifted by six days before anything looked.
    column = index_date_column(index)
    if column is not None:
        for spec in specs:
            name = os.path.basename(spec["path"])
            for cells in index_rows(index):
                if any(name in cell for cell in cells) and len(cells) > column:
                    if cells[column] != spec["date"]:
                        problems.append(
                            "%s says it is dated %s; the index says %s"
                            % (spec["path"], spec["date"], cells[column])
                        )
    return problems


def check_spec_index(index_doc, specs):
    """Every specification in a directory is listed in that directory's index,
    nothing is listed there that does not exist, and the index agrees with each
    document about its date.

    The indexes are repository documents -- they are read on GitHub too -- so
    they are written by hand rather than generated. This is what stops a
    version from being published as a page nobody links to, or a link from
    outliving its file. The check runs at build time, which is CI.

    It also keeps the two directories from blurring: a superseded version left
    in `spec/` would fail, because `spec/README.md` names one document and
    means it.
    """
    directory = os.path.dirname(index_doc)
    index_path = os.path.join(REPO_ROOT, *index_doc.split("/"))
    with open(index_path, encoding="utf-8") as fh:
        index = fh.read()
    problems = spec_index_problems(index, directory, specs)
    if problems:
        raise SystemExit(
            "%s is out of step with %s/:\n" % (index_doc, directory)
            + "".join("  %s\n" % p for p in problems)
        )


SPECS = discover_specs("spec")
HISTORIC_SPECS = discover_specs("spec/history")
check_spec_index("spec/README.md", SPECS)
check_spec_index("spec/history/README.md", HISTORIC_SPECS)

if len(SPECS) != 1:
    raise SystemExit(
        "spec/ holds %d specifications; it holds the current one and nothing "
        "else, so a first-time reader meets one document. Superseded versions "
        "belong in spec/history/." % len(SPECS)
    )

# Shown in the footer. Derived, so neither a new specification version nor a
# change of its status leaves a stale claim on every page -- the document says
# what it is, and the footer repeats it.
SPEC_VERSION = SPECS[0]["label"]
SPEC_STATUS = SPECS[0]["status"].lower()


def spec_pages():
    """The current specification, then every superseded one, newest first.

    The current one carries its own status; every older one is described as
    superseded by the version that followed it, which is the next entry up.
    Only the current one goes in the navigation -- the rest are reached from
    the history index, which is the whole point of their being there.
    """
    chain = SPECS + HISTORIC_SPECS
    pages = []
    for i, spec in enumerate(chain):
        if i == 0:
            state = spec["status"]
        else:
            state = "Superseded by " + chain[i - 1]["label"]
        subtitle = " - ".join(
            part for part in ("Version " + spec["label"], state, spec["date"]) if part
        )
        pages.append(Page(
            source=spec["path"],
            output=spec["path"].replace(".md", ".html"),
            title="Base91z Specification v" + spec["label"],
            toc=True,
            subtitle=subtitle,
            strip_first_heading=True,
        ))
    return pages

MARKDOWN_EXTENSIONS = ["tables", "fenced_code", "toc", "attr_list", "sane_lists"]

# Anything that is not a letter, digit, underscore, hyphen or space. GitHub
# drops these from a heading before turning it into an anchor.
ANCHOR_DROP_RE = re.compile(r"[^\w\- ]", re.UNICODE)


def github_slugify(value, separator):
    """Heading -> anchor id, the way GitHub does it.

    Every Markdown file here is read on GitHub as well as on this site, and its
    cross-references are written against GitHub's ids. python-markdown's own
    slugify collapses runs of whitespace, so "Reviews wanted - the rest"
    becomes ``reviews-wanted-the-rest`` there and ``reviews-wanted--the-rest``
    on GitHub -- one dash apart, and a broken link on whichever side is not
    matched. Not collapsing is the whole difference.
    """
    return ANCHOR_DROP_RE.sub("", value.lower()).replace(" ", separator)


class Page:
    """One generated HTML page, rendered from one Markdown source file."""

    def __init__(self, source, output, title, nav_label=None, toc=False,
                 subtitle=None, strip_first_heading=False, link_base=None,
                 body_class="", lang="en"):
        self.source = source  # repo-relative path of the Markdown source
        self.output = output  # site-relative path of the generated HTML
        self.title = title
        self.nav_label = nav_label
        self.toc = toc
        self.subtitle = subtitle
        self.strip_first_heading = strip_first_heading
        # Directory that relative links in this source resolve against: the
        # source's own directory, so the same links keep working on GitHub.
        self.link_base = (
            os.path.dirname(source) if link_base is None else link_base
        )
        # Extra class on the page wrapper, for the few rules that apply to one
        # page only (see ``.page-home`` in assets/style.css).
        self.body_class = body_class
        # Document language. Everything here is written in English except the
        # Impressum, which is German because the law that requires it is; a
        # screen reader should be told which is which.
        self.lang = lang


PAGES = [
    Page(
        # The landing page is the README, so the two cannot disagree about what
        # the format is or what it measures.
        source="README.md",
        output="index.html",
        title="base91z",
        nav_label="Home",
        subtitle=SITE_TAGLINE,
        strip_first_heading=True,
        body_class=" page-home",
    ),
    Page(
        source="spec/README.md",
        output="spec/index.html",
        title="Specification versions",
        nav_label="Spec",
        subtitle="Every published version of the Base91z specification.",
        strip_first_heading=True,
    ),
    *spec_pages(),
    Page(
        source="spec/history/README.md",
        output="spec/history/index.html",
        title="History",
        subtitle="Superseded versions, kept so a decision can be traced to the document that made it.",
        strip_first_heading=True,
    ),
    Page(
        source="bench/README.md",
        output="benchmarks/index.html",
        title="Benchmarks",
        nav_label="Benchmarks",
        toc=True,
        subtitle=(
            "The three corpora the measurements run on, and how to reproduce "
            "them. The numbers themselves are in the specification."
        ),
        strip_first_heading=True,
    ),
    Page(
        source="rust/README.md",
        output="implementation.html",
        title="The implementation",
        nav_label="Implementation",
        toc=True,
        subtitle=(
            "A Rust encoder and decoder for v0.4.0, and what building it "
            "found that the arithmetic had missed."
        ),
        strip_first_heading=True,
    ),
    Page(
        source="SECURITY.md",
        output="security.html",
        title="Security",
        toc=True,
        subtitle=(
            "How to report something, what the threat model is, and what is "
            "run against the decoder."
        ),
        strip_first_heading=True,
    ),
    Page(
        # Linked from the footer of every page rather than the navigation: it
        # has to be reachable from anywhere, and it is not what a reader came
        # for.
        source="IMPRESSUM.md",
        output="impressum.html",
        title="Impressum",
        lang="de",
        subtitle="Anbieterkennzeichnung nach § 5 DDG und § 18 MStV, Haftung und Nutzung der Inhalte.",
        strip_first_heading=True,
    ),
    Page(
        source="CHANGELOG.md",
        output="changelog.html",
        title="Changelog",
        nav_label="Changelog",
        toc=True,
        subtitle="What changed in each release, and what measurement changed it.",
        strip_first_heading=True,
    ),
]

# Repository paths that have a generated page. Keys are repo-relative paths
# exactly as they may appear in a Markdown link target.
PATH_TO_PAGE = {
    "README.md": "index.html",
    "spec/README.md": "spec/index.html",
    "spec": "spec/index.html",
    "spec/history/README.md": "spec/history/index.html",
    "spec/history": "spec/history/index.html",
    **{s["path"]: s["path"].replace(".md", ".html")
       for s in SPECS + HISTORIC_SPECS},
    "bench/README.md": "benchmarks/index.html",
    "bench": "benchmarks/index.html",
    "rust/README.md": "implementation.html",
    "rust": "implementation.html",
    "IMPRESSUM.md": "impressum.html",
    "SECURITY.md": "security.html",
    "CHANGELOG.md": "changelog.html",
}

# A line that is nothing but shields.io badges, and the README bullet that
# links to this very site. Both are GitHub chrome: the badges report CI state
# to someone reading the repository, and the site does not need a link to
# itself in its own first list.
BADGE_LINE_RE = re.compile(r"^\s*(?:\[!\[[^\]]*\]\([^)]*\)\]\([^)]*\)\s*)+$", re.MULTILINE)


def strip_github_chrome(text):
    """Remove what only makes sense when the README is read on GitHub.

    Every badge here sits on a line of its own, so unlike upstream this strips
    all of them rather than the first line: a line that is nothing but badges
    is repository chrome wherever it appears.
    """
    return BADGE_LINE_RE.sub("", text)


# Per-source Markdown filters, applied before conversion. Keyed by the same
# repo-relative path a Page names.
SOURCE_FILTERS = {
    "README.md": strip_github_chrome,
}


TEMPLATE = """<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{description}">
<link rel="stylesheet" href="{root}assets/style.css">
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='72'>91</text></svg>">
</head>
<body>
<a class="skip-link" href="#content">Skip to content</a>
<header class="site-header">
  <div class="wrap header-inner">
    <a class="brand" href="{root}index.html"><span class="brand-mark">91</span><span>Base91z</span></a>
    <nav class="site-nav">{nav}
      <a class="nav-external" href="{repo}">GitHub</a>
    </nav>
  </div>
</header>
<div class="wrap page{page_class}">
{sidebar}
<main id="content" class="content">
<div class="page-head">
<h1>{heading}</h1>
{subtitle}
</div>
{body}
</main>
</div>
<footer class="site-footer">
  <div class="wrap">
    <p><strong>Base91z</strong> - specification v{spec_version} ({spec_status}),
    with a prototype encoder and decoder in Rust.</p>
    <p class="footer-warn">Specification and implementation were drafted with AI
    assistance and then verified against measurements, and reviews - security,
    documentation, usability, anything - are wanted. The specification carries
    its own security considerations; read them before decoding untrusted
    input.</p>
    <p class="footer-meta">Source: <a href="{repo}">{repo_label}</a>
    &middot; This page is generated from <a href="{source_url}">{source}</a>
    &middot; Contact: <a href="mailto:keywan.ghadami@gmail.com">keywan.ghadami@gmail.com</a>
    &middot; <a href="{root}security.html">Security</a>
    &middot; <a href="{root}impressum.html">Impressum</a></p>
  </div>
</footer>
</body>
</html>
"""


def relative_url(from_output, to_output):
    """URL of ``to_output`` as seen from the page at ``from_output``."""
    from_dir = os.path.dirname(from_output) or "."
    rel = os.path.relpath(to_output, from_dir)
    return rel.replace(os.sep, "/")


def rewrite_link(target, link_base, output_path):
    """Rewrite one Markdown link target for the generated site."""
    if not target or target.startswith(
        ("http://", "https://", "mailto:", "tel:", "#")
    ):
        return target

    anchor = ""
    if "#" in target:
        target, anchor = target.split("#", 1)
        anchor = "#" + anchor
    if not target:
        return anchor

    resolved = os.path.normpath(os.path.join(link_base, target))
    resolved = resolved.replace(os.sep, "/").rstrip("/")

    if resolved in PATH_TO_PAGE:
        return relative_url(output_path, PATH_TO_PAGE[resolved]) + anchor

    # Everything else stays in the repository: link to GitHub so the page is
    # useful rather than 404-ing on a path the site does not publish.
    on_disk = os.path.join(REPO_ROOT, resolved)
    prefix = GITHUB_TREE if os.path.isdir(on_disk) else GITHUB_BLOB
    return prefix + resolved + anchor


HREF_RE = re.compile(r'(<a\b[^>]*?\shref=")([^"]*)(")', re.IGNORECASE)


def rewrite_links(body_html, link_base, output_path):
    def replace(match):
        target = html.unescape(match.group(2))
        return match.group(1) + html.escape(
            rewrite_link(target, link_base, output_path), quote=True
        ) + match.group(3)

    return HREF_RE.sub(replace, body_html)


FIRST_H1_RE = re.compile(r"^\s*<h1[^>]*>.*?</h1>\s*", re.IGNORECASE | re.DOTALL)


def build_nav(output_path):
    items = []
    for page in PAGES:
        if not page.nav_label:
            continue
        href = relative_url(output_path, page.output)
        current = ' class="current"' if page.output == output_path else ""
        items.append(
            '\n      <a href="%s"%s>%s</a>' % (href, current, page.nav_label)
        )
    return "".join(items)


def first_paragraph_text(body_html):
    match = re.search(r"<p>(.*?)</p>", body_html, re.DOTALL)
    if not match:
        return SITE_TAGLINE
    text = re.sub(r"<[^>]+>", "", match.group(1))
    text = html.unescape(text).strip().replace("\n", " ")
    return (text[:180] + "...") if len(text) > 180 else text


def render_page(page, output_dir):
    source_abs = os.path.join(REPO_ROOT, page.source)
    with open(source_abs, encoding="utf-8") as fh:
        text = fh.read()

    source_filter = SOURCE_FILTERS.get(page.source)
    if source_filter:
        text = source_filter(text)

    converter = markdown.Markdown(
        extensions=MARKDOWN_EXTENSIONS,
        extension_configs={
            "toc": {
                "permalink": "#",
                "toc_depth": "2-3",
                "slugify": github_slugify,
            }
        },
    )
    body = converter.convert(text)
    toc_html = getattr(converter, "toc", "")

    if page.strip_first_heading:
        body = FIRST_H1_RE.sub("", body, count=1)

    body = rewrite_links(body, page.link_base, page.output)
    toc_html = rewrite_links(toc_html, page.link_base, page.output)

    depth = page.output.count("/")
    root = "../" * depth

    sidebar = ""
    if page.toc and toc_html:
        sidebar = (
            '<aside class="toc" aria-label="Table of contents">'
            '<p class="toc-title">On this page</p>%s</aside>' % toc_html
        )

    rendered = TEMPLATE.format(
        lang=html.escape(page.lang, quote=True),
        title=html.escape(
            page.title if page.output == "index.html"
            else "%s - %s" % (page.title, SITE_TITLE)
        ),
        description=html.escape(page.subtitle or first_paragraph_text(body)),
        heading=html.escape(page.title),
        subtitle=(
            '<p class="subtitle">%s</p>' % html.escape(page.subtitle)
            if page.subtitle else ""
        ),
        body=body,
        nav=build_nav(page.output),
        sidebar=sidebar,
        page_class=(" has-toc" if sidebar else "") + page.body_class,
        root=root,
        repo=GITHUB_REPO,
        repo_label=html.escape(GITHUB_LABEL),
        source=html.escape(page.source),
        source_url=GITHUB_BLOB + page.source,
        spec_version=SPEC_VERSION,
        spec_status=html.escape(SPEC_STATUS),
    )

    destination = os.path.join(output_dir, page.output)
    os.makedirs(os.path.dirname(destination) or output_dir, exist_ok=True)
    with open(destination, "w", encoding="utf-8") as fh:
        fh.write(rendered)
    return destination


def build(output_dir):
    if os.path.exists(output_dir):
        shutil.rmtree(output_dir)
    os.makedirs(output_dir)

    shutil.copytree(
        os.path.join(SITE_DIR, "assets"), os.path.join(output_dir, "assets")
    )
    # GitHub Pages must serve these files verbatim, not run them through Jekyll.
    open(os.path.join(output_dir, ".nojekyll"), "w").close()

    for page in PAGES:
        print("  %-40s <- %s" % (page.output, page.source))
        render_page(page, output_dir)

    print("built %d pages into %s" % (len(PAGES), output_dir))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        default=os.path.join(SITE_DIR, "_build"),
        help="output directory (default: site/_build)",
    )
    parser.add_argument(
        "--serve",
        action="store_true",
        help="serve the built site on http://localhost:8000 afterwards",
    )
    args = parser.parse_args()

    build(args.output)

    if args.serve:
        import functools
        import http.server

        handler = functools.partial(
            http.server.SimpleHTTPRequestHandler, directory=args.output
        )
        print("serving on http://localhost:8000 (Ctrl-C to stop)")
        http.server.ThreadingHTTPServer(("", 8000), handler).serve_forever()


if __name__ == "__main__":
    main()
