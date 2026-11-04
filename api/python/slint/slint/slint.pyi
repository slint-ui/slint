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
    r"""
    A Color object represents a color in the RGB color space with an alpha. Each color channel and the alpha are
    represented as an 8-bit integer. The alpha channel is:

    - `0`: fully transparent
    - `255`: fully opaque

    You can construct colors from a dict with the red, green, blue, and optional alpha channels.
    You can also construct them from a color string as a `.slint` file writes it, such as `"#ff000080"` or `"blue"`.
    """

    red: int
    r"""The red channel."""
    green: int
    r"""The green channel."""
    blue: int
    r"""The blue channel."""
    alpha: int
    r"""The alpha channel."""
    def __new__(
        cls,
        maybe_value: str | RgbaColor | RgbColor | dict[str, int] | None = None,
    ) -> Self: ...
    def brighter(self, factor: float) -> Color:
        r"""Returns a new color that is brighter than this color by the given factor."""

    def darker(self, factor: float) -> Color:
        r"""Returns a new color that is darker than this color by the given factor."""

    def transparentize(self, factor: float) -> Color:
        r"""
        Returns a new version of this color with the opacity decreased by `factor`.

        The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
        """

    def mix(self, other: Color, factor: float) -> Color:
        r"""
        Returns a new color that is a mix of this color and `other`. The specified factor is
        clamped to be between `0.0` and `1.0` and then applied to this color, while `1.0 - factor`
        is applied to `other`.
        """

    def with_alpha(self, alpha: float) -> Color:
        r"""Returns a new version of this color with the opacity set to `alpha`."""

    def __eq__(self, other: object) -> bool: ...

class Brush:
    r"""
    A brush describes how a shape, such as a rectangle, path, or even text, is filled. You can also apply a brush
    to the outline of a shape, which fills the outline itself.

    You can only construct brushes from solid colors.

    **Note:** In the future, we plan to lift this constraint and let you declare gradient brushes programmatically.
    """

    color: Color
    r"""The brush's color."""
    def __new__(cls, maybe_value: Color | None) -> Self: ...
    def is_transparent(self) -> bool:
        r"""Returns `True` if this brush contains a fully transparent color (alpha value is `0`)."""

    def is_opaque(self) -> bool:
        r"""Returns `True` if this brush is fully opaque."""

    def brighter(self, factor: float) -> Brush:
        r"""
        Returns a new version of this brush that has the brightness increased
        by the specified factor. This is done by calling `Color.brighter` on
        all the colors of this brush.
        """

    def darker(self, factor: float) -> Brush:
        r"""
        Returns a new version of this brush that has the brightness decreased
        by the specified factor. This is done by calling `Color.darker` on
        all the colors of this brush.
        """

    def transparentize(self, amount: float) -> Brush:
        r"""
        Returns a new version of this brush with the opacity decreased by `amount`.

        The transparency is obtained by multiplying the alpha channel by `(1 - amount)`.

        See also `Color.transparentize`.
        """

    def with_alpha(self, alpha: float) -> Brush:
        r"""
        Returns a new version of this brush with the related color's opacities
        set to `alpha`.
        """

    def __eq__(self, other: object) -> bool: ...

class Keys:
    r"""
    Represents a key binding created by the `@keys(...)` macro in Slint.

    This is an opaque type. Use `str()` to get a platform-native representation
    of the key binding, such as `Ctrl+A` on Linux or Windows, and `⌘A` on macOS.
    """

    @staticmethod
    def from_parts(parts: list[str]) -> Keys:
        r"""
        Creates a `Keys` from a list of string parts, such as `["Control", "Shift?", "Z"]`.

        Each element is either a modifier name or a key name. Raises `ValueError` if the parts can't be parsed.
        """

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

    def __new__(cls) -> Self:
        r"""Creates empty styled text."""

    @staticmethod
    def from_plain_text(text: str) -> StyledText:
        r"""
        Creates styled text from plain text.
        """

    @staticmethod
    def from_markdown(markdown: str) -> StyledText:
        r"""
        Parses Markdown and returns a StyledText object.

        Raises:
            ValueError: If the Markdown contains unsupported syntax.
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
    r"""The size of the image as a tuple of `width` and `height`."""
    width: int
    r"""The width of the image in pixels."""
    height: int
    r"""The height of the image in pixels."""
    path: pathlib.Path | None
    r"""The path of the image if it was loaded from disk, or `None`."""
    def __new__(
        cls,
    ) -> Self: ...
    @staticmethod
    def load_from_path(path: str | os.PathLike[Any] | pathlib.Path) -> Image:
        r"""
        Loads the image from the specified path. Raises `RuntimeError` if the image can't be loaded.
        """

    @staticmethod
    def load_from_svg_data(data: typing.Sequence[int]) -> Image:
        r"""
        Creates a new image from bytes that describe the image in SVG format.
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
         - A value of 3 for bytes-per-pixel is interpreted as an RGB image, a value of 4 means RGBA.

        The image is created by performing a deep copy of the array's data. Subsequent changes to the buffer aren't
        reflected in a previously created Image.

        Example of importing a Matplotlib figure into an image:
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
    r"""
    The TimerMode specifies what happens after the timer fires.

    Used by the `Timer.start()` function.
    """

    SingleShot = auto()
    r"""A SingleShot timer is fired only once."""
    Repeated = auto()
    r"""A Repeated timer is fired repeatedly until it's stopped or dropped."""

