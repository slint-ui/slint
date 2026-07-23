# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Sample package exercising the gen_mdx selection and rendering logic."""

import enum

from . import sub  # documented submodule (mirrors slint.language)
from .models import ListThing, Point, Thing
from .native import Base  # noqa: F401  # imported but not in __all__ -> not documented


class Mode(enum.Enum):
    """A mode."""

    A = "a"
    """The A mode."""
    B = "b"
    """The B mode."""


def do_it(count: int) -> None:
    """Do it `count` times; see `Thing`."""


__all__ = ["ListThing", "Mode", "Point", "Thing", "do_it", "sub"]
