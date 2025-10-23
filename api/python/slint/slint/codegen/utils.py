# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
