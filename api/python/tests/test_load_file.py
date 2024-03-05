# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

from slint import load_file
import os


def test_load_file():
    module = load_file(os.path.join(os.path.dirname(
        __spec__.origin), "test_load_file.slint"))
    assert list(module.__dict__.keys()) == ["App"]
    instance = module.App()
    del instance
