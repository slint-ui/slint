# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Hand-maintained type stubs for the native module; keep in sync with the
# pyo3 classes and functions when changing the Rust API surface.
# ruff: noqa: F401


import builtins
import datetime
import gettext
import os
import pathlib
import typing
from collections.abc import Buffer, Callable, Coroutine
from enum import Enum, auto
from typing import Any, Self

from . import language as language

class RgbColor:
    red: int
    green: int
    blue: int

class RgbaColor:
    red: int
    green: int
    blue: int
    alpha: int

class Color:
    red: int
    green: int
    blue: int
    alpha: int
    def __new__(
        cls,
        maybe_value: str | RgbaColor | RgbColor | dict[str, int] | None = None,
    ) -> Self: ...
    def brighter(self, factor: float) -> Color: ...
    def darker(self, factor: float) -> Color: ...
    def transparentize(self, factor: float) -> Color: ...
    def mix(self, other: Image, factor: float) -> Color: ...
    def with_alpha(self, alpha: float) -> Color: ...
    def __eq__(self, other: object) -> bool: ...

class Brush:
    color: Color
    def __new__(cls, maybe_value: Color | None) -> Self: ...
    def is_transparent(self) -> bool: ...
    def is_opaque(self) -> bool: ...
    def brighter(self, factor: float) -> Brush: ...
    def darker(self, factor: float) -> Brush: ...
    def transparentize(self, amount: float) -> Brush: ...
    def with_alpha(self, alpha: float) -> Brush: ...
    def __eq__(self, other: object) -> bool: ...

class Keys:
    r"""
    Represents a key binding created by the `@keys(...)` macro in Slint.

    This is an opaque type. Use `str()` to get a platform-native representation
    of the key binding (e.g. "Ctrl+A" on Linux/Windows, "⌘A" on macOS).
    """

    @staticmethod
    def from_parts(parts: list[str]) -> Keys: ...
    def __eq__(self, other: object) -> bool: ...

class DataTransfer:
    r"""
    Represents some form of type-indexed possibly-lazy data transfer.

    Used for accessing the platform clipboard and drag-and-drop APIs.
    """

    def __new__(cls) -> Self:
        r"""Constructs an empty `DataTransfer`."""

    plain_text: str | None
    r"""
    The plain text representation of this `DataTransfer`, or `None` if no plain text
    is available. Assigning `None` or the empty string clears any previously-set
    plain text; assigning any other string overwrites it.
    """

    has_plain_text: bool
    r"""`True` if this `DataTransfer` advertises a plain text representation."""

    image: Image | None
    r"""
    The image representation of this `DataTransfer`, or `None` if no image is
    available. Assigning `None` clears any previously-set image; assigning any
    other image overwrites it.
    """

    has_image: bool
    r"""`True` if this `DataTransfer` advertises an image representation."""

    is_empty: bool
    r"""
    `True` if this `DataTransfer` carries no data: no plain text, no image, and no
    user data.
    """

    user_data: object | None
    r"""
    Application-internal user data attached to this `DataTransfer`. Use this when the
    drag-and-drop or clipboard operation stays inside the current Python application and you
    want to avoid serializing to plain text or an image.

    Reading returns the Python object previously assigned, or `None` if none was set (or the
    user data was set by a non-Python binding). Assigning `None` clears any previously attached
    Python user data.
    """

    def __eq__(self, other: object) -> bool: ...

class StyledText:
    r"""
    Python wrapper for Slint's `styled-text` type.
    """

    def __new__(cls) -> Self: ...
    @staticmethod
    def from_plain_text(text: str) -> StyledText:
        r"""
        Creates styled text from plain text.
        """

    @staticmethod
    def from_markdown(markdown: str) -> StyledText:
        r"""
        Parses markdown and returns a StyledText object.

        Raises:
            ValueError: If the markdown contains unsupported syntax.
        """

    def __eq__(self, other: object) -> bool: ...

class LogicalPosition:
    r"""A 2D position in logical pixels."""

    x: float
    r"""The horizontal coordinate."""
    y: float
    r"""The vertical coordinate."""
    def __new__(cls, x: float = 0.0, y: float = 0.0) -> Self: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class LogicalSize:
    r"""A 2D size in logical pixels."""

    width: float
    r"""The width."""
    height: float
    r"""The height."""
    def __new__(cls, width: float = 0.0, height: float = 0.0) -> Self: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Image:
    r"""
    Image objects can be set on Slint Image elements for display. Construct Image objects from a path to an
    image file on disk, using `Image.load_from_path`.
    """

    size: tuple[int, int]
    width: int
    height: int
    path: pathlib.Path | None
    def __new__(
        cls,
    ) -> Self: ...
    @staticmethod
    def load_from_path(path: str | os.PathLike[Any] | pathlib.Path) -> Image:
        r"""
        Loads the image from the specified path. Returns None if the image can't be loaded.
        """

    @staticmethod
    def load_from_svg_data(data: typing.Sequence[int]) -> Image:
        r"""
        Creates a new image from a string that describes the image in SVG format.
        """

    @staticmethod
    def load_from_array(array: Buffer) -> Image:
        r"""
        Creates a new image from an array-like object that implements the [Buffer Protocol](https://docs.python.org/3/c-api/buffer.html).
        Use this function to import images created by third-party modules such as matplotlib or Pillow.

        The array must satisfy certain constraints to represent an image:

         - The buffer's format needs to be `B` (unsigned char)
         - The shape must be a tuple of (height, width, bytes-per-pixel)
         - If a stride is defined, the row stride must be equal to width * bytes-per-pixel, and the column stride must equal the bytes-per-pixel.
         - A value of 3 for bytes-per-pixel is interpreted as RGB image, a value of 4 means RGBA.

        Example of importing a matplot figure into an image:
        ```python
        import slint
        import matplotlib

        from matplotlib.backends.backend_agg import FigureCanvasAgg
        from matplotlib.figure import Figure

        fig = Figure(figsize=(5, 4), dpi=100)
        canvas = FigureCanvasAgg(fig)
        ax = fig.add_subplot()
        ax.plot([1, 2, 3])
        canvas.draw()

        buffer = canvas.buffer_rgba()
        img = slint.Image.load_from_array(buffer)
        ```

        Example of loading an image with Pillow:
        ```python
        import slint
        from PIL import Image
        import numpy as np

        pil_img = Image.open("hello.jpeg")
        array = np.array(pil_img)
        img = slint.Image.load_from_array(array)
        ```
        """

