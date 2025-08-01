# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint
import numpy as np
from PIL import Image
from pathlib import Path


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


def test_image_loading() -> None:
    image = Image.open(
        base_dir() / ".." / ".." / ".." / ".." / "logo" / "slint-logo-simple-dark.png"
    )
    assert image.size == (282, 84)
    array = np.array(image)
    slint_image = slint.Image.load_from_array(array)
    assert slint_image.width == 282
    assert slint_image.height == 84
