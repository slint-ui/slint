# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import copy
import json
from dataclasses import dataclass, field
from typing import TypedDict

FAILURES = {"Failed", "Error", "Crashed"}


def record(item):
    return {**item, "status": "Not run", "steps": [], "output": "", "duration": 0.0}


@dataclass
class RunState:
    records: dict = field(default_factory=dict)
    diagnostics: list = field(default_factory=list)
    state: str = "collecting"
    exit_code: int | None = None
    cancelled: bool = False
    debug: dict = field(default_factory=dict)
    inspection: dict = field(default_factory=dict)
    environment: dict = field(default_factory=dict)

    def apply(self, event):
        kind = event["kind"]
        nodeid = event.get("nodeid")
        if kind == "collected":
            self.records[event["id"]] = record(event)
            if self.state == "running":
                self.records[event["id"]]["status"] = "Queued"
        elif kind == "state":
            self.state = event["state"]
            if self.state == "running":
                for item in self.records.values():
                    if item["status"] == "Not run":
                        item["status"] = "Queued"
        elif kind == "debug-state":
            self.debug = {**self.debug, **event}
        elif kind == "inspection":
            self.inspection = event
            self.environment["inspection_seconds"] = self.environment.get(
                "inspection_seconds", 0
            ) + event.get("capture_duration", 0)
        elif kind == "environment":
            self.environment.update(event["data"])
        elif kind in ("error", "warning"):
            self.diagnostics.append(event)
        elif kind == "finished":
            self.state = "finished"
            self.debug = {**self.debug, "paused": False}
            self.exit_code = event["code"]
            self.cancelled = event.get("cancelled", False)
            for item in self.records.values():
                for action in item["steps"]:
                    if action.get("status") == "Running":
                        action["status"] = "Cancelled" if self.cancelled else "Error"
                if item["status"] == "Running":
                    item["status"] = "Cancelled" if self.cancelled else "Error"
                elif item["status"] == "Queued":
                    item["status"] = "Not run"
        elif nodeid in self.records:
            item = self.records[nodeid]
            if kind == "test-start":
                item["status"] = "Running"
            elif kind == "action-start":
                parent = next(
                    (
                        a
                        for a in item["steps"]
                        if a.get("action_id") == event.get("parent_id")
                    ),
                    None,
                )
                item["steps"].append(
                    {
                        **event,
                        "status": "Running",
                        "duration": 0.0,
                        "screenshot": "",
                        "depth": parent.get("depth", 0) + 1 if parent else 0,
                    }
                )
            elif kind == "action-end":
                action = next(
                    (
                        a
                        for a in item["steps"]
                        if a.get("action_id") == event["action_id"]
                    ),
                    None,
                )
                if action is not None:
                    action.update(event)
            elif kind == "step":
                item["steps"].append(event)
                self.environment["capture_seconds"] = self.environment.get(
                    "capture_seconds", 0
                ) + event.get("capture_duration", 0)
            elif kind == "report":
                item["output"] += (
                    event.get("detail", "") + "\n" + event.get("output", "")
                )
            elif kind == "test-end":
                item.update(
                    status=event["status"],
                    duration=event["duration"],
                    strict_xpass=event.get("strict_xpass", False),
                )
            elif kind == "application-crash":
                item["status"] = "Crashed"
                item["output"] += (
                    f"\nApplication exited unexpectedly: {event['returncode']}\n"
                )

    def snapshot(self):
        return copy.deepcopy(self.__dict__)


def matching(records, query="", marker="", outcome="All"):
    query, marker = query.casefold().strip(), marker.casefold().strip()
    return [
        nodeid
        for nodeid, item in records.items()
        if query in f"{nodeid} {item['title']}".casefold()
        and (not marker or any(marker in m.casefold() for m in item.get("markers", [])))
        and (
            outcome == "All"
            or (
                outcome == "Failures"
                and (item["status"] in FAILURES or item.get("strict_xpass", False))
            )
            or item["status"] == outcome
        )
    ]


class TreeNode(TypedDict):
    id: str
    title: str
    depth: int
    ancestors: list[str]
    members: list[str]


