# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Sample models for the gen_mdx tests."""

import typing

from .native import Base


class Thing(Base):
    """A thing. Use `ListThing` to hold several."""

    name: str
    """The thing's name."""

    def greet(self) -> str:
        """Return a greeting."""
        return "hi"

    def _internal(self) -> None: ...

    def secret(self) -> None:
        """@private"""


class ListThing[T](Thing):
    """A generic list of things."""

    def first(self) -> typing.Optional[T]:
        """Return the first item, if any."""
        return None


class Point(typing.NamedTuple):
    """A 2D point."""

    x: int
    """The horizontal coordinate."""
    y: int
    """The vertical coordinate."""
