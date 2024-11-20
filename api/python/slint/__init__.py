# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from importlib.machinery import ModuleSpec
import os
import sys
from . import slint as native
import types
import logging
import importlib
import copy
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

        def mk_setter_getter(prop_name):
            def getter(self):
                return self.__instance__.get_global_property(
                    global_name, prop_name)

            def setter(self, value):
                return self.__instance__.set_global_property(
                    global_name, prop_name, value)

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(prop_name)

    for callback_name in compdef.global_callbacks(global_name):
        python_prop = _normalize_prop(callback_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(callback_name):
            def getter(self):
                def call(*args):
                    return self.__instance__.invoke_global(global_name, callback_name, *args)
                return call

            def setter(self, value):
                return self.__instance__.set_global_callback(
                    global_name, callback_name, value)

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(callback_name)

    for function_name in compdef.global_functions(global_name):
        python_prop = _normalize_prop(function_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated function {prop_name}")
            continue

        def mk_getter(function_name):
            def getter(self):
                def call(*args):
                    return self.__instance__.invoke_global(global_name, function_name, *args)
                return call

            return property(getter)

        properties_and_callbacks[python_prop] = mk_getter(function_name)

    return type("SlintGlobalClassWrapper", (), properties_and_callbacks)


def _build_class(compdef):

    def cls_init(self, **kwargs):
        self.__instance__ = compdef.create()
        for name, value in self.__class__.__dict__.items():
            if hasattr(value, "slint.callback"):
                callback_info = getattr(value, "slint.callback")
                name = callback_info["name"]

                def mk_callback(self, callback):
                    def invoke(*args, **kwargs):
                        return callback(self, *args, **kwargs)
                    return invoke

                if "global_name" in callback_info:
                    self.__instance__.set_global_callback(
                        callback_info["global_name"], name, mk_callback(self, value))
                else:
                    self.__instance__.set_callback(
                        name, mk_callback(self, value))

        for prop, val in kwargs.items():
            setattr(self, prop, val)

    properties_and_callbacks = {
        "__init__": cls_init
    }

    for prop_name in compdef.properties.keys():
        python_prop = _normalize_prop(prop_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(prop_name):
            def getter(self):
                return self.__instance__.get_property(prop_name)

            def setter(self, value):
                return self.__instance__.set_property(
                    prop_name, value)

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(prop_name)

    for callback_name in compdef.callbacks:
        python_prop = _normalize_prop(callback_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(callback_name):
            def getter(self):
                def call(*args):
                    return self.__instance__.invoke(callback_name, *args)
                return call

            def setter(self, value):
                return self.__instance__.set_callback(
                    callback_name, value)

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(callback_name)

    for function_name in compdef.functions:
        python_prop = _normalize_prop(function_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated function {prop_name}")
            continue

        def mk_getter(function_name):
            def getter(self):
                def call(*args):
                    return self.__instance__.invoke(function_name, *args)
                return call

            return property(getter)

        properties_and_callbacks[python_prop] = mk_getter(function_name)

    for global_name in compdef.globals:
        global_class = _build_global_class(compdef, global_name)

        def mk_global(global_class):
            def global_getter(self):
                wrapper = global_class()
                setattr(wrapper, "__instance__", self.__instance__)
                return wrapper

            return property(global_getter)

        properties_and_callbacks[global_name] = mk_global(global_class)

    return type("SlintClassWrapper", (Component,), properties_and_callbacks)


def _build_struct(name, struct_prototype):

    def new_struct(cls, *args, **kwargs):
        inst = copy.copy(struct_prototype)

        for prop, val in kwargs.items():
            setattr(inst, prop, val)

        return inst

    type_dict = {
        "__new__": new_struct,
    }

    return type(name, (), type_dict)


def load_file(path, quiet=False, style=None, include_paths=None, library_paths=None, translation_domain=None):
    compiler = native.Compiler()

    if style is not None:
        compiler.style = style
    if include_paths is not None:
        compiler.include_paths = include_paths
    if library_paths is not None:
        compiler.library_paths = library_paths
    if translation_domain is not None:
        compiler.translation_domain = translation_domain

    result = compiler.build_from_path(path)

    diagnostics = result.diagnostics
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
    for comp_name in result.component_names:
        wrapper_class = _build_class(result.component(comp_name))

        setattr(module, comp_name, wrapper_class)

    for name, struct_or_enum_prototype in result.structs_and_enums.items():
        name = _normalize_prop(name)
        struct_wrapper = _build_struct(name, struct_or_enum_prototype)
        setattr(module, name, struct_wrapper)

    for orig_name, new_name in result.named_exports:
        orig_name = _normalize_prop(orig_name)
        new_name = _normalize_prop(new_name)
        setattr(module, new_name, getattr(module, orig_name))

    return module


class SlintAutoLoader:
    def __init__(self, base_dir=None):
        if base_dir:
            self.local_dirs = [base_dir]
        else:
            self.local_dirs = None

    def __getattr__(self, name):
        for path in self.local_dirs or sys.path:
            dir_candidate = os.path.join(path, name)
            if os.path.isdir(dir_candidate):
                loader = SlintAutoLoader(dir_candidate)
                setattr(self, name, loader)
                return loader

            file_candidate = dir_candidate + ".slint"
            if os.path.isfile(file_candidate):
                type_namespace = load_file(file_candidate)
                setattr(self, name, type_namespace)
                return type_namespace

            dir_candidate = os.path.join(path, name.replace('_', '-'))
            file_candidate = dir_candidate + ".slint"
            if os.path.isfile(file_candidate):
                type_namespace = load_file(file_candidate)
                setattr(self, name, type_namespace)
                return type_namespace

        return None


loader = SlintAutoLoader()


def _callback_decorator(callable, info):
    if "name" not in info:
        info["name"] = callable.__name__
    setattr(callable, "slint.callback", info)
    return callable


def callback(global_name=None, name=None):
    if callable(global_name):
        callback = global_name
        return _callback_decorator(callback, {})
    else:
        info = {}
        if name:
            info["name"] = name
        if global_name:
            info["global_name"] = global_name
        return lambda callback: _callback_decorator(callback, info)


Image = native.PyImage
Color = native.PyColor
Brush = native.PyBrush
Model = native.PyModelBase
ListModel = models.ListModel
Model = models.Model
Timer = native.Timer
TimerMode = native.TimerMode
Struct = native.PyStruct

def set_xdg_app_id(app_id: str):
    native.set_xdg_app_id(app_id)