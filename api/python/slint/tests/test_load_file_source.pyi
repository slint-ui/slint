from __future__ import annotations

import enum
from typing import (
    Any,
    Callable,
)
import slint

__all__ = ['Diag', 'App', 'MyData', 'Secret_Struct', 'TestEnum', 'MyDiag', 'Public_Struct']

class MyData:
    def __init__(self, *, age: float = ..., name: str = ...) -> None: ...
    age: float
    name: str

class Secret_Struct:
    def __init__(self, *, balance: float = ...) -> None: ...
    balance: float

class TestEnum(enum.Enum):
    Variant1 = 'Variant1'
    Variant2 = 'Variant2'

class Diag(slint.Component):
    def __init__(self, **kwargs: Any) -> None: ...
    class MyGlobal:
        global_prop: str
        global_callback: Callable[[str], str]
        minus_one: Callable[[int], None]
    class SecondGlobal:
        second: str

class App(slint.Component):
    def __init__(self, **kwargs: Any) -> None: ...
    builtin_enum: Any
    enum_property: TestEnum
    hello: str
    model_with_enums: slint.ListModel[Any]
    translated: str
    call_void: Callable[[], None]
    invoke_call_void: Callable[[], None]
    invoke_global_callback: Callable[[str], str]
    invoke_say_hello: Callable[[str], str]
    invoke_say_hello_again: Callable[[str], str]
    say_hello: Callable[[str], str]
    say_hello_again: Callable[[str], str]
    plus_one: Callable[[int], None]
    class MyGlobal:
        global_prop: str
        global_callback: Callable[[str], str]
        minus_one: Callable[[int], None]
    class SecondGlobal:
        second: str

MyDiag = Diag

Public_Struct = Secret_Struct

