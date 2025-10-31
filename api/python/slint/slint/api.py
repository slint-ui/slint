# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

r"""
.. include:: ../README.md
"""

import asyncio
import copy
import gettext
import keyword
import logging
import os
import pathlib
import sys
import types
import typing
from collections.abc import Coroutine
from pathlib import Path
from typing import Any, Callable, TypeVar, overload

from .core import (
    Brush,
    Color,
    Compiler,
    ComponentDefinition,
    ComponentInstance,
    DiagnosticLevel,
    Image,
    PyDiagnostic,
    PyStruct,
    Timer,
    TimerMode,
)
from .core import (
    init_translations as _slint_init_translations,
)
from .loop import SlintEventLoop
from .models import ListModel, Model


class CompileError(Exception):
    message: str
    """The error message that produced this compile error."""

    diagnostics: list[PyDiagnostic]
    """A list of detailed diagnostics that were produced as part of the compilation."""

    def __init__(self, message: str, diagnostics: list[PyDiagnostic]):
        """@private"""
        super().__init__(message)
        self.message = message
        self.diagnostics = diagnostics
        for diag in self.diagnostics:
            self.add_note(str(diag))


class Component:
    """Component is the base class for all instances of Slint components. Use the member functions to show or hide the
    window, or spin the event loop."""

    __instance__: ComponentInstance

    def show(self) -> None:
        """Shows the window on the screen."""

        self.__instance__.show()

    def hide(self) -> None:
        """Hides the window from the screen."""

        self.__instance__.hide()

    def run(self) -> None:
        """Shows the window, runs the event loop, hides it when the loop is quit, and returns."""
        self.show()
        run_event_loop()
        self.hide()


def _normalize_prop(name: str) -> str:
    ident = name.replace("-", "_")
    if ident and ident[0].isdigit():
        ident = f"_{ident}"
    if keyword.iskeyword(ident):
        ident = f"{ident}_"
    return ident


def _build_global_class(compdef: ComponentDefinition, global_name: str) -> Any:
    properties_and_callbacks = {}

    global_props = compdef.global_properties(global_name)
    global_callbacks = compdef.global_callbacks(global_name)
    global_functions = compdef.global_functions(global_name)

    assert global_props is not None
    assert global_callbacks is not None
    assert global_functions is not None

    for prop_name in global_props.keys():
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

    for callback_name in global_callbacks:
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

    for function_name in global_functions:
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
    compdef: ComponentDefinition,
) -> typing.Callable[..., Component]:
    def cls_init(self: Component, **kwargs: Any) -> Any:
        self.__instance__ = compdef.create()
        for name, value in self.__class__.__dict__.items():
            if hasattr(value, "slint.callback"):
                callback_info = getattr(value, "slint.callback")
                name = callback_info["name"]

                is_async = getattr(value, "slint.async", False)
                if is_async:
                    if "global_name" in callback_info:
                        global_name = callback_info["global_name"]
                        if not compdef.global_callback_returns_void(global_name, name):
                            raise RuntimeError(
                                f"Callback '{name}' in global '{global_name}' cannot be used with a callback decorator for an async function, as it doesn't return void"
                            )
                    else:
                        if not compdef.callback_returns_void(name):
                            raise RuntimeError(
                                f"Callback '{name}' cannot be used with a callback decorator for an async function, as it doesn't return void"
                            )

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


