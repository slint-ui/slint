# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from typing import Any, Callable

import slint

__all__ = ["CounterWindow"]

class CounterWindow(slint.Component):
    def __init__(self, **kwargs: Any) -> None: ...
    counter: int
    request_increase: Callable[[], None]
