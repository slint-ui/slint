# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Display lexer based on internal/compiler/lexer.rs and tree-sitter-slint's grammar."""

import re
from functools import lru_cache

from syntax import PALETTES, literal

KEYWORDS = {
    "export",
    "import",
    "from",
    "as",
    "component",
    "inherits",
    "global",
    "struct",
    "enum",
    "interface",
    "property",
    "callback",
    "function",
    "pure",
    "public",
    "private",
    "in",
    "out",
    "in-out",
    "if",
    "else",
    "for",
    "return",
    "animate",
    "states",
    "transitions",
    "when",
    "changed",
    "true",
    "false",
    "self",
    "root",
    "parent",
    "children",
    "slot",
}
TYPES = {
    "int",
    "float",
    "bool",
    "string",
    "color",
    "brush",
    "image",
    "length",
    "physical-length",
    "duration",
    "angle",
    "percent",
    "easing",
    "model",
    "void",
}
NUMBER = re.compile(r"[0-9]+(?:\.[0-9]*)?(?:%|[A-Za-z]+)?")
COLOR = re.compile(r"#[A-Za-z0-9]*")


def tokens(source):
    cursor = 0
    interpolation = []
    while cursor < len(source):
        start = cursor
        char = source[cursor]
        kind = "plain"
        if source.startswith("//", cursor):
            cursor = source.find("\n", cursor)
            cursor = len(source) if cursor < 0 else cursor
            kind = "comment"
        elif source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            kind = "comment"
        elif char == '"' or (char == "}" and interpolation and interpolation[-1] == 0):
            if char == "}":
                interpolation.pop()
            cursor += 1
            while cursor < len(source):
                if source.startswith("\\{", cursor):
                    interpolation.append(0)
                    cursor += 2
                    break
                if source[cursor] == '"':
                    cursor += 1
                    break
                cursor += 2 if source[cursor] == "\\" else 1
            cursor = min(cursor, len(source))
            kind = "string"
        elif char.isascii() and char.isdigit():
            match = NUMBER.match(source, cursor)
            assert match is not None
            cursor = match.end()
            kind = "number"
        elif char == "#":
            match = COLOR.match(source, cursor)
            assert match is not None
            cursor = match.end()
            kind = "number"
        elif char.isidentifier() or char == "@":
            cursor += 1
            while cursor < len(source) and (
                source[cursor] == "-" or ("_" + source[cursor]).isidentifier()
            ):
                cursor += 1
            word = source[start:cursor]
            kind = (
                "keyword"
                if word in KEYWORDS
                else "builtin"
                if word in TYPES or word.startswith("@")
                else "definition"
                if word[:1].isupper()
                else "identifier"
            )
        else:
            if interpolation and char == "{":
                interpolation[-1] += 1
            elif interpolation and char == "}":
                interpolation[-1] -= 1
            cursor += 2 if source.startswith((":=", "<=>", "=>"), cursor) else 1
            if source.startswith("<=>", start):
                cursor = start + 3
        yield start, cursor, kind


@lru_cache(maxsize=64)
def code_lines(source, dark=False, language="slint"):
    lines = [""]
    palette = PALETTES[dark]
    stream = tokens(source) if language == "slint" else [(0, len(source), "plain")]
    for start, end, kind in stream:
        pieces = source[start:end].split("\n")
        for index, piece in enumerate(pieces):
            if index:
                lines.append("")
            escaped = literal(piece)
            lines[-1] += (
                f'<font color="{palette[kind]}">{escaped}</font>'
                if kind in palette
                else escaped
            )
    return tuple(lines)


def assignment_line(source, element, property_name):
    """Return a unique direct binding's one-based line, or zero when unresolved."""
    lexical = [
        (source[a:b], a)
        for a, b, kind in tokens(source)
        if kind not in ("comment", "string") and source[a:b].strip()
    ]
    definitions = []
    for index in range(len(lexical) - 3):
        if lexical[index][0] == element and lexical[index + 1][0] == ":=":
            opening = index + 2
            while opening < len(lexical) and (
                lexical[opening][0] == "."
                or lexical[opening][0].replace("-", "_").isidentifier()
            ):
                opening += 1
            if opening < len(lexical) and lexical[opening][0] == "{":
                definitions.append(opening)
    if len(definitions) != 1:
        return 0
    depth, lines = 1, []
    for index in range(definitions[0] + 1, len(lexical)):
        value, offset = lexical[index]
        if (
            depth == 1
            and value == property_name
            and lexical[index - 1][0] in ("{", ";", "}")
            and index + 1 < len(lexical)
            and lexical[index + 1][0] == ":"
        ):
            lines.append(source.count("\n", 0, offset) + 1)
        depth += (value == "{") - (value == "}")
        if depth == 0:
            return lines[0] if len(lines) == 1 else 0
    return 0
