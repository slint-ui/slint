# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cSpell:ignore samplepkg
"""Tests for the docs generator. The pure functions need neither griffe nor the
network; the integration tests run the selection/rendering against a small
fixture package under tests/fixtures/samplepkg."""

import zlib
from pathlib import Path

import griffe
import pytest

import gen_mdx

FIXTURES = Path(__file__).parent / "fixtures"


# ---- pure functions ---------------------------------------------------------


def test_python_doc_version():
    assert gen_mdx.python_doc_version(">= 3.12") == "3.12"
    assert gen_mdx.python_doc_version(">=3.13,<4") == "3.13"
    with pytest.raises(ValueError):
        gen_mdx.python_doc_version("nonsense")


def _inventory(body: str) -> bytes:
    header = b"# Sphinx inventory version 2\n# Project: x\n# Version: 1\n# zlib\n"
    return header + zlib.compress(body.encode())


def test_parse_inventory_keeps_only_py_type_roles():
    body = (
        "int py:class 1 library/functions.html#$ -\n"
        "list comprehension 1 - -\n"  # decoy: non-py role for the same name
        "dict std:2to3fixer 1 library/2to3.html#x -\n"  # decoy: std domain
        "pathlib.Path py:class 1 library/pathlib.html#$ -\n"
        "typing.Optional py:data 1 library/typing.html#$ -"
    )
    base = "https://docs.python.org/3.12/"
    links = gen_mdx.parse_inventory(_inventory(body), base)
    assert links["int"] == f"{base}library/functions.html#int"
    assert links["pathlib.Path"] == f"{base}library/pathlib.html#pathlib.Path"
    assert "typing.Optional" in links
    assert "list" not in links  # comprehension role filtered out
    assert "dict" not in links  # std: domain filtered out


def test_docstring_to_mdx_links_escapes_and_preserves_code():
    manifest = {"Thing": "/api/classes/thing/", "do_it": "/api/functions/do_it/"}
    out = gen_mdx.docstring_to_mdx("Use `Thing` and `unknown`.", manifest)
    assert '<XRef to="Thing" />' in out
    assert "`unknown`" in out  # unknown symbol stays inline code
    # trailing () is stripped when matching a function symbol
    assert '<XRef to="do_it" />' in gen_mdx.docstring_to_mdx(
        "Call `do_it()`.", manifest
    )
    # JSX-significant characters escaped in prose
    escaped = gen_mdx.docstring_to_mdx("a < b {c}", {})
    assert "&lt;" in escaped and "&#123;" in escaped and "&#125;" in escaped
    # fenced code preserved verbatim (not escaped, not linked)
    fenced = gen_mdx.docstring_to_mdx("```\n<x> {y} `Thing`\n```", manifest)
    assert "<x> {y} `Thing`" in fenced


def test_render_annotation_forward_ref_string():
    assert (
        gen_mdx.render_annotation("'Thing'", {"Thing": "/x"})
        == '<XRef to="Thing" plain />'
    )
    assert gen_mdx.render_annotation('"Unknown"', {}) == "Unknown"


def test_slug_and_page_url():
    assert gen_mdx.slug("FooBar") == "foobar"
    assert gen_mdx.page_url("api/classes", "Model") == "/api/classes/model/"
    assert (
        gen_mdx.page_url("api/classes", "Model", "row_count")
        == "/api/classes/model/#row_count"
    )


# ---- integration against the fixture package --------------------------------


@pytest.fixture
def sample(monkeypatch):
    monkeypatch.setattr(gen_mdx, "PACKAGE", "samplepkg")
    mod = griffe.load("samplepkg", search_paths=[str(FIXTURES)], resolve_aliases=True)
    assert isinstance(mod, griffe.Module)
    pages = gen_mdx.collect_pages(mod)
    return pages, gen_mdx.build_manifest(pages)


def _page(pages, name):
    return next(p for p in pages if p.name == name)


def test_collect_pages_routes_by_kind_and_respects_all(sample):
    pages, _ = sample
    dirs = {p.name: p.dir_path for p in pages}
    assert dirs["Thing"] == "api/classes"
    assert dirs["ListThing"] == "api/classes"
    assert dirs["Mode"] == "api/enumerations"
    assert dirs["do_it"] == "api/functions"
    assert "Base" not in dirs  # imported but not in __all__


def test_manifest_urls(sample):
    _, manifest = sample
    assert manifest["Thing"] == "/api/classes/thing/"
    assert manifest["Mode"] == "/api/enumerations/mode/"
    assert manifest["do_it"] == "/api/functions/do_it/"


def test_class_members_inherit_and_hide(sample):
    pages, manifest = sample
    out = gen_mdx.render_page(_page(pages, "Thing"), manifest)
    assert "### greet" in out  # own method
    assert "### shared" in out  # inherited from the in-package base
    assert "### name" in out  # annotated attribute -> property
    assert "init_self" not in out  # inherited helper excluded by name
    assert "secret" not in out  # @private docstring excluded
    assert "_internal" not in out  # underscore-prefixed excluded


def test_generic_title_and_docstring_links(sample):
    pages, manifest = sample
    list_out = gen_mdx.render_page(_page(pages, "ListThing"), manifest)
    assert 'title: "ListThing[T]"' in list_out  # PEP 695 type params in title
    thing_out = gen_mdx.render_page(_page(pages, "Thing"), manifest)
    assert '<XRef to="ListThing" />' in thing_out  # docstring symbol linked


def test_import_statement_top_level(sample):
    pages, manifest = sample
    # a class, a function and an enum all show `from <pkg> import <name>`
    thing_out = gen_mdx.render_page(_page(pages, "Thing"), manifest)
    assert "```python\nfrom samplepkg import Thing\n```" in thing_out
    do_it_out = gen_mdx.render_page(_page(pages, "do_it"), manifest)
    assert "```python\nfrom samplepkg import do_it\n```" in do_it_out
    mode_out = gen_mdx.render_page(_page(pages, "Mode"), manifest)
    assert "```python\nfrom samplepkg import Mode\n```" in mode_out


def test_import_statement_submodule(sample):
    pages, manifest = sample
    # a submodule member imports from the submodule, not the top-level package
    out = gen_mdx.render_page(_page(pages, "SubThing"), manifest)
    assert "```python\nfrom samplepkg.sub import SubThing\n```" in out


def test_bases_line(sample):
    pages, manifest = sample
    thing_out = gen_mdx.render_page(_page(pages, "Thing"), manifest)
    assert "**Bases:** Base" in thing_out  # undocumented base shown as plain text
    list_out = gen_mdx.render_page(_page(pages, "ListThing"), manifest)
    # documented base linked
    assert '**Bases:** <XRef to="samplepkg.models.Thing" plain />' in list_out
    point_out = gen_mdx.render_page(_page(pages, "Point"), manifest)
    assert "**Bases:**" not in point_out  # typing.NamedTuple base suppressed