class TimerMode(Enum):
    SingleShot = auto()
    Repeated = auto()

class Timer:
    running: bool
    interval: datetime.timedelta
    def __new__(
        cls,
    ) -> Self: ...
    def start(
        self, mode: TimerMode, interval: datetime.timedelta, callback: typing.Any
    ) -> None: ...
    @staticmethod
    def single_shot(duration: datetime.timedelta, callback: typing.Any) -> None: ...
    def stop(self) -> None: ...
    def restart(self) -> None: ...

def set_xdg_app_id(app_id: str) -> None: ...
def invoke_from_event_loop(callable: typing.Callable[[], None]) -> None: ...
def run_event_loop() -> None: ...
def quit_event_loop() -> None: ...
def init_translations(
    translations: gettext.GNUTranslations | None,
) -> None: ...
def build_features() -> list[str]: ...

class PyModelBase:
    def init_self(self, *args: Any) -> None: ...
    def row_count(self) -> int: ...
    def row_data(self, row: int) -> Any | None: ...
    def set_row_data(self, row: int, value: Any) -> None: ...
    def append(self, value: Any) -> None: ...
    def remove_row(self, row: int) -> None: ...
    def insert_row(self, row: int, value: Any) -> None: ...
    def notify_row_changed(self, row: int) -> None: ...
    def notify_row_removed(self, row: int, count: int) -> None: ...
    def notify_row_added(self, row: int, count: int) -> None: ...

class PyStruct(Any): ...

class ValueType(Enum):
    Void = auto()
    Number = auto()
    String = auto()
    Bool = auto()
    Model = auto()
    Struct = auto()
    Brush = auto()
    Image = auto()
    StyledText = auto()
    Keys = auto()

class DiagnosticLevel(Enum):
    Error = auto()
    Warning = auto()
    Note = auto()

class PyDiagnostic:
    level: DiagnosticLevel
    message: str
    line_number: int
    column_number: int
    source_file: str | None

class ComponentInstance:
    def _process_pending_events(self) -> None: ...
    def show(self) -> None: ...
    def hide(self) -> None: ...
    def invoke(self, callback_name: str, *args: Any) -> Any: ...
    def invoke_global(
        self, global_name: str, callback_name: str, *args: Any
    ) -> Any: ...
    def set_property(self, property_name: str, value: Any) -> None: ...
    def get_property(self, property_name: str) -> Any: ...
    def set_callback(
        self, callback_name: str, callback: Callable[..., Any]
    ) -> None: ...
    def set_global_callback(
        self, global_name: str, callback_name: str, callback: Callable[..., Any]
    ) -> None: ...
    def set_global_property(
        self, global_name: str, property_name: str, value: Any
    ) -> None: ...
    def get_global_property(self, global_name: str, property_name: str) -> Any: ...

class ComponentDefinition:
    def create(self) -> ComponentInstance: ...
    name: str
    globals: list[str]
    functions: list[str]
    callbacks: list[str]
    properties: dict[str, ValueType]
    def global_functions(self, global_name: str) -> list[str]: ...
    def global_callbacks(self, global_name: str) -> list[str]: ...
    def global_properties(self, global_name: str) -> dict[str, ValueType]: ...
    def callback_returns_void(self, callback_name: str) -> bool: ...
    def global_callback_returns_void(
        self, global_name: str, callback_name: str
    ) -> bool: ...

class CompilationResult:
    component_names: list[str]
    diagnostics: list[PyDiagnostic]
    named_exports: list[tuple[str, str]]
    structs_and_enums: tuple[dict[str, PyStruct], dict[str, Enum]]
    generated_api: GeneratedAPI
    def component(self, name: str) -> ComponentDefinition: ...

class Compiler:
    include_paths: list[os.PathLike[Any] | pathlib.Path]
    library_paths: dict[str, os.PathLike[Any] | pathlib.Path]
    translation_domain: str
    style: str
    def build_from_path(
        self, path: os.PathLike[Any] | pathlib.Path
    ) -> CompilationResult: ...
    def build_from_source(
        self, source: str, path: os.PathLike[Any] | pathlib.Path
    ) -> CompilationResult: ...

class AsyncAdapter:
    def __new__(
        cls,
        fd: int,
    ) -> Self: ...
    def wait_for_readable(self, callback: typing.Callable[[int], None]) -> None: ...
    def wait_for_writable(self, callback: typing.Callable[[int], None]) -> None: ...

class GeneratedAPI:
    def __new__(
        cls, path: str | os.PathLike[Any] | pathlib.Path, json: str
    ) -> Self: ...
    @staticmethod
    def compare_generated_vs_actual(
        generated: GeneratedAPI, actual: GeneratedAPI
    ) -> None: ...
