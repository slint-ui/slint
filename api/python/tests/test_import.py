# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import pytest
from slint import slint as native
import sys
import os


def test_magic_import():
    import test_load_file_slint as compiledmodule
    instance = compiledmodule.App()
    del instance


def test_magic_import_path():
    oldsyspath = sys.path
    with pytest.raises(ModuleNotFoundError, match="No module named 'printerdemo_slint'"):
        import printerdemo_slint
    try:
        sys.path.append(os.path.join(os.path.dirname(__file__),
                        "..", "..", "..", "examples", "printerdemo", "ui"))
        import printerdemo_slint
        instance = printerdemo_slint.MainWindow()
        del instance
    finally:
        sys.path = oldsyspath
