# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import sys

from .cli import main

if __name__ == "__main__":  # pragma: no cover - CLI entry point
    sys.exit(main())
