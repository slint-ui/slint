# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

from . import slint as native
import types


def load_file(path):
    compiler = native.ComponentCompiler()
    compdef = compiler.build_from_path(path)

    module = types.SimpleNamespace()
    setattr(module, compdef.name, type("SlintMetaClass", (), {
        "__compdef": compdef,
        "__new__": lambda cls: cls.__compdef.create()
    }))

    return module


Image = native.PyImage
Color = native.PyColor
Brush = native.PyBrush
Model = native.PyModelBase
