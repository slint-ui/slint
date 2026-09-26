# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from types import SimpleNamespace

import pytest
import slint_testing as low

from slint_test import native


class Transport:
    def __init__(self, **response):
        self.response = native.Response(**response)

    def _send_request(self, request):
        self.request = request
        return low.AUTResponse.FromString(self.response.SerializeToString())


def test_capability_negotiation_preserves_unknown_wire_fields():
    assert native.supported(
        Transport(testing_capabilities_response={"pointer_target_version": 1})
    )
    assert not native.supported(
        Transport(testing_capabilities_response={"pointer_target_version": 2})
    )
    assert not native.supported(Transport(error={"message": "Empty request"}))
    with pytest.raises(RuntimeError, match="Disconnected"):
        native.supported(Transport(error={"message": "Disconnected"}))


@pytest.mark.parametrize(
    "part, match",
    [
        ({"status": "invented"}, "Unknown pointer target status"),
        ({"status": "ready"}, "Invalid pointer target position"),
        (
            {"status": "ready", "position": {"x": float("nan")}},
            "Invalid pointer target position",
        ),
        (
            {"status": "covered", "position": {}, "performed": True},
            "Invalid checked click result",
        ),
    ],
)
def test_malformed_target_responses_are_protocol_errors(part, match):
    element = SimpleNamespace(
        app=Transport(pointer_target_response=part), handle=low.Handle()
    )
    with pytest.raises(RuntimeError, match=match):
        native.target(element)


def test_checked_click_reports_performed_and_target():
    app = Transport(
        pointer_target_response={
            "status": "ready",
            "position": {"x": 12, "y": 34},
            "performed": True,
        }
    )
    result = native.target(SimpleNamespace(app=app, handle=low.Handle()), click=True)
    assert result["performed"] and (result["x"], result["y"]) == (12, 34)
    assert app.request.WhichOneof("msg") == "request_checked_click"


def test_destroyed_element_is_retryable_but_invalid_handle_is_not():
    from slint_test.diagnostics import StaleElement

    for message in (
        "Stale element",
        "Element handle for checked_click refers to element that was destroyed",
    ):
        with pytest.raises(StaleElement):
            native.exchange(Transport(error={"message": message}), native.Request())
    with pytest.raises(RuntimeError, match="Invalid element handle"):
        native.exchange(
            Transport(error={"message": "Invalid element handle for checked_click"}),
            native.Request(),
        )