def _build_struct(name: str, struct_prototype: PyStruct) -> type:
    field_names = {field_name for field_name, _ in struct_prototype}

    def new_struct(cls: Any, *args: Any, **kwargs: Any) -> PyStruct:
        if args:
            raise TypeError(f"{name}() accepts keyword arguments only")

        unexpected = set(kwargs) - field_names
        if unexpected:
            formatted = ", ".join(sorted(unexpected))
            raise TypeError(f"{name}() got unexpected keyword argument(s): {formatted}")

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
    """This function is the low-level entry point into Slint for instantiating components. It loads the `.slint` file at
    the specified `path` and returns a namespace with all exported components as Python classes, as well as enums, and structs.

    * `quiet`: Set to true to prevent any warnings during compilation from being printed to stderr.
    * `style`: Specify a widget style.
    * `include_paths`: Additional include paths used to look up `.slint` files imported from other `.slint` files.
    * `library_paths`: A dictionary that maps library names to their location in the file system. This is then used to look up
       library imports, such as `import { MyButton } from "@mylibrary";`.
    * `translation_domain`: The domain to use for looking up the catalogue run-time translations. This must match the
       translation domain used when extracting translations with `slint-tr-extractor`.

    """

    compiler = Compiler()

    if style is not None:
        compiler.style = style
    if include_paths is not None:
        compiler.include_paths = include_paths  # type: ignore[assignment]
    if library_paths is not None:
        compiler.library_paths = library_paths  # type: ignore[assignment]
    if translation_domain is not None:
        compiler.set_translation_domain(translation_domain)

    result = compiler.build_from_path(Path(path))

    diagnostics = result.diagnostics
    if diagnostics:
        if not quiet:
            for diag in diagnostics:
                if diag.level == DiagnosticLevel.Warning:
                    logging.warning(diag)

        errors = [diag for diag in diagnostics if diag.level == DiagnosticLevel.Error]
        if errors:
            raise CompileError(f"Could not compile {path}", diagnostics)

    module = types.SimpleNamespace()
    for comp_name in result.component_names:
        comp = result.component(comp_name)

        if comp is None:
            continue

        wrapper_class = _build_class(comp)

        setattr(module, comp_name, wrapper_class)

    structs, enums = result.structs_and_enums

    for name, struct_prototype in structs.items():
        name = _normalize_prop(name)
        struct_wrapper = _build_struct(name, struct_prototype)
        setattr(module, name, struct_wrapper)

    for name, enum_class in enums.items():
        name = _normalize_prop(name)
        setattr(module, name, enum_class)

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
"""Use the global `loader` object to load Slint files from the file system. It exposes two stages of attributes:
1. Any lookup of an attribute in the loader tries to match a file in `sys.path` with the `.slint` extension. For example
   `loader.my_component` looks for a file `my_component.slint` in the directories in `sys.path`.
2. Any lookup in the object returned by the first stage tries to match an exported component in the loaded file, or a
   struct, or enum. For example `loader.my_component.MyComponent` looks for an *exported* component named `MyComponent`
   in the file `my_component.slint`.

**Note:** The first entry in the module search path `sys.path` is the directory that contains the input script.

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

    try:
        import inspect

        if inspect.iscoroutinefunction(callable):

            def run_as_task(*args, **kwargs) -> None:  # type: ignore
                loop = asyncio.get_event_loop()
                loop.create_task(callable(*args, **kwargs))

            setattr(run_as_task, "slint.callback", info)
            setattr(run_as_task, "slint.async", True)
            return run_as_task
    except ImportError:
        pass

    return callable


_T_Callback = TypeVar("_T_Callback", bound=Callable[..., Any])


@overload
def callback(__func: _T_Callback, /) -> _T_Callback: ...


@overload
def callback(
    *, global_name: str | None = ..., name: str | None = ...
) -> Callable[[_T_Callback], _T_Callback]: ...


@overload
def callback(
    __func: _T_Callback, /, *, global_name: str | None = ..., name: str | None = ...
) -> _T_Callback: ...


def callback(
    __func: _T_Callback | None = None,
    /,
    *,
    global_name: str | None = None,
    name: str | None = None,
) -> typing.Union[_T_Callback, typing.Callable[[_T_Callback], _T_Callback]]:
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

    If your Python method has a different name from the Slint component's callback, use the `name` parameter to specify
    the correct name. Similarly, use the `global_name` parameter to specify the name of the correct global singleton in
    the Slint component.

    **Note:** The callback decorator can also be used with async functions. They will be run as task in the asyncio event loop.
    This is only supported for callbacks that don't return any value, and requires Python >= 3.13.
    """

    # If used as @callback without args: __func is the callable
    if __func is not None and callable(__func):
        return _callback_decorator(__func, {})

    info: dict[str, str] = {}
    if name:
        info["name"] = name
    if global_name:
        info["global_name"] = global_name

    def _wrapper(fn: _T_Callback) -> _T_Callback:
        return typing.cast(_T_Callback, _callback_decorator(fn, info))

    return _wrapper


def set_xdg_app_id(app_id: str) -> None:
    """Sets the application id for use on Wayland or X11 with [xdg](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
    compliant window managers. This id must be set before the window is shown; it only applies to Wayland or X11."""

    set_xdg_app_id(app_id)


quit_event = asyncio.Event()


def run_event_loop(
    main_coro: typing.Optional[Coroutine[None, None, None]] = None,
) -> None:
    """Runs the main Slint event loop. If specified, the coroutine `main_coro` is run in parallel. The event loop doesn't
    terminate when the coroutine finishes, it terminates when calling `quit_event_loop()`.

    Example:
    ```python
    import slint

    ...
    image_model: slint.ListModel[slint.Image] = slint.ListModel()
    ...

    async def main_receiver(image_model: slint.ListModel) -> None:
        async with aiohttp.ClientSession() as session:
            async with session.get("http://some.server/svg-image") as response:
                svg = await response.read()
                image = slint.Image.from_svg_data(svg)
                image_model.append(image)

    ...
    slint.run_event_loop(main_receiver(image_model))
    ```

    """

    async def run_inner() -> None:
        global quit_event
        loop = typing.cast(SlintEventLoop, asyncio.get_event_loop())

        quit_task = asyncio.ensure_future(quit_event.wait(), loop=loop)

        tasks: typing.List[asyncio.Task[typing.Any]] = [quit_task]

        main_task = None
        if main_coro:
            main_task = loop.create_task(main_coro)
            tasks.append(main_task)

        done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)

        if main_task is not None and main_task in done:
            main_task.result()  # propagate exception if thrown
            if quit_task in pending:
                await quit_event.wait()

    global quit_event
    quit_event = asyncio.Event()

    with asyncio.Runner(loop_factory=SlintEventLoop) as runner:
        runner.run(run_inner())


def quit_event_loop() -> None:
    """Quits the running event loop in the next event processing cycle. This will make an earlier call to `run_event_loop()`
    return."""
    global quit_event
    quit_event.set()


def init_translations(translations: typing.Optional[gettext.GNUTranslations]) -> None:
    """Installs the specified translations object to handle translations originating from the Slint code.

    Example:
    ```python
    import gettext
    import slint

    translations_dir = os.path.join(os.path.dirname(__file__), "lang")
    try:
        translations = gettext.translation("my_app", translations_dir, ["de"])
        slint.install_translations(translations)
    except OSError:
        pass
    ```
    """
    _slint_init_translations(translations)


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
    "run_event_loop",
    "quit_event_loop",
    "init_translations",
]
