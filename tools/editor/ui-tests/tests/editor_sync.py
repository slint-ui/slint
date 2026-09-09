# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import time
from contextlib import contextmanager
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path


@dataclass
class EditorSync:
    directory: Path
    request_id: int = 0
    session: str | None = None

    def _request(
        self,
        *,
        mode: str,
        sources: dict[Path, bytes] | None = None,
        after: int = 0,
        outcome: str | None = None,
        timeout: float = 15,
    ) -> dict:
        self.request_id += 1
        request = {
            "id": self.request_id,
            "mode": mode,
            "after": after,
            "sources": {
                path.resolve().as_uri(): content.decode("utf-8")
                for path, content in (sources or {}).items()
            },
        }
        if outcome is not None:
            request["outcome"] = outcome
        temporary = self.directory / "request.tmp"
        temporary.write_text(json.dumps(request))
        temporary.replace(self.directory / "request.json")
        deadline = time.monotonic() + timeout
        last_response = None
        try:
            while True:
                try:
                    last_response = json.loads(
                        (self.directory / "response.json").read_text()
                    )
                except FileNotFoundError:
                    pass
                if last_response is not None and last_response.get("id") == self.request_id:
                    if "protocol" in last_response and last_response["protocol"] != 2:
                        raise AssertionError(
                            f"unsupported editor sync protocol: {last_response!r}"
                        )
                    response_session = last_response.get("session")
                    if response_session is not None:
                        if self.session is None:
                            self.session = response_session
                        elif self.session != response_session:
                            raise AssertionError(
                                "editor sync session changed while waiting: "
                                f"{self.session!r} -> {response_session!r}"
                            )
                    if last_response.get("overflow"):
                        raise AssertionError(
                            f"editor sync event history overflowed after cursor {after}: {last_response!r}"
                        )
                    if last_response.get("ready"):
                        return last_response
                if time.monotonic() >= deadline:
                    raise AssertionError(
                        f"Editor synchronization did not reach {mode} within {timeout}s: "
                        f"{last_response!r}. Build with system-testing enabled."
                    )
                time.sleep(0.02)
        finally:
            (self.directory / "request.json").unlink(missing_ok=True)

    def checkpoint(self, timeout: float = 15) -> dict:
        return self._request(mode="checkpoint", timeout=timeout)

    def wait_for_processed(
        self,
        path: Path,
        expected: bytes,
        *,
        after: int = 0,
        outcome: str | None = None,
        timeout: float = 15,
    ) -> dict:
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
        expected: bytes,
        timeout: float = 15,
        *,
        after: int = 0,
    ) -> None:
        self._request(
            mode="applied", sources={path: expected}, after=after, timeout=timeout
        )

    @contextmanager
    def action(self, timeout: float = 15):
        checkpoint = self.checkpoint(timeout)
        yield EditorAction(self, checkpoint, timeout)


@dataclass
class EditorAction:
    sync: EditorSync
    checkpoint: dict
    timeout: float

    @property
    def writes_before(self) -> int:
        return int(self.checkpoint.get("writes", 0))

    def wait_for_settled(self, path: Path, expected: bytes) -> dict:
        return self.sync._request(
            mode="settled",
            sources={path: expected},
            after=int(self.checkpoint["cursor"]),
            timeout=self.timeout,
        )

    def assert_no_source_writes(self, timeout: float = 0.2) -> None:
        current = self.sync.checkpoint(timeout)
        assert int(current.get("writes", 0)) == self.writes_before, (
            f"action wrote source files: before={self.writes_before}, "
            f"after={current.get('writes')}"
        )


current_editor_sync: ContextVar[EditorSync] = ContextVar("current_editor_sync")


def wait_for_source(path: Path, expected: bytes, timeout: float = 15) -> None:
    """Wait for the requested source in the installed preview and completed history work."""
    current_editor_sync.get().wait_for_source(path, expected, timeout)
