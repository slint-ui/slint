from __future__ import annotations

import enum

from typing import Any, Callable

import slint

__all__ = ['CounterWindow']

class CounterWindow(slint.Component):
    def __init__(self, **kwargs: Any) -> None:
        ...
    counter: int
    request_increase: Callable[[], None]

