# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import time
from collections import Counter
from typing import Any

import slint_testing as low

from .core import BoundApplication, Session


def capture(
    application: low.Application, *, limit: int = 1500
) -> tuple[dict[str, Any], bytes]:
    """Inspect visible elements in the first window; bounds do not prove hit targets."""
    session = Session(process=application.process)
    transport = BoundApplication(application, session)
    window = transport.first_window
    if window is None:
        raise RuntimeError("No application window is available")
    props = window._get_props()
    scale = props.scale_factor or 1
    root = window.root_element
    elements = root.query_descendants().find_all()
    rows: list[dict[str, Any]] = []
    ids: Counter[str] = Counter()
    deadline = time.monotonic() + 5
    for element in elements[:limit]:
        if time.monotonic() > deadline:
            break
        p = element._get_props()
        names = list(p.type_names_and_ids)
        ids.update({name.id for name in names if name.id})
        role = low.AccessibleRole(p.accessible_role).name
        rows.append(
            {
                "index": len(rows),
                "handle": {
                    "index": element.handle.index,
                    "generation": element.handle.generation,
                },
                "id": names[0].id if names else "",
                "type": names[0].type_name if names else "",
                "role": role,
                "name": p.accessible_label,
                "value": p.accessible_value,
                "enabled": p.accessible_enabled,
                "read_only": p.accessible_read_only,
                "selected": p.accessible_item_selected,
                "checked": p.accessible_checked,
                "opacity": p.computed_opacity,
                "bounds": {
                    "x": p.absolute_position.x,
                    "y": p.absolute_position.y,
                    "width": p.size.width,
                    "height": p.size.height,
                },
            }
        )
    complete = len(rows) == len(elements)
    roles = Counter((row["role"], row["name"]) for row in rows)
    for row in rows:
        suggestion = ""
        if complete and row["id"] and ids[row["id"]] == 1:
            suggestion = f"window.get_by_id({row['id']!r})"
        if (
            not suggestion
            and complete
            and row["role"] != "Unknown"
            and roles[(row["role"], row["name"])] == 1
        ):
            suggestion = (
                f"window.get_by_role({row['role']!r}, name={row['name']!r}, exact=True)"
            )
        row["locator"] = suggestion
    return {
        "elements": rows,
        "truncated": not complete,
        "width": props.size.width / scale,
        "height": props.size.height / scale,
    }, window.grab_window_as_png()
