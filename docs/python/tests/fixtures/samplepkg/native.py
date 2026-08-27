# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Native-style base, simulating a class that other modules re-export."""


class Base:
    def shared(self) -> int:
        """A method inherited by subclasses."""
        return 0

    def init_self(self) -> None:
        """@private"""

    def _hidden(self) -> None: ...
