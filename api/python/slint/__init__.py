# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

from . import slint as native
import types
import logging
from . import models


class CompileError(Exception):
    def __init__(self, message, diagnostics):
        self.message = message
        self.diagnostics = diagnostics


class Component:
    def show(self):
        self.__instance__.show()

    def hide(self):
        self.__instance__.hide()

    def run(self):
        self.__instance__.run()


def _normalize_prop(name):
    return name.replace("-", "_")


def _build_global_class(compdef, global_name):
    properties_and_callbacks = {}

    for prop_name in compdef.global_properties(global_name).keys():
        python_prop = _normalize_prop(prop_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def getter(self): return self.__instance__.get_global_property(
            global_name, prop_name)

        def setter(self, value): return self.__instance__.set_global_property(
            global_name, prop_name, value)

        properties_and_callbacks[python_prop] = property(getter, setter)

    for callback_name in compdef.global_callbacks(global_name):
        python_prop = _normalize_prop(callback_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def getter(self):
            def call(*args):
                return self.__instance__.invoke_global(global_name, callback_name, *args)
            return call

        def setter(self, value): return self.__instance__.set_global_callback(
            global_name, callback_name, value)

        properties_and_callbacks[python_prop] = property(getter, setter)

    return type("SlintGlobalClassWrapper", (), properties_and_callbacks)


def _build_class(compdef):

    def cls_init(self, **kwargs):
        self.__instance__ = compdef.create()

    properties_and_callbacks = {
        "__init__": cls_init
    }

    for prop_name in compdef.properties.keys():
        python_prop = _normalize_prop(prop_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def getter(self): return self.__instance__.get_property(prop_name)

        def setter(self, value): return self.__instance__.set_property(
            prop_name, value)

        properties_and_callbacks[python_prop] = property(getter, setter)

    for callback_name in compdef.callbacks:
        python_prop = _normalize_prop(callback_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def getter(self):
            def call(*args):
                return self.__instance__.invoke(callback_name, *args)
            return call

        def setter(self, value): return self.__instance__.set_callback(
            callback_name, value)

        properties_and_callbacks[python_prop] = property(getter, setter)

    for global_name in compdef.globals:
        global_class = _build_global_class(compdef, global_name)

        def global_getter(self):
            wrapper = global_class()
            setattr(wrapper, "__instance__", self.__instance__)
            return wrapper
        properties_and_callbacks[global_name] = property(global_getter)

    return type("SlintClassWrapper", (Component,), properties_and_callbacks)


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

    wrapper_class = _build_class(compdef)

    module = types.SimpleNamespace()
    setattr(module, compdef.name, wrapper_class)

    return module


Image = native.PyImage
Color = native.PyColor
Brush = native.PyBrush
Model = native.PyModelBase
ListModel = models.ListModel
Model = models.Model
