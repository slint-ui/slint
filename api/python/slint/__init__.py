# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

from . import slint as native
import types
import logging


class CompileError(Exception):
    def __init__(self, message, diagnostics):
        self.message = message
        self.diagnostics = diagnostics


def load_file(path, quiet=False, style=None, include_paths=None, library_paths=None, translation_domain=None):
    compiler = native.ComponentCompiler()

    if style is not None:
        compiler.style = style
    if include_paths is not None:
        compiler.include_paths = include_paths
    if library_paths is not None:
        compiler.library_paths = library_paths
    if translation_domain is not None:
        compiler.translation_domain = translation_domain

    compdef = compiler.build_from_path(path)

    diagnostics = compiler.diagnostics
    if diagnostics:
        if not quiet:
            for diag in diagnostics:
                if diag.level == native.DiagnosticLevel.Warning:
                    logging.warning(diag)

            errors = [diag for diag in diagnostics if diag.level ==
                      native.DiagnosticLevel.Error]
            if errors:
                raise CompileError(f"Could not compile {path}", diagnostics)

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
