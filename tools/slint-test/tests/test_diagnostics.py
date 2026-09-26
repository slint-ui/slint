# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from typing import cast

import pytest

from slint_test import Session, expect, reporting
from slint_test.control import debugging
from slint_test.core import Locator, StaleElement
from slint_test.diagnostics import AssertionFailure


@pytest.mark.parametrize(
    "actual, expected, summary",
    [
        ("200", "250", "Expected 250, observed 200"),
        (False, True, "Expected True, observed False"),
        ("", "ready", "Expected ready, observed ''"),
        ("old\nvalue", "new", "Expected new, observed old\\nvalue"),
    ],
)
def test_failure_summary_preserves_observed_values(actual, expected, summary):
    events = []
    with pytest.raises(AssertionFailure) as failure, reporting(events.append):
        expect.poll(lambda: actual, message="Width").to_equal(expected, timeout=30)
    assert failure.value.diagnostic["summary"] == summary
    assert failure.value.diagnostic["actual"] == repr(actual)
    assert events[-1]["diagnostic"] == failure.value.diagnostic
    assert "last observed" in str(failure.value)
    json.dumps(events)


def test_no_observation_is_not_presented_as_a_value():
    def stale():
        raise StaleElement

    with pytest.raises(AssertionFailure) as failure:
        expect.poll(stale).to_equal("ready", timeout=30)
    assert failure.value.diagnostic["summary"] == "Expected ready; no value observed"
    assert failure.value.diagnostic["actual"] is None


def test_transport_failure_is_not_reported_as_a_comparison():
    def disconnected():
        raise RuntimeError("Application disconnected")

    with pytest.raises(RuntimeError, match="disconnected") as failure:
        expect.poll(disconnected).to_equal("ready")
    assert not hasattr(failure.value, "diagnostic")


def test_negative_and_sustained_assertions():
    with pytest.raises(AssertionFailure) as failure:
        expect.poll(lambda: 200).not_to_equal(200, timeout=30)
    assert (
        failure.value.diagnostic["summary"]
        == "Expected a value other than 200, observed 200"
    )
    with pytest.raises(AssertionFailure) as failure:
        expect.poll(lambda: 200).to_remain(250, for_ms=30)
    assert failure.value.diagnostic["summary"] == "Expected 250, observed 200"
    assert failure.value.diagnostic["comparison"] == "remain"


def test_debugger_receives_diagnostic_before_reporting():
    received = []

    class Control:
        def before(self, event):
            pass

        def after(self, event):
            pass

        def failed(self, event, error):
            received.append(event["diagnostic"])

    with pytest.raises(AssertionFailure), debugging(Control()):
        expect.poll(lambda: 200, session=Session()).to_equal(250, timeout=30)
    assert received[0]["summary"] == "Expected 250, observed 200"


def test_large_values_have_bounded_summary_and_complete_detail():
    actual = "a" * 1000
    with pytest.raises(AssertionFailure) as failure:
        expect.poll(lambda: actual).to_equal("b" * 1000, timeout=30)
    assert len(failure.value.diagnostic["summary"]) < 230
    assert failure.value.diagnostic["actual"] == repr(actual)


def test_same_display_with_different_types_is_disambiguated():
    with pytest.raises(AssertionFailure) as failure:
        expect.poll(lambda: 200).to_equal("200", timeout=30)
    assert (
        failure.value.diagnostic["summary"]
        == "Expected '200' (str), observed 200 (int)"
    )


def test_locator_failure_keeps_element_and_adapter_context():
    from types import SimpleNamespace

    from slint_test.assertions import LocatorAssertion

    element = SimpleNamespace(
        handle=SimpleNamespace(index=12, generation=3), accessible_value="200"
    )
    locator = SimpleNamespace(
        name="Width",
        window=SimpleNamespace(session=Session()),
        read=lambda getter: getter(element),
        source_context=lambda: {"element": "root-rectangle", "property": "width"},
    )
    with pytest.raises(AssertionFailure) as failure:
        LocatorAssertion(cast(Locator, locator)).to_have_value("250", timeout=30)
    assert failure.value.diagnostic["location"] == {
        "handle": {"index": 12, "generation": 3},
        "property": "accessible_value",
        "source": {"element": "root-rectangle", "property": "width"},
    }
    json.dumps(failure.value.diagnostic)


def test_location_failure_cannot_replace_assertion(caplog):
    from slint_test.assertions import Assertion

    def broken():
        raise RuntimeError("Source unavailable")

    with pytest.raises(AssertionFailure) as failure:
        Assertion(lambda: 200, Session(), "Width", context=broken).to_equal(
            250, timeout=30
        )
    assert failure.value.diagnostic["summary"] == "Expected 250, observed 200"
    assert "Source unavailable" in caplog.text
