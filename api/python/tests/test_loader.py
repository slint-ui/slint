# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import loader
import sys
import os


def test_magic_import() -> None:
    instance = loader.test_load_file.App()
    del instance


def test_magic_import_path() -> None:
    oldsyspath = sys.path
    assert loader.printerdemo is None
    try:
        sys.path.append(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
        instance = loader.demos.printerdemo.ui.printerdemo.MainWindow()
        del instance
    finally:
        sys.path = oldsyspath
