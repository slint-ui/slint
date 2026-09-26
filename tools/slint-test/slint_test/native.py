# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Versioned extensions to the installed slint-testing transport."""

import math
from pathlib import Path
from typing import Any, cast

import slint_testing as low
from google.protobuf import descriptor_pb2, descriptor_pool, message_factory

from .diagnostics import StaleElement

_pool = cast(Any, descriptor_pool).DescriptorPool()
_descriptor = cast(Any, descriptor_pb2).FileDescriptorSet.FromString(
    Path(__file__).with_name("native.descriptor").read_bytes()
)
for _file in _descriptor.file:
    _pool.Add(_file)
Request: Any = message_factory.GetMessageClass(
    _pool.FindMessageTypeByName("proto.RequestToAUT")
)
Response: Any = message_factory.GetMessageClass(
    _pool.FindMessageTypeByName("proto.AUTResponse")
)


def exchange(app, request):
    response = Response.FromString(app._send_request(request).SerializeToString())
    if response.WhichOneof("msg") == "error":
        message = response.error.message
        if message == "Stale element" or (
            message.startswith("Element handle for ")
            and message.endswith("refers to element that was destroyed")
        ):
            raise StaleElement(message)
        raise RuntimeError(message)
    return response


def supported(app):
    try:
        response = exchange(app, Request(request_testing_capabilities={}))
    except RuntimeError as error:
        if str(error) == "Empty request":
            return False
        raise
    return (
        response.WhichOneof("msg") == "testing_capabilities_response"
        and response.testing_capabilities_response.pointer_target_version == 1
    )


def find_all(query):
    request = Request()
    part = request.request_query_element_descendants
    part.element_handle.ParseFromString(query.element.handle.SerializeToString())
    part.find_all = True
    part.include_clipped = True
    for instruction in query.instructions:
        part.query_stack.add().ParseFromString(instruction.SerializeToString())
    response = exchange(query.element.app, request)
    if response.WhichOneof("msg") != "element_query_response":
        raise RuntimeError("Invalid element query response")
    return [
        low.Element(query.element.app, low.Handle.FromString(h.SerializeToString()))
        for h in response.element_query_response.element_handles
    ]


def target(element, *, scroll=False, click=False):
    request = Request()
    part = (
        request.request_checked_click
        if click
        else request.request_scroll_into_view
        if scroll
        else request.request_pointer_target
    )
    part.element_handle.ParseFromString(element.handle.SerializeToString())
    response = exchange(element.app, request)
    if response.WhichOneof("msg") != "pointer_target_response":
        raise RuntimeError("Invalid pointer target response")
    target = response.pointer_target_response
    if target.status not in {
        "ready",
        "covered",
        "clipped",
        "disabled",
        "busy",
        "no-target",
        "unsupported",
    }:
        raise RuntimeError("Unknown pointer target status: " + target.status)
    if not target.HasField("position") or not all(
        math.isfinite(value) for value in (target.position.x, target.position.y)
    ):
        raise RuntimeError("Invalid pointer target position")
    if target.performed and (not click or target.status != "ready"):
        raise RuntimeError("Invalid checked click result")
    return {
        "status": target.status,
        "x": target.position.x,
        "y": target.position.y,
        "detail": target.detail,
        "scrollable": target.scrollable,
        "performed": target.performed,
    }
