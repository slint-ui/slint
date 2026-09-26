# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import html
import re

import pytest

from syntax import highlight, spans


@pytest.mark.parametrize(
    "source",
    [
        '@pytest.mark.parametrize("size", [1, 2])\ndef test_size(size):\n    assert size > 0\n',
        'class Example:\n\tvalue = "é 😀 <font color=red> & **bold** `code`"\n\n',
        '# comment\nx = """first\n    second\n\nlast"""\n',
        "def broken(:\n    value = (42\n",
        "x = 1\n  x = 2\n x = 3\n",
        "\n\n\t\n",
        'text = "unicode\u2028separator"\nnumber = 42\n',
        "",
        'x = f"hello {len(items):02d}"',
    ],
)
def test_markup_preserves_exact_source(source):
    for dark in (False, True):
        markup = highlight(source, dark)
        plain = html.unescape(re.sub(r"</?font[^>]*>", "", markup))
        assert plain == source
        assert not re.search(r"<(?!/?font\b)", markup)


def test_token_categories_and_unicode_offsets():
    source = '@pytest.mark.slow\nclass Café:\n    def check(self):\n        # note\n        return len("é") + 42\n'
    tokens = [(source[start:end], kind) for start, end, kind in spans(source)]
    for pair in [
        ("pytest", "decorator"),
        ("class", "keyword"),
        ("Café", "definition"),
        ("check", "definition"),
        ("# note", "comment"),
        ("len", "builtin"),
        ('"é"', "string"),
        ("42", "number"),
    ]:
        assert pair in tokens
    assert highlight(source, False) != highlight(source, True)
