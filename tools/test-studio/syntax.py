# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import builtins
import io
import keyword
import token
import tokenize
from functools import lru_cache

PALETTES = {
    False: {
        "keyword": "#cf222e",
        "string": "#0a3069",
        "number": "#0550ae",
        "comment": "#57606a",
        "definition": "#8250df",
        "builtin": "#953800",
        "decorator": "#8250df",
    },
    True: {
        "keyword": "#ff7b72",
        "string": "#a5d6ff",
        "number": "#79c0ff",
        "comment": "#9198a1",
        "definition": "#d2a8ff",
        "builtin": "#ffa657",
        "decorator": "#d2a8ff",
    },
}


def spans(source):
    offsets = [0]
    for line in io.StringIO(source):
        offsets.append(offsets[-1] + len(line))
    definition = False
    decorator = False
    try:
        for item in tokenize.generate_tokens(io.StringIO(source).readline):
            kind = None
            if item.type == token.NAME:
                if keyword.iskeyword(item.string):
                    kind = "keyword"
                elif definition:
                    kind = "definition"
                elif decorator:
                    kind = "decorator"
                elif item.string in vars(builtins):
                    kind = "builtin"
                definition = item.string in ("def", "class")
            elif item.type == token.NUMBER:
                kind = "number"
            elif item.type == token.COMMENT:
                kind = "comment"
            elif item.type == token.STRING or token.tok_name[item.type].startswith(
                ("FSTRING_", "TSTRING_")
            ):
                kind = "string"
            elif item.type == token.OP and item.string == "@":
                decorator = (
                    item.start[1] == 0
                    or not source[
                        offsets[item.start[0] - 1] : offsets[item.start[0] - 1]
                        + item.start[1]
                    ].strip()
                )
                kind = "decorator" if decorator else None
            elif item.type in (token.NEWLINE, token.NL):
                decorator = False
            if kind and item.string:
                start = offsets[item.start[0] - 1] + item.start[1]
                end = offsets[item.end[0] - 1] + item.end[1]
                yield start, end, kind
    except (tokenize.TokenError, IndentationError, SyntaxError):
        # Collection errors can leave incomplete source available for inspection.
        return


def literal(text):
    # Numeric entities keep Markdown from interpreting Python syntax or whitespace.
    return "".join(f"&#{ord(c)};" for c in text)


@lru_cache(maxsize=64)
def highlight(source, dark=False):
    palette = PALETTES[dark]
    chunks = []
    cursor = 0
    for start, end, kind in spans(source):
        chunks.append(literal(source[cursor:start]))
        chunks.append(
            f'<font color="{palette[kind]}">{literal(source[start:end])}</font>'
        )
        cursor = end
    chunks.append(literal(source[cursor:]))
    return "".join(chunks)
