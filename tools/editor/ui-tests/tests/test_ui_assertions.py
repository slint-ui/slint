# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, cast

import pytest
import slint_testing
from ui_assertions import AssertionFailure, expect


@dataclass
class Element:
    accessible_label: str = ""
    value: str = ""
    accessible_enabled: bool = True
    accessible_item_selected: bool = False
    absolute_rect: slint_testing.LogicalRect = field(
        default_factory=lambda: slint_testing.LogicalRect(0.0, 0.0, 10.0, 10.0)
    )
    gone: bool = False

    @property
    def accessible_value(self) -> str:
        if self.gone:
            raise slint_testing.RequestError("element was destroyed")
        return self.value


class Query(slint_testing.ElementQuery):
    """Stands in for a query; `results` yields the matches of successive runs."""

    def __init__(self, *results: list[Element]):
        self.results = list(results)

    def __repr__(self) -> str:
        return "query"

    def find_all(self) -> list[Any]:
        return self.results.pop(0) if len(self.results) > 1 else self.results[0]

    def find_one(self) -> Any:
        matches = self.find_all()
        if len(matches) != 1:
            raise slint_testing.ElementLookupError(f"found {len(matches)}")
        return matches[0]


def test_poll_retries_until_value_matches():
    values = iter(["loading", "ready"])
    expect.poll(lambda: next(values)).to_equal("ready", timeout=0.1)


def test_poll_failure_reports_expected_and_observed():
    with pytest.raises(AssertionFailure) as caught:
        expect.poll(lambda: 200, message="preview width").to_equal(250, timeout=0)
    assert str(caught.value) == "Expected 250, observed 200"


def test_poll_propagates_unexpected_errors():
    def fail():
        raise RuntimeError("application disconnected")

    with pytest.raises(RuntimeError, match="application disconnected"):
        expect.poll(fail).to_equal(True)


def test_query_assertion_follows_replaced_element():
    original = Element("Name", value="first", gone=True)
    replacement = Element("Name", value="second")
    expect(cast(Any, Query([original], [replacement]))).to_have_value(
        "second", timeout=0.1
    )


def test_query_failure_distinguishes_no_observation():
    with pytest.raises(AssertionFailure) as caught:
        expect(cast(Any, Query([]))).to_have_value("ready", timeout=0)
    assert str(caught.value) == "Expected ready; no value observed"


def test_element_read_failure_is_no_observation():
    with pytest.raises(AssertionFailure) as caught:
        expect(cast(Any, Element(gone=True))).to_have_value("ready", timeout=0)
    assert str(caught.value) == "Expected ready; no value observed"


def test_query_state_and_visibility_assertions():
    selected = Element("Selected", accessible_item_selected=True)
    expect(cast(Any, Query([selected]))).to_be_selected(timeout=0)
    expect(cast(Any, Query([]))).to_be_hidden(timeout=0)
    expect(cast(Any, Query([selected]))).to_be_visible(timeout=0)


def test_counting_needs_a_query():
    with pytest.raises(TypeError, match="query"):
        expect(cast(Any, Element())).to_be_hidden(timeout=0)


def test_geometry_supports_approximate_values():
    element = Element(
        "Selection",
        absolute_rect=slint_testing.LogicalRect(20.1, 24.0, 100.2, 80.0),
    )
    expect(cast(Any, Query([element]))).to_have_geometry(
        x=pytest.approx(20, abs=0.5),
        width=pytest.approx(100, abs=0.5),
        timeout=0,
    )


def test_invalid_timeout_is_rejected():
    with pytest.raises(ValueError, match="timeout"):
        expect.poll(lambda: True).to_equal(True, timeout=-1)
