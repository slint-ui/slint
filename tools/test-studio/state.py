# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import copy
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
        elif kind == "environment":
            self.environment.update(event["data"])
        elif kind in ("error", "warning"):
            self.diagnostics.append(event)
        elif kind == "finished":
            self.state = "finished"
            self.exit_code = event["code"]
            self.cancelled = event.get("cancelled", False)
            for item in self.records.values():
                if item["status"] == "Running":
                    item["status"] = "Cancelled" if self.cancelled else "Error"
                elif item["status"] == "Queued":
                    item["status"] = "Not run"
        elif nodeid in self.records:
            item = self.records[nodeid]
            if kind == "test-start":
                item["status"] = "Running"
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
