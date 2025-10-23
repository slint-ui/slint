from __future__ import annotations

from pathlib import Path

from slint.codegen.utils import normalize_identifier, path_literal


def test_normalize_identifier() -> None:
    assert normalize_identifier("foo-bar") == "foo_bar"
    assert normalize_identifier("1value") == "_1value"


def test_path_literal() -> None:
    path = Path("/tmp/demo")
    assert path_literal(path) == repr(str(path))
