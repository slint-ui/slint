# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Private lifecycle waits for the real editor."""

import json
import time
from collections.abc import Iterator
from contextlib import contextmanager
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

PROTOCOL_VERSION = 4


class Process(Protocol):
    def poll(self) -> int | None: ...


@dataclass(frozen=True)
class SyncCheckpoint:
    session: str
    cursor: int
    writes: int
    accepted_edits: int
    operation: int | None = None

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)


@dataclass(frozen=True)
class SyncResult:
    data: dict[str, Any]

    @property
    def cursor(self) -> int:
        return self.data["cursor"]

    @property
    def operation(self) -> int | None:
        return self.data.get("operation")

    @property
    def outcome(self) -> str | None:
        return self.data.get("outcome")


@dataclass
class EditorSync:
    directory: Path
    request_id: int = 0
    session: str | None = None
    process: Process | None = None

    def _decode(self, response: object) -> dict[str, Any]:
        if not isinstance(response, dict):
            raise TypeError(f"malformed editor sync response: {response!r}")
        if response.get("error"):
            raise AssertionError(f"editor sync error: {response!r}")
        if response.get("protocol") != PROTOCOL_VERSION:
            raise AssertionError(f"unsupported editor sync protocol: {response!r}")
        session = response.get("session")
        if not isinstance(session, str) or not session:
            raise AssertionError(f"missing editor session: {response!r}")
        if self.session is not None and self.session != session:
            raise AssertionError(f"stale editor session: {response!r}")
        for name in ("id", "cursor", "writes", "accepted_edits"):
            if type(response.get(name)) is not int or response[name] < 0:
                raise AssertionError(f"invalid {name}: {response!r}")
        if type(response.get("ready")) is not bool:
            raise AssertionError(f"invalid ready state: {response!r}")
        if not isinstance(response.get("events"), list):
            raise TypeError(f"invalid event history: {response!r}")
        self.session = session
        return response

    def _request(
        self,
        *,
        mode: str,
        timeout: float = 15,
        deadline: float | None = None,
        after: SyncCheckpoint | int = 0,
        sources: dict[Path, bytes | None] | None = None,
        outcome: str | None = None,
        operation: int | None = None,
        gate: int | None = None,
        edit: int | None = None,
        kind: str | None = None,
        url: Path | None = None,
    ) -> SyncResult:
        deadline = time.monotonic() + timeout if deadline is None else deadline
        if self.session is None and mode != "handshake":
            self._request(mode="handshake", deadline=deadline)
        if isinstance(after, SyncCheckpoint):
            if after.session != self.session:
                raise AssertionError("checkpoint belongs to another editor session")
            after = after.cursor
        self.request_id += 1
        request = {
            "id": self.request_id,
            "protocol": PROTOCOL_VERSION,
            "session": self.session,
            "mode": mode,
            "after": after,
            "sources": {
                p.resolve().as_uri(): None if c is None else c.decode("utf-8")
                for p, c in (sources or {}).items()
            },
            "outcome": outcome,
            "operation": operation,
            "gate": gate,
            "edit": edit,
            "kind": kind,
            "url": None if url is None else url.resolve().as_uri(),
        }
        temporary = self.directory / "request.tmp"
        temporary.write_text(json.dumps(request))
        temporary.replace(self.directory / "request.json")
        last: object = None
        trace = self.directory / "trace.jsonl"
        with trace.open("a") as log:
            log.write(json.dumps({"request": request}) + "\n")
        try:
            while True:
                if self.process is not None and self.process.poll() is not None:
                    raise AssertionError(f"editor exited during {mode}: {last!r}")
                try:
                    raw = json.loads((self.directory / "response.json").read_text())
                except FileNotFoundError:
                    raw = None
                if raw is not None:
                    if not isinstance(raw, dict) or type(raw.get("id")) is not int:
                        raise AssertionError(f"malformed response: {raw!r}")
                    if raw["id"] == self.request_id:
                        response = self._decode(raw)
                        if response != last:
                            with trace.open("a") as log:
                                log.write(json.dumps({"response": response}) + "\n")
                        last = response
                        if response["ready"]:
                            return SyncResult(response)
                if time.monotonic() >= deadline:
                    raise AssertionError(f"editor sync did not reach {mode}: {last!r}")
                time.sleep(min(0.02, max(0, deadline - time.monotonic())))
        finally:
            (self.directory / "request.json").unlink(missing_ok=True)

    def checkpoint(self, timeout: float = 15) -> SyncCheckpoint:
        r = self._request(mode="checkpoint", timeout=timeout).data
        return SyncCheckpoint(
            r["session"], r["cursor"], r["writes"], r["accepted_edits"]
        )

    def wait_for_observed(
        self,
        path: Path,
        content: bytes | None,
        *,
        after: SyncCheckpoint | int,
        timeout: float = 15,
    ) -> SyncResult:
        return self._request(
            mode="observed", sources={path: content}, after=after, timeout=timeout
        )

    def wait_for_processed(
        self,
        path: Path,
        content: bytes | None,
        *,
        after: SyncCheckpoint | int = 0,
        outcome: str | None = None,
        timeout: float = 15,
    ) -> SyncResult:
        return self._request(
            mode="processed",
            sources={path: content},
            after=after,
            outcome=outcome,
            timeout=timeout,
        )

    def wait_for_source(
        self,
        path: Path,
        expected: bytes | None,
        timeout: float = 15,
        *,
        after: SyncCheckpoint | int = 0,
        deadline: float | None = None,
    ) -> SyncResult:
        return self._request(
            mode="applied",
            sources={path: expected},
            after=after,
            timeout=timeout,
            deadline=deadline,
        )

    wait_for_applied = wait_for_source

    @contextmanager
    def action(self, timeout: float = 15) -> Iterator["EditorAction"]:
        r = self._request(mode="begin", timeout=timeout).data
        action = EditorAction(self, r["operation"], timeout)
        try:
            yield action
        finally:
            action.seal()

    @contextmanager
    def gate(
        self, kind: str, path: Path, timeout: float = 15
    ) -> Iterator["EditorGate"]:
        r = self._request(mode="gate_open", kind=kind, url=path, timeout=timeout).data
        gate = EditorGate(self, r["gate"], timeout)
        try:
            yield gate
        finally:
            if self.process is None or self.process.poll() is None:
                gate.release()


