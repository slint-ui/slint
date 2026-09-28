# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import re
import time
from collections.abc import Callable
from typing import TypeVar

import slint_testing

Name = str | re.Pattern[str]
T = TypeVar("T")


class StaleElement(Exception):
    pass


def _matches(actual: str, expected: Name | None, exact: bool) -> bool:
    if expected is None:
        return True
    if isinstance(expected, re.Pattern):
        return expected.search(actual) is not None
    return actual == expected if exact else expected in actual


class Scope:
    def _roots(self, deadline: float) -> list[slint_testing.Element]:
        raise NotImplementedError

    def get_by_role(
        self,
        role: slint_testing.AccessibleRole,
        *,
        name: Name | None = None,
        exact: bool = True,
    ) -> Locator:
        return Locator(
            self,
            f"get_by_role({role.name}, name={name!r})",
            role=role,
            name=name,
            exact=exact,
        )

    def get_by_accessible_name(self, name: Name, *, exact: bool = True) -> Locator:
        return Locator(
            self,
            f"get_by_accessible_name({name!r})",
            name=name,
            exact=exact,
        )

    def get_by_id(self, identifier: str) -> Locator:
        return Locator(self, f"get_by_id({identifier!r})", identifier=identifier)


class Window(Scope):
    def __init__(self, raw: slint_testing.Window):
        self.raw = raw

    def __getattr__(self, name: str):
        return getattr(self.raw, name)

    @property
    def root_element(self) -> slint_testing.Element:
        return self.raw.root_element

    def _roots(self, deadline: float) -> list[slint_testing.Element]:
        del deadline
        return [self.raw.root_element]


class Locator(Scope):
    def __init__(
        self,
        scope: Scope,
        description: str,
        *,
        role: slint_testing.AccessibleRole | None = None,
        name: Name | None = None,
        exact: bool = True,
        identifier: str | None = None,
        index: int | None = None,
    ):
        self.scope = scope
        self.description = description
        self.role = role
        self.name = name
        self.exact = exact
        self.identifier = identifier
        self.index = index

    def __repr__(self) -> str:
        prefix = f"{self.scope!r} >> " if isinstance(self.scope, Locator) else ""
        suffix = f".nth({self.index})" if self.index is not None else ""
        return prefix + self.description + suffix

    def _find(self, deadline: float) -> list[slint_testing.Element]:
        matches = []
        for root in self.scope._roots(deadline):
            query = root.query_descendants()
            if self.role is not None:
                query = query.match_accessible_role(self.role)
            if self.identifier is not None:
                query = query.match_id(self.identifier)
            for element in query.find_all():
                if not element.is_valid:
                    continue
                if _matches(element.accessible_label, self.name, self.exact):
                    matches.append(element)
            if not root.is_valid:
                raise StaleElement
        if self.index is None:
            return matches
        return matches[self.index : self.index + 1]

    def _resolve(self, deadline: float) -> slint_testing.Element:
        last_count = 0
        while True:
            try:
                matches = self._find(deadline)
                last_count = len(matches)
                if last_count == 1:
                    return matches[0]
            except StaleElement:
                pass
            if time.monotonic() >= deadline:
                raise AssertionError(
                    f"expected exactly one element for {self!r}, found {last_count}"
                )
            time.sleep(0.02)

    def _roots(self, deadline: float) -> list[slint_testing.Element]:
        return [self._resolve(deadline)]

    def resolve(self, *, timeout: float = 5) -> slint_testing.Element:
        return self._resolve(time.monotonic() + timeout)

    def all(self, *, timeout: float = 5) -> list[slint_testing.Element]:
        deadline = time.monotonic() + timeout
        while True:
            try:
                return self._find(deadline)
            except StaleElement:
                if time.monotonic() >= deadline:
                    raise AssertionError(f"scope for {self!r} remained stale")
                time.sleep(0.02)

    def count(self) -> int:
        return len(self.all())

    def nth(self, index: int) -> Locator:
        if index < 0:
            raise ValueError("index must be nonnegative")
        return Locator(
            self.scope,
            self.description,
            role=self.role,
            name=self.name,
            exact=self.exact,
            identifier=self.identifier,
            index=index,
        )

    def read(self, getter: Callable[[slint_testing.Element], T]) -> T:
        deadline = time.monotonic() + 5
        while True:
            element = self._resolve(deadline)
            value = getter(element)
            if element.is_valid:
                return value
            if time.monotonic() >= deadline:
                raise AssertionError(f"element for {self!r} remained stale")

    def invoke_accessible_default_action(self, *, timeout: float = 5) -> None:
        self.resolve(timeout=timeout).invoke_accessible_default_action()

    def single_click(
        self,
        button: slint_testing.PointerEventButton,
        *,
        timeout: float = 5,
    ) -> None:
        self.resolve(timeout=timeout).single_click(button)

    def double_click(
        self,
        button: slint_testing.PointerEventButton,
        *,
        timeout: float = 5,
    ) -> None:
        self.resolve(timeout=timeout).double_click(button)
