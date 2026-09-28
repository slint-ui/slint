# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import logging
import math
from collections.abc import Callable
from typing import Any

from .control import active_time
from .core import Locator, Session
from .diagnostics import AssertionFailure, WaitTimeout
from .reporting import step


class Assertion:
    def __init__(
        self,
        read: Callable[[], Any],
        session: Session,
        label: str,
        *,
        target: str = "",
        context: Callable[[], dict[str, Any]] | None = None,
    ):
        self.read, self.session, self.label = read, session, label
        self.target = target or label
        self.context = context

    def _compare(self, expected: Any, timeout: float | None, *, negate: bool) -> None:
        title = "Expect changed " if negate else "Expect "
        description = (
            f"{self.label}: expected "
            + ("a value other than " if negate else "")
            + repr(expected)
        )
        with step(
            title + self.label,
            layer="assertion",
            expected=repr(expected),
            target=self.target,
        ) as result:
            try:
                actual = self.session.wait(
                    self.read,
                    lambda value: value != expected if negate else value == expected,
                    timeout=timeout,
                    description=description,
                )
            except WaitTimeout as error:
                failure = AssertionFailure(
                    expected,
                    error,
                    target=self.target,
                    comparison="not_equal" if negate else "equal",
                )
                if self.context is not None:
                    try:
                        failure.diagnostic["location"] = self.context()
                    except Exception:
                        logging.getLogger(__name__).exception(
                            "Assertion location unavailable"
                        )
                raise failure from error
            result["actual"] = repr(actual)

    def to_equal(self, expected: Any, *, timeout: float | None = None) -> None:
        self._compare(expected, timeout, negate=False)

    def not_to_equal(self, expected: Any, *, timeout: float | None = None) -> None:
        self._compare(expected, timeout, negate=True)

    def to_remain(
        self, expected: Any, *, for_ms: float = 250, interval_ms: float = 20
    ) -> None:
        if (
            not all(map(math.isfinite, (for_ms, interval_ms)))
            or for_ms < 0
            or interval_ms <= 0
        ):
            raise ValueError("Invalid observation interval")
        with step(
            "Expect sustained " + self.label,
            layer="assertion",
            expected=repr(expected),
            for_ms=for_ms,
        ):
            end = active_time() + for_ms / 1000
            while True:
                self.session.check()
                actual = self.read()
                if actual != expected:
                    failure = WaitTimeout(
                        f"{self.label}: expected {expected!r}, observed {actual!r}",
                        actual=actual,
                        observed=True,
                        timeout=for_ms,
                    )
                    raise AssertionFailure(
                        expected, failure, target=self.target, comparison="remain"
                    )
                remaining = end - active_time()
                if remaining <= 0:
                    return
                self.session.cancel.wait(min(interval_ms / 1000, remaining))


class LocatorAssertion:
    def __init__(self, locator: Locator, message: str = ""):
        self.locator, self.message = locator, message

    def _target(self, property_name: str) -> str:
        name = (
            self.locator.name
            if isinstance(self.locator.name, str)
            else self.locator.identifier or "Element"
        )
        return f"{name} · {property_name}"

    def _property(self, name: str, expected: Any, timeout: float | None) -> None:
        location: dict[str, Any] = {"property": name}

        def read_element(element):
            location["handle"] = {
                "index": element.handle.index,
                "generation": element.handle.generation,
            }
            return getattr(element, name)

        def context():
            if self.locator.source_context is not None:
                location["source"] = self.locator.source_context()
            return location

        Assertion(
            lambda: self.locator.read(read_element),
            self.locator.window.session,
            f"{self.message} {self.locator!r}.{name}",
            target=self._target(name.removeprefix("accessible_").replace("_", " ")),
            context=context,
        ).to_equal(expected, timeout=timeout)

    def to_have_value(self, value: str, *, timeout: float | None = None) -> None:
        self._property("accessible_value", value, timeout)

    def not_to_have_value(self, value: str, *, timeout: float | None = None) -> None:
        Assertion(
            lambda: self.locator.read(lambda element: element.accessible_value),
            self.locator.window.session,
            f"{self.message} {self.locator!r}.accessible_value",
            target=self._target("value"),
        ).not_to_equal(value, timeout=timeout)

    def to_have_accessible_name(
        self, name: str, *, timeout: float | None = None
    ) -> None:
        self._property("accessible_label", name, timeout)

    def to_have_description(
        self, description: str, *, timeout: float | None = None
    ) -> None:
        self._property("accessible_description", description, timeout)

    def to_be_enabled(
        self, enabled: bool = True, *, timeout: float | None = None
    ) -> None:
        self._property("accessible_enabled", enabled, timeout)

    def to_be_checked(
        self, checked: bool = True, *, timeout: float | None = None
    ) -> None:
        self._property("accessible_checked", checked, timeout)

    def to_be_selected(
        self, selected: bool = True, *, timeout: float | None = None
    ) -> None:
        self._property("accessible_item_selected", selected, timeout)

    def to_have_count(self, count: int, *, timeout: float | None = None) -> None:
        Assertion(
            self.locator.count,
            self.locator.window.session,
            repr(self.locator) + " count",
            target=self._target("count"),
        ).to_equal(count, timeout=timeout)

    def to_be_visible(self, *, timeout: float | None = None) -> None:
        Assertion(
            self.locator.visible_count,
            self.locator.window.session,
            repr(self.locator) + " visible count",
            target=self._target("visibility"),
        ).to_equal(1, timeout=timeout)

    def to_be_hidden(self, *, timeout: float | None = None) -> None:
        Assertion(
            self.locator.visible_count,
            self.locator.window.session,
            repr(self.locator) + " visible count",
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
        timeout: float | None = None,
    ) -> None:
        values = {
            key: value
            for key, value in {"x": x, "y": y, "width": width, "height": height}.items()
            if value is not None
        }

        values.update(expected or {})
        if values.keys() - {"x", "y", "width", "height"}:
            raise ValueError("Geometry uses x, y, width, and height")

        def read():
            bounds = self.locator.bounds()
            return {key: getattr(bounds, key) for key in values}

        Assertion(
            read,
            self.locator.window.session,
            repr(self.locator) + " geometry",
            target=self._target("geometry"),
        ).to_equal(values, timeout=timeout)


class Expect:
    def __call__(self, locator: Locator, message: str = "") -> LocatorAssertion:
        return LocatorAssertion(locator, message)

    def poll(
        self,
        read: Callable[[], Any],
        *,
        session: Session | None = None,
        message: str = "custom condition",
    ) -> Assertion:
        return Assertion(read, session or Session(), message)


expect = Expect()
