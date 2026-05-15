#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
"""Aggregate ``cspell --no-progress`` lines of the form ``path:line:col - Unknown word (token)``.

Groups by ``token.casefold()`` so case variants merge (CSpell is case-insensitive for
dictionary checks). By default, selects tokens that appear in **at least two distinct
files** (same casefold group may repeat many times in one file; that still counts as one
file). Optional ``--min-occurrences`` tightens the filter on total issue lines.

Examples::

    python3 tools/cspell/aggregate_spellcheck_unknowns.py \\
        .cspell/spellcheck-empty-project-words.log \\
        -o .cspell/spellcheck-unknowns-aggregated.json

    # Legacy: total issue lines >= 2 (same file repeats count)
    python3 tools/cspell/aggregate_spellcheck_unknowns.py log.txt \\
        --min-distinct-files 1 --min-occurrences 2
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

# cspell-cli: "path/to/file:12:34 - Unknown word (someword)"
ISSUE_RE = re.compile(r"^(.+?):(\d+):(\d+)\s+-\s+Unknown word \((.+)\)\s*$")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path, help="Captured stdout/stderr from pnpm spellcheck / cspell")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path(".cspell/spellcheck-unknowns-aggregated.json"),
        help="JSON output path (default: .cspell/spellcheck-unknowns-aggregated.json)",
    )
    parser.add_argument(
        "--min-distinct-files",
        type=int,
        default=2,
        help="Include a casefold token only if it appears in at least this many distinct paths (default: 2)",
    )
    parser.add_argument(
        "--min-occurrences",
        type=int,
        default=1,
        help="Also require at least this many total issue lines for that token (default: 1)",
    )
    parser.add_argument(
        "--include-single-file-keys",
        action="store_true",
        help="Also emit sorted casefold keys that matched only one distinct file (can be large)",
    )
    args = parser.parse_args()

    log_path = args.log
    if not log_path.is_file():
        print(f"error: log not found: {log_path}", file=sys.stderr)
        sys.exit(2)

    if args.min_distinct_files < 1:
        print("error: --min-distinct-files must be >= 1", file=sys.stderr)
        sys.exit(2)
    if args.min_occurrences < 1:
        print("error: --min-occurrences must be >= 1", file=sys.stderr)
        sys.exit(2)

    # casefold -> stats
    surfaces: dict[str, set[str]] = defaultdict(set)
    file_sets: dict[str, set[str]] = defaultdict(set)
    positions: dict[str, list[tuple[str, int, int, str]]] = defaultdict(list)
    parsed = 0
    skipped = 0

    for raw in log_path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = ISSUE_RE.match(raw)
        if not m:
            skipped += 1
            continue
        rel_path, line_s, _col_s, word = m.groups()
        key = word.casefold()
        surfaces[key].add(word)
        file_sets[key].add(rel_path)
        positions[key].append((rel_path, int(line_s), int(_col_s), word))
        parsed += 1

    occurrences: dict[str, int] = {k: len(v) for k, v in positions.items()}

    def passes(k: str) -> bool:
        return len(file_sets[k]) >= args.min_distinct_files and occurrences[k] >= args.min_occurrences

    selected = {k for k in occurrences if passes(k)}
    single_file_only = sorted(
        k for k in occurrences if len(file_sets[k]) == 1 and k not in selected
    )

    selected_payload: dict[str, dict[str, object]] = {}
    for k in sorted(selected, key=lambda x: (-len(file_sets[x]), -occurrences[x], x)):
        selected_payload[k] = {
            "occurrence_count": occurrences[k],
            "distinct_files": len(file_sets[k]),
            "surface_forms": sorted(surfaces[k], key=str.casefold),
            "files": sorted(file_sets[k]),
        }

    payload: dict[str, object] = {
        "meta": {
            "source_log": str(log_path),
            "inclusion": {
                "group_by": "casefold",
                "min_distinct_files": args.min_distinct_files,
                "min_occurrences": args.min_occurrences,
            },
            "lines_matched": parsed,
            "lines_skipped_non_issue": skipped,
            "unique_casefold_tokens": len(occurrences),
            "occurrence_total": sum(occurrences.values()),
            "tokens_selected": len(selected),
            "tokens_not_selected": len(occurrences) - len(selected),
            "tokens_single_distinct_file_only": len(single_file_only),
        },
        "selected_casefold": selected_payload,
    }
    if args.include_single_file_keys:
        payload["single_distinct_file_casefold_keys"] = single_file_only

    out = args.output
    if not out.is_absolute():
        out = Path.cwd() / out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Wrote {out}", file=sys.stderr)
    print(
        f"parsed={parsed} unique_cf={len(occurrences)} selected={len(selected)} "
        f"single_file_only={len(single_file_only)}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
