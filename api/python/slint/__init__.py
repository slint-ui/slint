# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

from . import slint as native


def load_file(path):
    compiler = native.ComponentCompiler()
    compdef = compiler.build_from_path(path)
    instance = compdef.create()
    return instance


Image = native.PyImage
Color = native.PyColor
Brush = native.PyBrush
