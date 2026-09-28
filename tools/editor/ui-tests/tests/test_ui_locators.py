# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, cast

import pytest
import slint_testing
from ui_locators import Window


@dataclass
class Element:
    accessible_label: str
    accessible_role: slint_testing.AccessibleRole
    identifier: str = ""
    children: list[Element] = field(default_factory=list)
    is_valid: bool = True
    actions: int = 0
    accessible_value: str = ""

    def query_descendants(self):
        return Query(self)

    def invoke_accessible_default_action(self):
        self.actions += 1


class Query:
    def __init__(self, root: Element):
        self.root = root
        self.role = None
        self.identifier = None

    def match_accessible_role(self, role):
        self.role = role
        return self

    def match_id(self, identifier):
        self.identifier = identifier
        return self

    def find_all(self):
        def descendants(element):
            for child in element.children:
                yield child
                yield from descendants(child)

        return [
            element
            for element in descendants(self.root)
            if (self.role is None or element.accessible_role == self.role)
            and (self.identifier is None or element.identifier == self.identifier)
        ]


@dataclass
class RawWindow:
    root_element: Element


def button(name: str, *, identifier: str = "") -> Element:
    return Element(name, slint_testing.AccessibleRole.Button, identifier)


def test_locator_queries_only_when_used():
    raw = RawWindow(Element("root", slint_testing.AccessibleRole.Unknown))
    locator = Window(cast(Any, raw)).get_by_role(
        slint_testing.AccessibleRole.Button, name="Save"
    )
    raw.root_element.children.append(button("Save"))
    assert locator.resolve().accessible_label == "Save"


def test_locator_resolves_replaced_ancestor():
    first = Element(
        "Panel",
        slint_testing.AccessibleRole.Groupbox,
        children=[
            Element(
                "Save",
                slint_testing.AccessibleRole.Button,
                identifier="first",
                accessible_value="first",
            )
        ],
    )
    raw = RawWindow(
        Element("root", slint_testing.AccessibleRole.Unknown, children=[first])
    )
    save = (
        Window(cast(Any, raw))
        .get_by_role(slint_testing.AccessibleRole.Groupbox, name="Panel")
        .get_by_role(slint_testing.AccessibleRole.Button, name="Save")
    )
    assert save.accessible_value == "first"

    first.is_valid = False
    raw.root_element.children = [
        Element(
            "Panel",
            slint_testing.AccessibleRole.Groupbox,
            children=[
                Element(
                    "Save",
                    slint_testing.AccessibleRole.Button,
                    identifier="second",
                    accessible_value="second",
                )
            ],
        )
    ]
    assert save.accessible_value == "second"
    save.invoke_accessible_default_action()
    assert raw.root_element.children[0].children[0].actions == 1


def test_locator_requires_one_match():
    raw = RawWindow(
        Element(
            "root",
            slint_testing.AccessibleRole.Unknown,
            children=[button("Save"), button("Save")],
        )
    )
    save = Window(cast(Any, raw)).get_by_role(
        slint_testing.AccessibleRole.Button, name="Save"
    )
    with pytest.raises(AssertionError, match="found 2"):
        save.resolve(timeout=0)


def test_locator_supports_scoping_ids_and_position():
    panel = Element(
        "Panel",
        slint_testing.AccessibleRole.Groupbox,
        children=[button("Duplicate", identifier="first"), button("Duplicate")],
    )
    window = Window(
        cast(
            Any,
            RawWindow(
                Element("root", slint_testing.AccessibleRole.Unknown, children=[panel])
            ),
        )
    )
    scope = window.get_by_role(slint_testing.AccessibleRole.Groupbox, name="Panel")
    duplicates = scope.get_by_role(
        slint_testing.AccessibleRole.Button, name="Duplicate"
    )
    assert duplicates.count() == 2
    assert cast(Any, duplicates.nth(1).resolve()).identifier == ""
    assert scope.get_by_id("first").resolve().accessible_label == "Duplicate"
