# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import gc
import weakref
from pathlib import Path

import slint
from slint import DataTransfer
from slint import slint as native


def test_default_is_empty() -> None:
    dt = DataTransfer()
    assert dt.has_plain_text is False
    assert dt.has_image is False
    assert dt.plain_text is None
    assert dt.image is None
    assert dt.user_data is None
    assert dt.is_empty is True


def test_plain_text_round_trip() -> None:
    dt = DataTransfer()
    dt.plain_text = "Hello, World!"
    assert dt.has_plain_text is True
    assert dt.plain_text == "Hello, World!"
    assert dt.is_empty is False


def test_is_empty_after_image() -> None:
    svg = b'<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>'
    image = slint.Image.load_from_svg_data(list(svg))
    dt = DataTransfer()
    dt.image = image
    assert dt.is_empty is False


def test_is_empty_after_user_data() -> None:
    dt = DataTransfer()
    dt.user_data = {"k": 1}
    assert dt.is_empty is False


def test_plain_text_assignment_overwrites() -> None:
    dt = DataTransfer()
    dt.plain_text = "first"
    dt.plain_text = "second"
    assert dt.plain_text == "second"


def test_image_round_trip() -> None:
    svg = b'<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>'
    image = slint.Image.load_from_svg_data(list(svg))
    dt = DataTransfer()
    dt.image = image
    assert dt.has_image is True
    fetched = dt.image
    assert fetched is not None
    assert fetched.width == image.width
    assert fetched.height == image.height


def test_assigning_empty_string_clears_plain_text() -> None:
    dt = DataTransfer()
    dt.plain_text = "hello"
    dt.plain_text = ""
    assert dt.has_plain_text is False
    assert dt.plain_text is None
    assert dt.is_empty is True


def test_assigning_none_clears_plain_text() -> None:
    dt = DataTransfer()
    dt.plain_text = "hello"
    dt.plain_text = None
    assert dt.has_plain_text is False
    assert dt.plain_text is None


def test_assigning_none_clears_image() -> None:
    svg = b'<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>'
    image = slint.Image.load_from_svg_data(list(svg))
    dt = DataTransfer()
    dt.image = image
    dt.image = None
    assert dt.has_image is False
    assert dt.image is None


def test_user_data_round_trip_dict() -> None:
    dt = DataTransfer()
    payload = {"key": "value", "n": 42}
    dt.user_data = payload
    fetched = dt.user_data
    assert fetched == payload
    # Same object, not a copy.
    assert fetched is payload


def test_user_data_round_trip_custom_class() -> None:
    class Marker:
        def __init__(self, n: int) -> None:
            self.n = n

    dt = DataTransfer()
    marker = Marker(5)
    dt.user_data = marker
    fetched = dt.user_data
    assert isinstance(fetched, Marker)
    assert fetched is marker


def test_user_data_overwrites() -> None:
    dt = DataTransfer()
    dt.user_data = "first"
    dt.user_data = "second"
    assert dt.user_data == "second"


def test_user_data_assign_none_clears() -> None:
    dt = DataTransfer()
    dt.user_data = {"k": 1}
    assert dt.user_data is not None
    dt.user_data = None
    assert dt.user_data is None


def test_plain_text_and_user_data_coexist() -> None:
    dt = DataTransfer()
    dt.plain_text = "hello"
    dt.user_data = {"k": 1}
    assert dt.has_plain_text is True
    assert dt.plain_text == "hello"
    assert dt.user_data == {"k": 1}


def test_equality() -> None:
    a = DataTransfer()
    assert a == a  # noqa: PLR0124 -- deliberately checks reflexive equality
    # Modifying one of two independently-constructed transfers makes them
    # unequal — equality is identity-based on the inner content, so two transfers
    # holding distinct payloads are different.
    b = DataTransfer()
    b.plain_text = "payload"
    assert a != b


def test_repr() -> None:
    dt = DataTransfer()
    dt.plain_text = "hi"
    text = repr(dt)
    assert text.startswith("DataTransfer(")


def test_user_data_cycle_is_collectable() -> None:
    # The Rust side stores the Python user-data object behind `Rc<dyn Any>`,
    # invisible to Python's GC. Without `__traverse__`/`__clear__` on
    # DataTransfer, a cycle through `user_data` would never be collected.
    class Holder:
        dt: DataTransfer | None = None

    dt = DataTransfer()
    holder = Holder()
    holder.dt = dt
    dt.user_data = holder

    weak_holder = weakref.ref(holder)

    del dt, holder
    gc.collect()

    assert weak_holder() is None


def test_callback_round_trip() -> None:
    """The Slint engine carries DataTransfer values through callbacks."""

    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export global Api {
            pure callback identity(data-transfer) -> data-transfer;
            pure callback set_plain(string) -> data-transfer;
            pure callback get_plain(data-transfer) -> string;
        }
        export component App {}
        """,
        Path(""),
    ).component("App")
    assert compdef is not None
    instance = compdef.create()
    assert instance is not None

    instance.set_global_callback("Api", "identity", lambda dt: dt)

    def make(text: str) -> DataTransfer:
        out = DataTransfer()
        out.plain_text = text
        return out

    instance.set_global_callback("Api", "set_plain", make)
    instance.set_global_callback("Api", "get_plain", lambda dt: dt.plain_text or "")

    source = DataTransfer()
    source.plain_text = "payload"
    echoed = instance.invoke_global("Api", "identity", source)
    assert isinstance(echoed, DataTransfer)
    assert echoed.plain_text == "payload"

    built = instance.invoke_global("Api", "set_plain", "constructed")
    assert isinstance(built, DataTransfer)
    assert built.plain_text == "constructed"

    assert instance.invoke_global("Api", "get_plain", built) == "constructed"
