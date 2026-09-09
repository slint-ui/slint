# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Revision-specific synchronization for headless editor tests."""

import json
import time
from contextlib import contextmanager
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import Any

PROTOCOL_VERSION = 2


@dataclass(frozen=True)
class SyncCheckpoint:
    session: str
    cursor: int
    writes: int
    accepted_edits: int
    operation: int | None = None

    def __getitem__(self, key: str):
        return getattr(self, key)


@dataclass(frozen=True)
class SyncResult:
    session: str
    cursor: int
    writes: int
    accepted_edits: int
    events: tuple[dict, ...]
    operation: int | None = None


@dataclass
class EditorSync:
    directory: Path
    request_id: int = 0
    session: str | None = None
    process: Any = None

    def _write_request(self, request: dict) -> None:
        temporary = self.directory / "request.tmp"
        temporary.write_text(json.dumps(request))
        temporary.replace(self.directory / "request.json")

    def _decode(self, response: object, expected_id: int) -> SyncResult:
        if not isinstance(response, dict):
            raise TypeError(f"malformed editor sync response: {response!r}")
        if response.get("id") != expected_id:
            raise AssertionError(f"stale editor sync response: {response!r}")
        if response.get("error"):
            raise AssertionError(f"editor sync error: {response['error']}")
        if response.get("protocol") != PROTOCOL_VERSION:
            raise AssertionError(f"unsupported editor sync protocol: {response!r}")
        session = response.get("session")
        if not isinstance(session, str) or not session:
            raise AssertionError(f"editor sync response has no session: {response!r}")
        if self.session is None:
            self.session = session
        elif self.session != session:
            raise AssertionError(
                f"editor sync session changed: {self.session!r} -> {session!r}"
            )
        cursor = response.get("cursor")
        writes = response.get("writes")
        accepted_edits = response.get("accepted_edits", 0)
        if not all(
            isinstance(value, int) and value >= 0
            for value in (cursor, writes, accepted_edits)
        ):
            raise TypeError(f"invalid editor sync counters: {response!r}")
        if (
            not isinstance(cursor, int)
            or not isinstance(writes, int)
            or not isinstance(accepted_edits, int)
        ):
            raise TypeError(f"invalid editor sync counters: {response!r}")
        operation = response.get("operation")
        if operation is not None and (not isinstance(operation, int) or operation < 1):
            raise AssertionError(f"invalid editor sync operation: {response!r}")
        return SyncResult(
            session,
            cursor,
            writes,
            accepted_edits,
            tuple(response.get("events", ())),
            operation,
        )

    def _request(
        self,
        *,
        mode: str,
        sources: dict[Path, bytes | None] | None = None,
        after: int = 0,
        outcome: str | None = None,
        operation: int | None = None,
        begin_action: bool = False,
        gate: str | None = None,
        release: bool = False,
        gate_control: bool = False,
        timeout: float = 15,
        handshake: bool = False,
    ) -> SyncResult:
        if not handshake and self.session is None:
            self._request(mode="handshake", timeout=timeout, handshake=True)
        self.request_id += 1
        request = {
            "id": self.request_id,
            "protocol": PROTOCOL_VERSION,
            "session": None if handshake else self.session,
            "mode": mode,
            "after": after,
            "operation": operation,
            "outcome": outcome,
            "sources": {
                path.resolve().as_uri(): (
                    None if content is None else content.decode("utf-8")
                )
                for path, content in (sources or {}).items()
            },
            "gate": gate,
            "release": release,
            "gate_control": gate_control,
            "begin_action": begin_action,
        }
        self._write_request(request)
        deadline = time.monotonic() + timeout
        last_response: object = None
        try:
            while True:
                try:
                    last_response = json.loads(
                        (self.directory / "response.json").read_text()
                    )
                except FileNotFoundError:
                    pass
                if (
                    isinstance(last_response, dict)
                    and last_response.get("id") == self.request_id
                ):
                    if last_response.get("ready"):
                        return self._decode(last_response, self.request_id)
                    if last_response.get("error"):
                        self._decode(last_response, self.request_id)
                if self.process is not None and self.process.poll() is not None:
                    raise AssertionError(
                        f"editor exited while waiting for {mode}: {last_response!r}"
                    )
                if time.monotonic() >= deadline:
                    raise AssertionError(
                        f"editor sync did not reach {mode} within {timeout}s: {last_response!r}"
                    )
                time.sleep(0.02)
        finally:
            (self.directory / "request.json").unlink(missing_ok=True)

    def checkpoint(self, timeout: float = 15) -> SyncCheckpoint:
        result = self._request(mode="checkpoint", timeout=timeout)
        return SyncCheckpoint(
            result.session,
            result.cursor,
            result.writes,
            result.accepted_edits,
            result.operation,
        )

    def begin_action(self, timeout: float = 15) -> SyncCheckpoint:
        result = self._request(mode="checkpoint", timeout=timeout, begin_action=True)
        return SyncCheckpoint(
            result.session,
            result.cursor,
            result.writes,
            result.accepted_edits,
            result.operation,
        )

    def wait_for_processed(
        self,
        path: Path,
        expected: bytes | None,
        *,
        after: int = 0,
        outcome: str | None = None,
        timeout: float = 15,
    ) -> SyncResult:
        return self._request(
            mode="processed",
            sources={path: expected},
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
        after: int = 0,
    ) -> SyncResult:
        return self._request(
            mode="applied", sources={path: expected}, after=after, timeout=timeout
        )

    wait_for_applied = wait_for_source

    def set_gate(self, gate: str, timeout: float = 15) -> SyncResult:
        """Enable a named scheduling gate without waiting for the held stage."""
        if gate not in {"source", "publication"}:
            raise ValueError(f"unknown editor sync gate: {gate}")
        return self._request(mode="gate", gate=gate, gate_control=True, timeout=timeout)

    def wait_for_gate(
        self, gate: str, *, after: int = 0, timeout: float = 15
    ) -> SyncResult:
        """Wait until the named gate has observed the requested operation."""
        return self._request(mode="gate", gate=gate, after=after, timeout=timeout)

    def release_gate(self, gate: str, timeout: float = 15) -> SyncResult:
        """Release a scheduling gate; the normal event loop drains held work."""
        return self._request(
            mode="gate", gate=gate, release=True, gate_control=True, timeout=timeout
        )

    @contextmanager
    def action(self, timeout: float = 15):
        yield EditorAction(self, self.begin_action(timeout), timeout)


