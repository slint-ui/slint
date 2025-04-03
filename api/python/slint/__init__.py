# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

r"""
.. include:: ../README.md
"""

import os
import sys
from . import slint as native
import types
import logging
import copy
import typing
from typing import Any
import pathlib
from .models import ListModel, Model
from .slint import Image, Color, Brush, Timer, TimerMode
from pathlib import Path

Struct = native.PyStruct


class CompileError(Exception):
    message: str
    """The error message that produced this compile error."""

    diagnostics: list[native.PyDiagnostic]
    """A list of detailed diagnostics that were produced as part of the compilation."""

    def __init__(self, message: str, diagnostics: list[native.PyDiagnostic]):
        """@private"""
        self.message = message
        self.diagnostics = diagnostics


class Component:
    """Component is the base class for all instances of Slint components. Use the member functions to show or hide the
    window, or spin the event loop."""

    __instance__: native.ComponentInstance

    def show(self) -> None:
        """Shows the window on the screen."""

        self.__instance__.show()

    def hide(self) -> None:
        """Hides the window from the screen."""

        self.__instance__.hide()

    def run(self) -> None:
        """Shows the window, runs the event loop, hides it when the loop is quit, and returns."""
        self.__instance__.run()


def _normalize_prop(name: str) -> str:
    return name.replace("-", "_")


