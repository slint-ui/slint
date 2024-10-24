# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import slint as native
from slint import loader
import sys
import os


def test_magic_import():
    instance = loader.test_load_file.App()
    del instance


def test_magic_import_path():
    oldsyspath = sys.path
    assert loader.printerdemo == None
    try:
        sys.path.append(os.path.join(os.path.dirname(__file__),
                        "..", "..", ".."))
        instance = loader.demos.printerdemo.ui.printerdemo.MainWindow()
        del instance
    finally:
        sys.path = oldsyspath
