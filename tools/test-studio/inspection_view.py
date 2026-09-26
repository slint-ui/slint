# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json

from slint_syntax import assignment_line

PROPERTY_KEYS = {
    "accessible_value": "value",
    "accessible_label": "name",
    "accessible_enabled": "enabled",
    "accessible_checked": "checked",
    "accessible_item_selected": "selected",
    "accessible_read_only": "read_only",
}


def failure_location(state):
    diagnostic = state.get("action", {}).get("diagnostic", {})
    return diagnostic.get("location", {}) if diagnostic.get("observed") else {}


def property_view(element, location):
    text = (
        (
            element.get("locator")
            or "No verified unique locator. Add a unique name or scope."
        )
        + "\n\n"
        + json.dumps(element, indent=2)
    )
    key = PROPERTY_KEYS.get(location.get("property"))
    line = 0
    if key and location.get("handle") and location["handle"] == element.get("handle"):
        line = next(
            (
                i
                for i, row in enumerate(text.splitlines(), 1)
                if row.startswith(f'  "{key}":')
            ),
            0,
        )
    return text, line


def source_view(inspection, location):
    reference = location.get("source", {})
    documents = inspection.get("sources", [])
    matches = [doc for doc in documents if doc.get("path") == reference.get("path")]
    document = (
        matches[0] if len(matches) == 1 else (documents[0] if documents else None)
    )
    if document is None:
        return (
            inspection.get(
                "source_text", "No source supplied by this application adapter."
            ),
            "Saved source",
            0,
        )
    text = document["text"]
    line = (
        assignment_line(
            text, reference.get("element", ""), reference.get("property", "")
        )
        if len(matches) == 1 and not document.get("truncated")
        else 0
    )
    return text, document["name"] + " · captured at pause", line