def _build_global_class(compdef: native.ComponentDefinition, global_name: str) -> Any:
    properties_and_callbacks = {}

    for prop_name in compdef.global_properties(global_name).keys():
        python_prop = _normalize_prop(prop_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(prop_or_callback_name: str) -> property:
            def getter(self: Component) -> Any:
                return self.__instance__.get_global_property(
                    global_name, prop_or_callback_name
                )

            def setter(self: Component, value: Any) -> None:
                self.__instance__.set_global_property(
                    global_name, prop_or_callback_name, value
                )

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(prop_name)

    for callback_name in compdef.global_callbacks(global_name):
        python_prop = _normalize_prop(callback_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(prop_or_callback_name: str) -> property:
            def getter(self: Component) -> typing.Callable[..., Any]:
                def call(*args: Any) -> Any:
                    return self.__instance__.invoke_global(
                        global_name, prop_or_callback_name, *args
                    )

                return call

            def setter(self: Component, value: typing.Callable[..., Any]) -> None:
                self.__instance__.set_global_callback(
                    global_name, prop_or_callback_name, value
                )

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(callback_name)

    for function_name in compdef.global_functions(global_name):
        python_prop = _normalize_prop(function_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated function {prop_name}")
            continue

        def mk_getter(function_name: str) -> property:
            def getter(self: Component) -> typing.Callable[..., Any]:
                def call(*args: Any) -> Any:
                    return self.__instance__.invoke_global(
                        global_name, function_name, *args
                    )

                return call

            return property(getter)

        properties_and_callbacks[python_prop] = mk_getter(function_name)

    return type("SlintGlobalClassWrapper", (), properties_and_callbacks)


def _build_class(
    compdef: native.ComponentDefinition,
) -> typing.Callable[..., Component]:
    def cls_init(self: Component, **kwargs: Any) -> Any:
        self.__instance__ = compdef.create()
        for name, value in self.__class__.__dict__.items():
            if hasattr(value, "slint.callback"):
                callback_info = getattr(value, "slint.callback")
                name = callback_info["name"]

                def mk_callback(
                    self: Any, callback: typing.Callable[..., Any]
                ) -> typing.Callable[..., Any]:
                    def invoke(*args: Any, **kwargs: Any) -> Any:
                        return callback(self, *args, **kwargs)

                    return invoke

                if "global_name" in callback_info:
                    self.__instance__.set_global_callback(
                        callback_info["global_name"], name, mk_callback(self, value)
                    )
                else:
                    self.__instance__.set_callback(name, mk_callback(self, value))

        for prop, val in kwargs.items():
            setattr(self, prop, val)

    properties_and_callbacks: dict[Any, Any] = {"__init__": cls_init}

    for prop_name in compdef.properties.keys():
        python_prop = _normalize_prop(prop_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(prop_or_callback_name: str) -> property:
            def getter(self: Component) -> Any:
                return self.__instance__.get_property(prop_or_callback_name)

            def setter(self: Component, value: Any) -> None:
                self.__instance__.set_property(prop_or_callback_name, value)

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(prop_name)

    for callback_name in compdef.callbacks:
        python_prop = _normalize_prop(callback_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated property {prop_name}")
            continue

        def mk_setter_getter(prop_or_callback_name: str) -> property:
            def getter(self: Component) -> typing.Callable[..., Any]:
                def call(*args: Any) -> Any:
                    return self.__instance__.invoke(prop_or_callback_name, *args)

                return call

            def setter(self: Component, value: typing.Callable[..., Any]) -> None:
                self.__instance__.set_callback(prop_or_callback_name, value)

            return property(getter, setter)

        properties_and_callbacks[python_prop] = mk_setter_getter(callback_name)

    for function_name in compdef.functions:
        python_prop = _normalize_prop(function_name)
        if python_prop in properties_and_callbacks:
            logging.warning(f"Duplicated function {prop_name}")
            continue

        def mk_getter(function_name: str) -> property:
            def getter(self: Component) -> typing.Callable[..., Any]:
                def call(*args: Any) -> Any:
                    return self.__instance__.invoke(function_name, *args)

                return call

            return property(getter)

        properties_and_callbacks[python_prop] = mk_getter(function_name)

    for global_name in compdef.globals:
        global_class = _build_global_class(compdef, global_name)

        def mk_global(global_class: typing.Callable[..., Any]) -> property:
            def global_getter(self: Component) -> Any:
                wrapper = global_class()
                setattr(wrapper, "__instance__", self.__instance__)
                return wrapper

            return property(global_getter)

        properties_and_callbacks[global_name] = mk_global(global_class)

    return type("SlintClassWrapper", (Component,), properties_and_callbacks)


def _build_struct(name: str, struct_prototype: native.PyStruct) -> type:
    def new_struct(cls: Any, *args: Any, **kwargs: Any) -> native.PyStruct:
        inst = copy.copy(struct_prototype)

        for prop, val in kwargs.items():
            setattr(inst, prop, val)

        return inst

    type_dict = {
        "__new__": new_struct,
    }

    return type(name, (), type_dict)


def load_file(
    path: str | os.PathLike[Any] | pathlib.Path,
    quiet: bool = False,
    style: typing.Optional[str] = None,
    include_paths: typing.Optional[typing.List[os.PathLike[Any] | pathlib.Path]] = None,
    library_paths: typing.Optional[
        typing.Dict[str, os.PathLike[Any] | pathlib.Path]
    ] = None,
    translation_domain: typing.Optional[str] = None,
) -> types.SimpleNamespace:
    """This function is the low-level entry point into Slint for Python. Loads the `.slint` file at the specified `path`
    and returns a namespace with all exported components as Python classes, as well as enums and structs.

    * `quiet`: Set to true to prevent any warnings during compilation to be printed to stderr.
    * `style`: Set this to use a specific a widget style.
    * `include_paths`: Additional include paths that will be used to look up `.slint` files imported from other `.slint` files.
    * `library_paths`: A dictionary that maps library names to their location in the file system. This is used to look up library imports,
       such as `import { MyButton } from "@mylibrary";`.
    * `translation_domain`: The domain to use for looking up the catalogue run-time translations. This must match the translation domain
       used when extracting translations with `slint-tr-extractor`.

    """

    compiler = native.Compiler()

    if style is not None:
        compiler.style = style
    if include_paths is not None:
        compiler.include_paths = include_paths
    if library_paths is not None:
        compiler.library_paths = library_paths
    if translation_domain is not None:
        compiler.translation_domain = translation_domain

    result = compiler.build_from_path(Path(path))

    diagnostics = result.diagnostics
    if diagnostics:
        if not quiet:
            for diag in diagnostics:
                if diag.level == native.DiagnosticLevel.Warning:
                    logging.warning(diag)

        errors = [
            diag for diag in diagnostics if diag.level == native.DiagnosticLevel.Error
        ]
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
    def __init__(self, base_dir: Path | None = None):
        self.local_dirs: typing.List[Path] | None = None
        if base_dir:
            self.local_dirs = [base_dir]

    def __getattr__(self, name: str) -> Any:
        for path in self.local_dirs or sys.path:
            dir_candidate = Path(path) / name
            if os.path.isdir(dir_candidate):
                loader = SlintAutoLoader(dir_candidate)
                setattr(self, name, loader)
                return loader

            file_candidate = dir_candidate.with_suffix(".slint")
            if os.path.isfile(file_candidate):
                type_namespace = load_file(file_candidate)
                setattr(self, name, type_namespace)
                return type_namespace

            dir_candidate = Path(path) / name.replace("_", "-")
            file_candidate = dir_candidate.with_suffix(".slint")
            if os.path.isfile(file_candidate):
                type_namespace = load_file(file_candidate)
                setattr(self, name, type_namespace)
                return type_namespace

        return None


loader = SlintAutoLoader()
"""The `loader` object is a global object that can be used to load Slint files from the file system. It exposes two stages of attributes:
1. Any lookup of an attribute in the loader will try to match a file in `sys.path` with the `.slint` extension. For example `loader.my_component` will look for a file `my_component.slint` in the directories in `sys.path`.
2. Any lookup in the object returned by the first stage will try to match an exported component in the loaded file, or a struct or enum. For example `loader.my_component.MyComponent` will look for an *exported* component named `MyComponent` in the file `my_component.slint`.

Note that the first entry in the module search path `sys.path` is the directory that contains the input script.

Example:
```python
import slint
# Look for a file `main.slint` in the current directory,
# #load & compile it, and instantiate the exported `MainWindow` component
main_window = slint.loader.main_window.MainWindow()
main_window.show()
...
```
"""


def _callback_decorator(
    callable: typing.Callable[..., Any], info: typing.Dict[str, Any]
) -> typing.Callable[..., Any]:
    if "name" not in info:
        info["name"] = callable.__name__
    setattr(callable, "slint.callback", info)
    return callable


def callback(
    global_name: str | None = None, name: str | None = None
) -> typing.Callable[..., Any]:
    """Use the callback decorator to mark a method as a callback that can be invoked from the Slint component.

    For the decorator to work, the method must be a member of a class that is Slint component.

    Example:
    ```python
    import slint

    class AppMainWindow(slint.loader.main_window.MainWindow):

        # Automatically connected to a callback button_clicked()
        # in main_window.slint's MainWindow.
        @slint.callback()
        def button_clicked(self):
            print("Button clicked")

    ...
    ```

    Use the `name` parameter to specify the name of the callback in the Slint component, if the name of the
    Python method differs from the name of the callback in the Slint component.

    Use the `global_name` parameter to specify the name of the global in the Slint component, if the callback
    is to be set on a Slint global object.
    """

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


def set_xdg_app_id(app_id: str) -> None:
    """Sets the application id for use on Wayland or X11 with [xdg](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
    compliant window managers. This must be set before the window is shown, and has only an effect on Wayland or X11."""

    native.set_xdg_app_id(app_id)


__all__ = [
    "CompileError",
    "Component",
    "load_file",
    "loader",
    "Image",
    "Color",
    "Brush",
    "Model",
    "ListModel",
    "Timer",
    "TimerMode",
    "set_xdg_app_id",
    "callback",
]