class Timer:
    r"""
    Timer is a handle to the timer system that triggers a callback after a specified
    period of time.

    Use `Timer.start()` to create a timer that repeatedly triggers a callback, or
    `Timer.single_shot()` to trigger a callback only once.

    The timer stops automatically when it's garbage collected. You must keep the Timer object
    around for as long as you want the timer to keep firing.

    ```python
    class AppWindow(...):
        def __init__(self):
            super().__init__()
            self.my_timer = None

        @slint.callback
        def button_clicked(self):
            self.my_timer = slint.Timer()
            self.my_timer.start(slint.TimerMode.Repeated, timedelta(seconds=1), self.do_something)

        def do_something(self):
            pass
    ```

    **Note:** Timers only work in the thread that runs the Slint event loop. They don't
    fire if you use them in another thread.
    """

    running: bool
    r"""`True` if the timer is running; `False` otherwise."""
    interval: datetime.timedelta
    r"""
    The duration of the timer.

    If you set this property while the timer is running (see `Timer.running`),
    the time when the callback is next invoked is recalculated to be in the
    specified duration relative to when you set the property.
    """
    def __new__(
        cls,
    ) -> Self: ...
    def start(
        self, mode: TimerMode, interval: datetime.timedelta, callback: typing.Any
    ) -> None:
        r"""
        Starts the timer with the given mode and interval, to call the callback when the
        timer fires. If the timer has been started previously and hasn't fired yet, it's restarted.

        Arguments:
        * `mode`: The timer mode to apply, that is, whether to fire the timer repeatedly or just once.
        * `interval`: The duration from now until the timer fires the first time, and subsequently
           for `TimerMode.Repeated` timers.
        * `callback`: The function to call when the time has been reached or exceeded.
        """

    @staticmethod
    def single_shot(duration: datetime.timedelta, callback: typing.Any) -> None:
        r"""
        Starts a timer with the duration and the callback to call when the
        timer fires. It fires only once and is then deleted.

        Arguments:
        * `duration`: The duration from now until the timer fires.
        * `callback`: The function to call when the time has been reached or exceeded.
        """

    def stop(self) -> None:
        r"""Stops the previously started timer. Does nothing if the timer has never been started."""

    def restart(self) -> None:
        r"""
        Restarts the timer. If the timer was previously started by calling `Timer.start()`
        with a duration and callback, then the time when the callback is next invoked
        is recalculated to be in the specified duration relative to when you call this function.

        Does nothing if the timer was never started.
        """

def set_xdg_app_id(app_id: str) -> None: ...
def invoke_from_event_loop(callable: typing.Callable[[], None]) -> None: ...
def run_event_loop() -> None: ...
def quit_event_loop() -> None: ...
def init_translations(
    translations: gettext.GNUTranslations | None,
) -> None: ...
def build_features() -> list[str]: ...

class PyModelBase:
    def notify_row_changed(self, row: int) -> None: ...
    def notify_row_removed(self, row: int, count: int) -> None: ...
    def notify_row_added(self, row: int, count: int) -> None: ...

class PyModelAdapter:
    @staticmethod
    def map(
        source: typing.Any,
        map_function: typing.Callable[[typing.Any], typing.Any],
        target: PyModelBase,
    ) -> PyModelAdapter: ...
    @staticmethod
    def reverse(source: typing.Any, target: PyModelBase) -> PyModelAdapter: ...
    @staticmethod
    def sort(
        source: typing.Any,
        key: typing.Callable[[typing.Any], typing.Any] | None,
        reverse: bool,
        target: PyModelBase,
    ) -> PyModelAdapter: ...
    @staticmethod
    def filter(
        source: typing.Any,
        filter_function: typing.Callable[[typing.Any], bool],
        target: PyModelBase,
    ) -> PyModelAdapter: ...
    def row_count(self) -> int: ...
    def row_data(self, row: int) -> typing.Any: ...
    def set_row_data(self, row: int, data: typing.Any) -> None: ...
    def reset(self) -> None: ...
    def source_row(self, row: int) -> int: ...

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
    Info = auto()
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
