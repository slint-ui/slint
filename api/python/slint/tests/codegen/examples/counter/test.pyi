from __future__ import annotations

import enum
from typing import (
    Any,
    Callable,
    Optional,
)

import slint

__all__ = ['OptionalDemo', 'OptionalFloat', 'OptionalBool', 'OptionalInt', 'OptionalString', 'OptionalEnum']

class OptionalFloat:
    def __init__(self, *, maybe_value: float = ...) -> None: ...
    maybe_value: float

class OptionalBool:
    def __init__(self, *, maybe_value: bool = ...) -> None: ...
    maybe_value: bool

class OptionalInt:
    def __init__(self, *, maybe_value: float = ...) -> None: ...
    maybe_value: float

class OptionalString:
    def __init__(self, *, maybe_value: str = ...) -> None: ...
    maybe_value: str

class OptionalEnum(enum.Enum):
    OptionA = 'OptionA'
    OptionB = 'OptionB'
    OptionC = 'OptionC'

class OptionalDemo(slint.Component):
    def __init__(self, **kwargs: Any) -> None: ...
    maybe_count: Optional[int]
    on_action: Callable[[Optional[float]], Optional[int]]
    compute: Callable[[Optional[str]], Optional[bool]]

