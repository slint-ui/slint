# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell: ignore binascii hexlify tobytes unraisable unraisablehook

import binascii
import gc
import os
import subprocess
import sys
import threading
from pathlib import Path

import numpy as np
import pytest

import slint
from slint import slint as native

SOURCE = Path(__file__).parent / "test-window.slint"

SNAPSHOT_SCRIPT = """
import sys

import numpy as np
import slint

app = slint.load_file(sys.argv[1]).App()
app.show()
snapshot = app.window.take_snapshot()
assert isinstance(snapshot, slint.Image)
assert snapshot.size == (20, 10)
pixels = np.asarray(snapshot)
assert pixels.shape == (10, 20, 4)
assert pixels[0, 0].tolist() == [0, 0, 255, 255]
assert pixels[9, 19].tolist() == [255, 0, 0, 255]
"""


@pytest.mark.skipif(
    "backend-testing" not in native.build_features(),
    reason="needs the testing backend for headless rendering",
)
def test_take_snapshot() -> None:
    # The backend is chosen once per process, and the one the tests run with can't render.
    env = {**os.environ, "SLINT_BACKEND": "testing-software"}
    subprocess.run(
        [sys.executable, "-c", SNAPSHOT_SCRIPT, str(SOURCE)], env=env, check=True
    )


@pytest.mark.skipif(
    "backend-testing" not in native.build_features(),
    reason="needs the testing backend",
)
def test_unknown_testing_renderer() -> None:
    env = {**os.environ, "SLINT_BACKEND": "testing-nonexistent"}
    script = "import sys, slint; slint.load_file(sys.argv[1]).App().show()"
    result = subprocess.run(
        [sys.executable, "-c", script, str(SOURCE)],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert "Unknown headless renderer" in result.stderr


def test_image_buffer() -> None:
    array = np.zeros((2, 3, 4), dtype=np.uint8)
    array[1, 2] = [10, 20, 30, 255]
    image = slint.Image.load_from_array(array)

    view = memoryview(image)
    assert view.readonly
    assert view.format == "B"
    assert view.shape == (2, 3, 4)
    assert view.strides == (12, 4, 1)
    assert np.array_equal(np.asarray(image), array)
    assert bytes(image) == array.tobytes()
    # `hexlify` asks for a buffer without a shape.
    assert binascii.hexlify(image) == binascii.hexlify(array.tobytes())

    rgb = slint.Image.load_from_array(np.full((1, 1, 3), 7, dtype=np.uint8))
    assert np.asarray(rgb).tolist() == [[[7, 7, 7, 255]]]


def test_image_buffer_outlives_image() -> None:
    array = np.full((2, 2, 4), 42, dtype=np.uint8)
    view = memoryview(slint.Image.load_from_array(array))
    assert bytes(view) == array.tobytes()


def test_image_buffers_share_memory() -> None:
    array = np.full((2, 2, 3), 42, dtype=np.uint8)
    image = slint.Image.load_from_array(array)
    assert np.shares_memory(np.asarray(image), np.asarray(image))


def test_empty_image_buffer() -> None:
    assert np.asarray(slint.Image()).shape == (0, 0, 4)


def test_image_buffer_released_on_other_thread() -> None:
    unraisable = []
    old_hook = sys.unraisablehook
    sys.unraisablehook = unraisable.append
    try:
        views = [memoryview(slint.Image.load_from_array(np.zeros((1, 1, 4), np.uint8)))]
        thread = threading.Thread(target=views.clear)
        thread.start()
        thread.join()
    finally:
        sys.unraisablehook = old_hook
    assert unraisable == []


def test_window_identity() -> None:
    app = slint.load_file(SOURCE).App()
    assert app.window is app.window


def test_window_attribute_can_be_assigned() -> None:
    app = slint.load_file(SOURCE).App()
    app.window = "mine"
    assert app.window == "mine"


def test_window_attribute_of_subclass() -> None:
    class App(slint.load_file(SOURCE).App):  # type: ignore[misc, name-defined]
        window = "mine"

    assert App().window == "mine"


def test_window_attribute_set_before_init() -> None:
    class App(slint.load_file(SOURCE).App):  # type: ignore[misc, name-defined]
        def __init__(self) -> None:
            self.window = "mine"
            super().__init__()

    assert App().window == "mine"


def test_window_property_in_slint() -> None:
    app = slint.load_file(SOURCE).WindowProperty()
    assert app.window == "from slint"
    app.window = "changed"
    assert app.window == "changed"


def test_window_without_component() -> None:
    app = slint.load_file(SOURCE).App()
    window = app.window
    del app
    gc.collect()
    with pytest.raises(RuntimeError, match="no longer exists"):
        window.take_snapshot()
