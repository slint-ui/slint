# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import argparse
from pathlib import Path

from .generator import generate_project
from .models import GenerationConfig


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="slint.codegen",
        description=("Generate Python source and stub files from Slint .slint inputs."),
    )

    subparsers = parser.add_subparsers(dest="command", required=False)

    gen_parser = subparsers.add_parser(
        "generate",
        help="Generate Python modules for the provided .slint inputs.",
    )
    _add_generate_arguments(gen_parser)

    # Allow invoking the root command without specifying the subcommand by
    # mirroring the generate options onto the root parser. This keeps the CLI
    # ergonomic (`python -m slint.codegen --input ...`).
    _add_generate_arguments(parser)

    return parser


def _add_generate_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--input",
        "-i",
        dest="inputs",
        action="append",
        type=Path,
        required=False,
        help=(
            "Path to a .slint file or directory containing .slint files. "
            "May be supplied multiple times. Defaults to the current working "
            "directory if omitted."
        ),
    )
    parser.add_argument(
        "--output",
        "-o",
        dest="output",
        type=Path,
        default=None,
        help=(
            "Directory that will receive the generated Python sources. "
            "When omitted, files are generated next to each input .slint file."
        ),
    )
    parser.add_argument(
        "--include",
        dest="include_paths",
        action="append",
        type=Path,
        default=None,
        help=(
            "Additional include paths to pass to the Slint compiler. "
            "May be provided multiple times."
        ),
    )
    parser.add_argument(
        "--library",
        dest="library_paths",
        action="append",
        default=None,
        metavar="NAME=PATH",
        help=(
            "Library import mapping passed to the Slint compiler in the form "
            "@mylib=path/to/lib."
        ),
    )
    parser.add_argument(
        "--style",
        dest="style",
        default=None,
        help="Widget style to apply when compiling (for example, 'material').",
    )
    parser.add_argument(
        "--translation-domain",
        dest="translation_domain",
        default=None,
        help="Translation domain to embed into generated modules.",
    )
    parser.add_argument(
        "--quiet",
        dest="quiet",
        action="store_true",
        help="Suppress compiler warnings during generation.",
    )


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    # The CLI accepts either the explicit "generate" subcommand or the root
    # invocation. Determine which parser branch was taken.
    command = getattr(args, "command", None)
    if command not in (None, "generate"):
        parser.error(f"Unknown command: {command}")

    inputs: list[Path] = args.inputs or [Path.cwd()]
    config = GenerationConfig(
        include_paths=args.include_paths or [],
        library_paths=_parse_library_paths(args.library_paths or []),
        style=args.style,
        translation_domain=args.translation_domain,
        quiet=bool(args.quiet),
    )

    generate_project(inputs=inputs, output_dir=args.output, config=config)
    return 0


def _parse_library_paths(values: list[str]) -> dict[str, Path]:
    mapping: dict[str, Path] = {}
    for raw in values:
        if "=" not in raw:
            raise SystemExit(
                f"Library mapping '{raw}' must be provided in the form NAME=PATH"
            )
        name, path_str = raw.split("=", maxsplit=1)
        name = name.strip()
        if not name:
            raise SystemExit("Library mapping requires a non-empty name before '='")
        path = Path(path_str.strip())
        mapping[name] = path
    return mapping


__all__ = ["main", "build_parser"]
