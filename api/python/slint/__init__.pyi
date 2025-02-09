# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# This file is automatically generated by pyo3_stub_gen
# ruff: noqa: E501, F401

import builtins
import datetime
import os
import pathlib
import typing
from typing import Self
from enum import Enum, auto


class RgbColor:
    red: builtins.int
    green: builtins.int
    blue: builtins.int

class RgbaColor:
    red: builtins.int
    green: builtins.int
    blue: builtins.int
    alpha: builtins.int


class Color:
    red: builtins.int
    green: builtins.int
    blue: builtins.int
    alpha: builtins.int
    def __new__(cls,maybe_value:typing.Optional[builtins.str | RgbaColor | RgbColor]): ...
    def brighter(self, factor:builtins.float) -> Self:
        ...

    def darker(self, factor:builtins.float) -> Self:
        ...

    def transparentize(self, factor:builtins.float) -> Self:
        ...

    def mix(self, other:Self, factor:builtins.float) -> Self:
        ...

    def with_alpha(self, alpha:builtins.float) -> Self:
        ...

    def __str__(self) -> builtins.str:
        ...

    def __eq__(self, other:Self) -> builtins.bool:
        ...



class Brush:
    color: Color
    def __new__(cls,maybe_value:typing.Optional[Color]): ...
    def is_transparent(self) -> builtins.bool:
        ...

    def is_opaque(self) -> builtins.bool:
        ...

    def brighter(self, factor:builtins.float) -> Self:
        ...

    def darker(self, factor:builtins.float) -> Self:
        ...

    def transparentize(self, amount:builtins.float) -> Self:
        ...

    def with_alpha(self, alpha:builtins.float) -> Self:
        ...

    def __eq__(self, other:Self) -> builtins.bool:
        ...


class Image:
    r"""
    Image objects can be set on Slint Image elements for display. Construct Image objects from a path to an
    image file on disk, using `Image.load_from_path`.
    """
    size: tuple[builtins.int, builtins.int]
    width: builtins.int
    height: builtins.int
    path: typing.Optional[builtins.str]
    def __new__(cls,): ...
    @staticmethod
    def load_from_path(path:builtins.str | os.PathLike | pathlib.Path) -> Self:
        r"""
        Loads the image from the specified path. Returns None if the image can't be loaded.
        """
        ...

    @staticmethod
    def load_from_svg_data(data:typing.Sequence[builtins.int]) -> Self:
        r"""
        Creates a new image from a string that describes the image in SVG format.
        """
        ...


class TimerMode(Enum):
    SingleShot = auto()
    Repeated = auto()


class Timer:
    def __new__(cls,): ...
    def start(self, mode:TimerMode, interval:datetime.timedelta, callback:typing.Any) -> None:
        ...

    @staticmethod
    def single_shot(duration:datetime.timedelta, callback:typing.Any) -> None:
        ...

    def stop(self) -> None:
        ...

    def restart(self) -> None:
        ...

    def running(self) -> builtins.bool:
        ...

    def set_interval(self, interval:datetime.timedelta) -> None:
        ...



