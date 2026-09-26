# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import html
import re

import pytest

from inspection_view import failure_location, property_view, source_view
from slint_syntax import assignment_line, code_lines, tokens

SOURCE = """export component Example inherits Window {
    width: 800px;
    root-rectangle := Rectangle {
        // width: 999px;
        child := Rectangle { width: 12px; }
        width: 200px;
        text: "width: 300px; }";
    }
}"""


@pytest.mark.parametrize("dark", [True, False])
def test_slint_grammar_coloring_and_exact_text(dark):
    source = (
        SOURCE
        + '\n/* outer /* nested */ done */\nbackground: #f0a;\ntext: "Hello \\{root.width}px";\nin-out property <length> café: 2.5cm;'
    )
    painted = code_lines(source, dark)
    assert (
        "\n".join(html.unescape(re.sub(r"</?font[^>]*>", "", line)) for line in painted)
        == source
    )
    lexed = [(source[a:b], kind) for a, b, kind in tokens(source)]
    for token in [
        ("export", "keyword"),
        ("Window", "definition"),
        ("in-out", "keyword"),
        ("2.5cm", "number"),
        ("#f0a", "number"),
        ("length", "builtin"),
        ("café", "identifier"),
        ("/* outer /* nested */ done */", "comment"),
    ]:
        assert token in lexed
    assert code_lines(source, True) != code_lines(source, False)


def test_assignment_resolves_exact_element_and_direct_property():
    assert assignment_line(SOURCE, "root-rectangle", "width") == 6
    assert assignment_line(SOURCE, "child", "width") == 5
    assert assignment_line(SOURCE, "missing", "width") == 0
    assert assignment_line(SOURCE, "root-rectangle", "height") == 0
    assert assignment_line(SOURCE + SOURCE, "root-rectangle", "width") == 0
    assert (
        assignment_line(
            SOURCE.replace("width: 200px;", "width: 200px; width: 300px;"),
            "root-rectangle",
            "width",
        )
        == 0
    )
    assert (
        assignment_line(SOURCE.replace("    }\n}", ""), "root-rectangle", "width") == 0
    )


def test_property_highlight_requires_exact_handle_and_property():
    element = {
        "handle": {"index": 12, "generation": 3},
        "value": "200",
        "name": "Width",
        "locator": "window.get_by_id('width')",
    }
    location = {"handle": element["handle"], "property": "accessible_value"}
    text, line = property_view(element, location)
    assert text.splitlines()[line - 1] == '  "value": "200",'
    assert property_view(element, {})[1] == 0
    assert (
        property_view(element, {**location, "handle": {"index": 12, "generation": 4}})[
            1
        ]
        == 0
    )
    assert property_view(element, {**location, "property": "unknown"})[1] == 0
    assert (
        failure_location(
            {"action": {"diagnostic": {"observed": False, "location": location}}}
        )
        == {}
    )


def test_source_highlight_requires_matching_complete_document():
    doc = {"name": "Main.slint", "path": "/fixture/Main.slint", "text": SOURCE}
    location = {
        "source": {
            "path": doc["path"],
            "element": "root-rectangle",
            "property": "width",
        }
    }
    assert source_view({"sources": [doc]}, location) == (
        SOURCE,
        "Main.slint · captured at pause",
        6,
    )
    assert source_view({"sources": [dict(doc, truncated=True)]}, location)[2] == 0
    assert (
        source_view({"sources": [dict(doc, path="/other/Main.slint")]}, location)[2]
        == 0
    )
    assert source_view({"source_text": "legacy source"}, location) == (
        "legacy source",
        "Saved source",
        0,
    )
