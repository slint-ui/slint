# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from typing import Any, NotRequired, TypedDict


class WaitTimeout(AssertionError):
    def __init__(self, detail: str, *, actual: Any, observed: bool, timeout: float):
        super().__init__(detail)
        self.actual, self.observed, self.timeout = actual, observed, timeout


def display(value: Any) -> str:
    text = (
        value
        if isinstance(value, str) and value.strip() == value and value
        else repr(value)
    )
    text = text.replace("\n", "\\n").replace("\r", "\\r")
    return text if len(text) <= 100 else text[:99] + "…"


class AssertionDiagnostic(TypedDict):
    kind: str
    summary: str
    target: str
    expected: str
    actual: str | None
    observed: bool
    timeout_ms: float
    comparison: str
    location: NotRequired[dict[str, Any]]


class AssertionFailure(AssertionError):
    def __init__(
        self,
        expected: Any,
        failure: WaitTimeout,
        *,
        target: str,
        comparison: str = "equal",
    ):
        super().__init__(str(failure))
        wanted = display(expected)
        observed = display(failure.actual) if failure.observed else ""
        if (
            failure.observed
            and wanted == observed
            and type(expected) is not type(failure.actual)
        ):
            wanted = f"{display(repr(expected))} ({type(expected).__name__})"
            observed = (
                f"{display(repr(failure.actual))} ({type(failure.actual).__name__})"
            )
        expectation = (
            f"a value other than {wanted}" if comparison == "not_equal" else wanted
        )
        summary = (
            f"Expected {expectation}, observed {observed}"
            if failure.observed
            else f"Expected {expectation}; no value observed"
        )
        self.diagnostic: AssertionDiagnostic = {
            "kind": "assertion",
            "summary": summary,
            "target": target,
            "expected": repr(expected),
            "actual": repr(failure.actual) if failure.observed else None,
            "observed": failure.observed,
            "timeout_ms": failure.timeout,
            "comparison": comparison,
        }