@dataclass
class EditorAction:
    sync: EditorSync
    checkpoint: SyncCheckpoint
    timeout: float
    finished: bool = False

    def complete(self, outcome: str) -> SyncResult:
        result = self.sync._request(
            mode="finish",
            after=self.checkpoint.cursor,
            outcome=outcome,
            operation=self.checkpoint.operation,
            timeout=self.timeout,
        )
        self.finished = True
        return result

    def wait_for_settled(
        self, *, outcome: str, timeout: float | None = None
    ) -> SyncResult:
        if self.checkpoint.operation is None:
            raise AssertionError("action checkpoint has no operation")
        if not self.finished:
            self.complete(outcome)
        return self.sync._request(
            mode="settled",
            after=self.checkpoint.cursor,
            outcome=outcome,
            operation=self.checkpoint.operation,
            timeout=self.timeout if timeout is None else timeout,
        )

    def assert_no_source_writes(self) -> None:
        current = self.sync.checkpoint(self.timeout)
        if current.writes != self.checkpoint.writes:
            raise AssertionError(
                f"action wrote source files: before={self.checkpoint.writes}, after={current.writes}"
            )


current_editor_sync: ContextVar[EditorSync] = ContextVar("current_editor_sync")


def wait_for_source(path: Path, expected: bytes | None, timeout: float = 15) -> None:
    """Wait for the requested source revision in the installed preview."""
    current_editor_sync.get().wait_for_source(path, expected, timeout)
