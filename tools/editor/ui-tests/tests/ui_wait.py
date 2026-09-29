# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import math
import time
from dataclasses import dataclass


@dataclass(frozen=True)
class Deadline:
    end: float
    timeout: float

    @classmethod
    def after(cls, timeout: float) -> Deadline:
        if not math.isfinite(timeout) or timeout < 0:
            raise ValueError("timeout must be finite and nonnegative")
        return cls(time.monotonic() + timeout, timeout)

    @property
    def expired(self) -> bool:
        return time.monotonic() >= self.end

    def pause(self, interval: float = 0.02) -> None:
        remaining = self.end - time.monotonic()
        if remaining > 0:
            time.sleep(min(interval, remaining))
