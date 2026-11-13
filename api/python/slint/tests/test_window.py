# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell: ignore tobytes

import os
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

import slint
from slint import slint as native

SOURCE = Path(__file__).parent / "test-window.slint"

SNAPSHOT_SCRIPT = """
import sys
from pathlib import Path

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

    rgb = slint.Image.load_from_array(np.full((1, 1, 3), 7, dtype=np.uint8))
    assert np.asarray(rgb).tolist() == [[[7, 7, 7, 255]]]


def test_image_buffer_outlives_image() -> None:
    array = np.full((2, 2, 4), 42, dtype=np.uint8)
    view = memoryview(slint.Image.load_from_array(array))
    assert bytes(view) == array.tobytes()
