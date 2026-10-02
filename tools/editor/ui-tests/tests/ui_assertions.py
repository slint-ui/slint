# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import math
import time
from collections.abc import Callable
from typing import Any

import slint_testing


def _display(value: Any) -> str:
    text = (
        value
        if isinstance(value, str) and value.strip() == value and value
        else repr(value)
    )
    text = text.replace("\n", "\\n").replace("\r", "\\r")
    return text if len(text) <= 100 else text[:99] + "…"


class AssertionFailure(AssertionError):
    def __init__(
        self,
        expected: Any,
        actual: Any,
        *,
        observed: bool,
        timeout: float,
        target: str,
        comparison: str = "equal",
    ):
        wanted = _display(expected)
        seen = _display(actual) if observed else ""
        expectation = (
            f"a value other than {wanted}" if comparison == "not_equal" else wanted
        )
        summary = (
            f"Expected {expectation}, observed {seen}"
            if observed
            else f"Expected {expectation}; no value observed"
        )
        super().__init__(summary)
        self.expected = expected
        self.actual = actual if observed else None
        self.observed = observed
        self.timeout = timeout
        self.target = target
        self.comparison = comparison


class Assertion:
    def __init__(
        self,
        read: Callable[[], Any],
        label: str,
        *,
        target: str = "",
    ):
        self.read = read
        self.label = label
        self.target = target or label

    def _compare(self, expected: Any, timeout: float, *, negate: bool) -> None:
        actual: Any = None
        observed = False

        def matches() -> bool:
            nonlocal actual, observed
            try:
                actual = self.read()
            except slint_testing.RequestError as error:
                # For example, the element was replaced; wait_until() tries again.
                actual = str(error)
                observed = False
                raise
            observed = True
            return actual != expected if negate else actual == expected

        try:
            slint_testing.wait_until(matches, timeout, message=self.label)
        except slint_testing.WaitTimeoutError as error:
            raise AssertionFailure(
                expected,
                actual,
                observed=observed,
                timeout=timeout,
                target=self.target,
                comparison="not_equal" if negate else "equal",
            ) from error

    def to_equal(self, expected: Any, *, timeout: float = 5) -> None:
        self._compare(expected, timeout, negate=False)

    def not_to_equal(self, expected: Any, *, timeout: float = 5) -> None:
        self._compare(expected, timeout, negate=True)

    def to_remain(
        self,
        expected: Any,
        *,
        for_seconds: float = 0.25,
        interval: float = 0.02,
    ) -> None:
        if (
            not all(map(math.isfinite, (for_seconds, interval)))
            or for_seconds < 0
            or interval <= 0
        ):
            raise ValueError("observation interval must be finite and nonnegative")
        end = time.monotonic() + for_seconds
        while True:
            actual = self.read()
            if actual != expected:
                raise AssertionFailure(
                    expected,
                    actual,
                    observed=True,
                    timeout=for_seconds,
                    target=self.target,
                    comparison="remain",
                )
            remaining = end - time.monotonic()
            if remaining <= 0:
                return
            time.sleep(min(interval, remaining))


class ElementAssertion:
    """Retrying assertions on an element's properties, and for a query, on its matches.

    For a slint_testing.ElementQuery, every attempt looks up its only match and reads it, so the
    assertion follows replaced elements within its own timeout. For a slint_testing.Element, every
    attempt reads it directly; a read that fails because the element is gone counts as no
    observation. Elements from tracking queries don't wait past the assertion's timeout.
    """

    def __init__(
        self,
        target: slint_testing.Element | slint_testing.ElementQuery,
        message: str = "",
    ):
        self.target = target
        self.message = message

    def _target(self, property_name: str) -> str:
        return f"{self.target!r} · {property_name}"

    def _read(self, getter: Callable[[Any], Any]) -> Any:
        element = (
            self.target.find_one()
            if isinstance(self.target, slint_testing.ElementQuery)
            else self.target
        )
        return getter(element)

    def _property(self, name: str, expected: Any, timeout: float) -> None:
        Assertion(
            lambda: self._read(lambda element: getattr(element, name)),
            f"{self.message} {self.target!r}.{name}".strip(),
            target=self._target(name.removeprefix("accessible_").replace("_", " ")),
        ).to_equal(expected, timeout=timeout)

    def to_have_value(self, value: str, *, timeout: float = 5) -> None:
        self._property("accessible_value", value, timeout)

    def not_to_have_value(self, value: str, *, timeout: float = 5) -> None:
        Assertion(
            lambda: self._read(lambda element: element.accessible_value),
            f"{self.message} {self.target!r}.accessible_value".strip(),
            target=self._target("value"),
        ).not_to_equal(value, timeout=timeout)

    def to_have_accessible_name(self, name: str, *, timeout: float = 5) -> None:
        self._property("accessible_label", name, timeout)

    def to_have_description(self, description: str, *, timeout: float = 5) -> None:
        self._property("accessible_description", description, timeout)

    def to_be_enabled(self, enabled: bool = True, *, timeout: float = 5) -> None:
        self._property("accessible_enabled", enabled, timeout)

    def to_be_checked(self, checked: bool = True, *, timeout: float = 5) -> None:
        self._property("accessible_checked", checked, timeout)

    def to_be_selected(self, selected: bool = True, *, timeout: float = 5) -> None:
        self._property("accessible_item_selected", selected, timeout)

    def _count(self) -> int:
        if not isinstance(self.target, slint_testing.ElementQuery):
            raise TypeError("counting needs a query, not an element")
        return len(self.target.find_all())

    def to_have_count(self, count: int, *, timeout: float = 5) -> None:
        Assertion(
            self._count,
            f"{self.message} {self.target!r} count".strip(),
            target=self._target("count"),
        ).to_equal(count, timeout=timeout)

    def to_be_visible(self, *, timeout: float = 5) -> None:
        Assertion(
            self._count,
            f"{self.message} {self.target!r} visible count".strip(),
            target=self._target("visibility"),
        ).to_equal(1, timeout=timeout)

    def to_be_hidden(self, *, timeout: float = 5) -> None:
        Assertion(
            self._count,
            f"{self.message} {self.target!r} visible count".strip(),
            target=self._target("visibility"),
        ).to_equal(0, timeout=timeout)

    def to_have_geometry(
        self,
        expected: dict[str, Any] | None = None,
        *,
        x: Any = None,
        y: Any = None,
        width: Any = None,
        height: Any = None,
        timeout: float = 5,
    ) -> None:
        values = {
            key: value
            for key, value in {"x": x, "y": y, "width": width, "height": height}.items()
            if value is not None
        }
        values.update(expected or {})
        if values.keys() - {"x", "y", "width", "height"}:
            raise ValueError("geometry uses x, y, width, and height")

        def geometry(element: Any) -> dict[str, Any]:
            rect = element.absolute_rect
            return {key: getattr(rect, key) for key in values}

        Assertion(
            lambda: self._read(geometry),
            f"{self.message} {self.target!r} geometry".strip(),
            target=self._target("geometry"),
        ).to_equal(values, timeout=timeout)


class Expect:
    def __call__(
        self,
        target: slint_testing.Element | slint_testing.ElementQuery,
        message: str = "",
    ) -> ElementAssertion:
        return ElementAssertion(target, message)

    def poll(
        self,
        read: Callable[[], Any],
        *,
        message: str = "custom condition",
    ) -> Assertion:
        return Assertion(read, message)


expect = Expect()