@dataclass
class EditorAction:
    sync: EditorSync
    operation: int
    timeout: float
    sealed: bool = False

    def seal(self) -> None:
        if not self.sealed:
            self.sync._request(
                mode="seal", operation=self.operation, timeout=self.timeout
            )
            self.sealed = True

    def wait_for_settled(
        self, *, outcome: str | None = None, timeout: float | None = None
    ) -> SyncResult:
        if not self.sealed:
            raise AssertionError("close the input action before waiting for completion")
        return self.sync._request(
            mode="settled",
            operation=self.operation,
            outcome=outcome,
            timeout=self.timeout if timeout is None else timeout,
        )

    def assert_no_source_writes(self) -> None:
        result = self.wait_for_settled()
        state = result.data["operation_state"]
        assert state["writes"] == 0, f"action wrote source: {state!r}"
        assert state["accepted_edits"] == 0, f"action accepted an edit: {state!r}"


@dataclass
class EditorGate:
    sync: EditorSync
    id: int
    timeout: float

    def wait_for_reached(self) -> SyncResult:
        return self.sync._request(mode="gate_wait", gate=self.id, timeout=self.timeout)

    def release(self) -> None:
        self.sync._request(mode="gate_release", gate=self.id, timeout=self.timeout)


current_editor_sync: ContextVar[EditorSync] = ContextVar("current_editor_sync")


def wait_for_source(
    path: Path,
    expected: bytes | None,
    timeout: float = 15,
    *,
    deadline: float | None = None,
) -> None:
    current_editor_sync.get().wait_for_source(
        path, expected, timeout, deadline=deadline
    )
