#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
"""Emit JSON mapping each line in ``.cspell/slint-project-words.txt`` to file usage.

For every dictionary word, runs ripgrep with ``-F`` (fixed-string) matches. When
the token is an ASCII identifier (``[A-Za-z0-9_]+``), also passes
``--word-regexp`` so hits align with whole tokens and not substrings inside
larger identifiers.

The words list file and other dictionary artifacts are excluded from hits (see
``EXTRA_EXCLUDE_GLOBS``). ``cspell.json`` ``ignorePaths`` are applied the same
way as ``tools/cspell/prune_single_file_words.py``.

By default matching is **case-sensitive** (no ``-i``) to reduce false positives.
Pass ``--ignore-case`` to match CSpell's typical dictionary behavior.

Examples::

    python3 tools/cspell/project_words_usage_json.py
    python3 tools/cspell/project_words_usage_json.py -o .cspell/slint-project-words-usage.json
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CSPELL_JSON = REPO_ROOT / "cspell.json"
PROJECT_WORDS_TXT = REPO_ROOT / ".cspell" / "slint-project-words.txt"

ASCII_IDENTIFIER = re.compile(r"^[A-Za-z0-9_]+$")

EXTRA_EXCLUDE_GLOBS = [
    "cspell.json",
    ".cspell/slint-project-words.txt",
    ".cspell/slint-project-words-audit.tsv",
    ".cspell/single-file-prune-audit.tsv",
    ".cspell/slint-project-words-usage.json",
]


def strip_jsonc_comments(src: str) -> str:
    """Remove ``//`` and ``/* */`` comments outside JSON strings (double-quoted)."""
    out: list[str] = []
    i = 0
    n = len(src)
    in_string = False
    escape = False
    while i < n:
        c = src[i]
        if in_string:
            out.append(c)
            if escape:
                escape = False
            elif c == "\\":
                escape = True
            elif c == '"':
                in_string = False
            i += 1
            continue
        if c == '"':
            in_string = True
            out.append(c)
            i += 1
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            while i < n and src[i] != "\n":
                i += 1
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            i += 2
            while i + 1 < n and not (src[i] == "*" and src[i + 1] == "/"):
                i += 1
            i = min(i + 2, n)
            continue
        out.append(c)
        i += 1
    return "".join(out)


def load_cspell_ignore_paths(path: Path) -> list[str]:
    raw = path.read_text(encoding="utf-8")
    data = json.loads(strip_jsonc_comments(raw))
    return list(data.get("ignorePaths") or [])


def load_txt_words(path: Path) -> list[str]:
    if not path.is_file():
        return []
    out: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        w = line.strip()
        if w:
            out.append(w)
    return out


def ignore_path_to_rg_globs(pattern: str) -> list[str]:
    p = pattern.strip()
    if not p:
        return []
    if p == "LICENSES":
        return [f"!{p}", f"!{p}/**"]
    return [f"!{p}"]


def rg_files_for_word(
    word: str,
    repo: Path,
    exclude_globs: list[str],
    *,
    ignore_case: bool,
) -> list[str]:
    """Return sorted relative POSIX paths of files containing ``word``."""
    args = [
        "rg",
        "--files-with-matches",
        "--null",
        "--no-messages",
        "--hidden",
    ]
    if ignore_case:
        args.append("-i")
    for g in exclude_globs:
        args.extend(["--glob", g])
    if ASCII_IDENTIFIER.fullmatch(word):
        args.append("--word-regexp")
    args.extend(["-F", "--", word, str(repo)])
    try:
        proc = subprocess.run(
            args,
            capture_output=True,
            timeout=120,
            check=False,
        )
    except FileNotFoundError:
        print("error: ripgrep (rg) not found in PATH", file=sys.stderr)
        sys.exit(2)
    if proc.returncode not in (0, 1):
        err = proc.stderr.decode("utf-8", errors="replace")
        print(f"rg failed (exit {proc.returncode}): {err}", file=sys.stderr)
        sys.exit(2)
    if not proc.stdout:
        return []
    raw = proc.stdout.split(b"\0")
    paths: list[str] = []
    for b in raw:
        if not b:
            continue
        try:
            rel = Path(b.decode("utf-8")).resolve().relative_to(repo.resolve())
        except ValueError:
            continue
        paths.append(rel.as_posix())
    return sorted(set(paths))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-o",
        "--output",
        default=".cspell/slint-project-words-usage.json",
        help="JSON output path (default: .cspell/slint-project-words-usage.json)",
    )
    parser.add_argument(
        "--ignore-case",
        "-i",
        action="store_true",
        help="Pass ripgrep -i (case-insensitive); default is case-sensitive",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=48,
        help="Parallel rg invocations (default: 48)",
    )
    args = parser.parse_args()

    if not PROJECT_WORDS_TXT.is_file():
        print(f"error: missing {PROJECT_WORDS_TXT}", file=sys.stderr)
        sys.exit(2)

    words = load_txt_words(PROJECT_WORDS_TXT)
    if not words:
        print("error: no words loaded from project words file", file=sys.stderr)
        sys.exit(2)

    exclude_globs: list[str] = []
    for p in load_cspell_ignore_paths(CSPELL_JSON):
        exclude_globs.extend(ignore_path_to_rg_globs(p))
    for p in EXTRA_EXCLUDE_GLOBS:
        g = f"!{p}"
        if g not in exclude_globs:
            exclude_globs.append(g)

    results: dict[str, list[str]] = {}
    with ThreadPoolExecutor(max_workers=max(1, args.workers)) as ex:
        futs = {
            ex.submit(
                rg_files_for_word,
                w,
                REPO_ROOT,
                exclude_globs,
                ignore_case=args.ignore_case,
            ): w
            for w in words
        }
        for fut in as_completed(futs):
            w = futs[fut]
            results[w] = fut.result()

    payload: dict[str, object] = {
        "meta": {
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "word_file": PROJECT_WORDS_TXT.relative_to(REPO_ROOT).as_posix(),
            "word_count": len(words),
            "case_sensitive": not args.ignore_case,
            "ascii_identifier_word_regexp": True,
            "ripgrep_fixed_strings": True,
        },
        "words": {},
    }
    words_out: dict[str, dict[str, object]] = {}
    for w in words:
        files = results[w]
        words_out[w] = {"file_count": len(files), "files": files}
    payload["words"] = words_out

    out_path = Path(args.output)
    if not out_path.is_absolute():
        out_path = REPO_ROOT / out_path
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    try:
        print(f"Wrote {out_path.relative_to(REPO_ROOT)}", file=sys.stderr)
    except ValueError:
        print(f"Wrote {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
