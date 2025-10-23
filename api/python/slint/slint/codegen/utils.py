from __future__ import annotations

from pathlib import Path


def normalize_identifier(identifier: str) -> str:
    identifier = identifier.replace("-", "_")
    if identifier[0].isdigit():
        identifier = f"_{identifier}"
    return identifier


def path_literal(value: str | Path) -> str:
    return repr(str(value))


__all__ = ["normalize_identifier", "path_literal"]