def tree_rows(records, ids, collapsed):
    groups: dict[str, TreeNode] = {}
    leaves: dict[str, TreeNode] = {}
    for nodeid in ids:
        item = records[nodeid]
        ancestors = []
        for group in item.get("groups", []):
            key, title = group["id"], group["title"]
            groups.setdefault(
                key,
                {
                    "id": key,
                    "title": title,
                    "depth": len(ancestors),
                    "ancestors": list(ancestors),
                    "members": [],
                },
            )["members"].append(nodeid)
            ancestors.append(key)
        leaves[nodeid] = {
            "id": nodeid,
            "title": item.get("case") or item["title"],
            "depth": len(ancestors),
            "ancestors": ancestors,
            "members": [nodeid],
        }
    rows, seen = [], set()
    for nodeid in ids:
        for key in leaves[nodeid]["ancestors"] + [nodeid]:
            if key in seen:
                continue
            seen.add(key)
            row = groups[key] if key in groups else leaves[key]
            if any(a in collapsed for a in row["ancestors"]):
                continue
            statuses = [records[n]["status"] for n in row["members"]]
            status = next(
                (
                    s
                    for s in ("Running", "Crashed", "Error", "Failed", "Queued")
                    if s in statuses
                ),
                statuses[0] if len(set(statuses)) == 1 else "Mixed",
            )
            rows.append(
                {
                    **row,
                    "group": key in groups,
                    "status": status,
                    "expanded": key not in collapsed,
                }
            )
    return rows


def rerun_selection(run_records, current_records):
    failed = [
        n
        for n, item in run_records.items()
        if item["status"] in FAILURES
        or (item["status"] == "Unexpected pass" and item.get("strict_xpass", False))
    ]
    return (
        [n for n in failed if n in current_records],
        [n for n in failed if n not in current_records],
    )


def action_rows(steps, collapsed):
    by_id = {s.get("action_id"): s for s in steps if s.get("action_id")}
    parents = {s.get("parent_id") for s in steps if s.get("parent_id")}
    rows = []
    for index, action in enumerate(steps):
        ancestor = action.get("parent_id")
        hidden = False
        visited = set()
        while ancestor and ancestor not in visited:
            visited.add(ancestor)
            if ancestor in collapsed:
                hidden = True
                break
            ancestor = by_id.get(ancestor, {}).get("parent_id")
        if not hidden:
            rows.append(
                {
                    **action,
                    "row_index": index,
                    "group": action.get("action_id") in parents,
                    "expanded": action.get("action_id") not in collapsed,
                }
            )
    return rows


def action_details(action):
    lines = (
        [action["diagnostic"]["summary"], action["diagnostic"].get("target", ""), ""]
        if action.get("diagnostic")
        else []
    ) + [
        f"Action: {action['title']}",
        f"Status: {action['status']}",
        f"Layer: {action.get('layer', 'generic')}",
        f"Duration: {action.get('duration', 0):.3f}s",
    ]
    source = action.get("source", {})
    if source:
        lines.extend(
            [
                f"Source: {source.get('file', '')}:{source.get('line', '')}",
                source.get("text", ""),
            ]
        )
    arguments = action.get("arguments", {})
    if arguments:
        lines.extend(["", "Arguments", json.dumps(arguments, indent=2)])
    for key in ("expected", "actual", "target_bounds", "detail", "warning"):
        if key in action:
            lines.extend(["", f"{key.replace('_', ' ').capitalize()}: {action[key]}"])
    if action.get("hit_target_verified") is False:
        lines.extend(["", "Hit target is unverified by this transport."])
    return "\n".join(lines)


def failure_presentation(state):
    action = state.get("action", {})
    diagnostic = action.get("diagnostic", {})
    error = state.get("error", "")
    if error and diagnostic.get("kind") == "assertion":
        summary = diagnostic["summary"]
        context = diagnostic.get("target", "")
        duration = diagnostic.get("timeout_ms")
        if duration is not None:
            context += (
                f" · observation window {duration:g} ms"
                if diagnostic.get("comparison") == "remain"
                else f" · timeout {duration:g} ms"
            )
    else:
        summary = (
            "Assertion failed"
            if error and action.get("layer") == "assertion"
            else "Action failed"
            if error
            else f"{state.get('reason', '')}: {action.get('title', '')}"
        )
        context = action.get("arguments", {}).get("target", "")
    source = action.get("source", {})
    location = source.get("file", "").replace("\\", "/").rsplit("/", 1)[-1]
    if location:
        location += f":{source.get('line', '')}"
    details = "\n\n".join(
        part
        for part in (
            error,
            f"{source.get('file', '')}:{source.get('line', '')}" if source else "",
            json.dumps(action.get("arguments", {}), indent=2),
        )
        if part
    )
    return {
        "summary": summary,
        "context": context,
        "source": location + "\n" + source.get("text", "") if source else "",
        "details": details,
    }
