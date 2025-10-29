from __future__ import annotations

from typing import (
    Any,
    Callable,
)
import slint

__all__ = ['CounterWindow']

class CounterWindow(slint.Component):
    def __init__(self, **kwargs: Any) -> None: ...
    alignment: Any
    counter: int
    request_increase: Callable[[], None]

