# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import time
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path


@dataclass
class EditorSync:
    directory: Path
    request_id: int = 0

    def _round_trip(
        self,
        sources: dict[str, str],
        timeout: float,
        require_ready: bool,
        failure: str,
    ) -> None:
        self.request_id += 1
        request = {"id": self.request_id, "sources": sources}
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
                if (
                    last_response is not None
                    and last_response["id"] == self.request_id
                    and (last_response["ready"] or not require_ready)
                ):
                    return
                if time.monotonic() >= deadline:
                    raise AssertionError(
                        f"{failure} within {timeout}s: "
                        f"{last_response!r}. Build with system-testing enabled."
                    )
                time.sleep(0.005)
        finally:
            (self.directory / "request.json").unlink(missing_ok=True)

    def wait_for_source(self, path: Path, expected: bytes, timeout: float = 15) -> None:
        self._round_trip(
            {path.as_uri(): expected.decode("utf-8")},
            timeout,
            require_ready=True,
            failure=f"Preview did not apply {path}",
        )

    def settle(self, timeout: float = 5) -> None:
        if not (self.directory / "response.json").exists():
            return
        self._round_trip(
            {}, timeout, require_ready=False, failure="Editor did not respond"
        )


current_editor_sync: ContextVar[EditorSync] = ContextVar("current_editor_sync")


def wait_for_source(path: Path, expected: bytes, timeout: float = 15) -> None:
    """Wait for the requested source in the installed preview and completed history work."""
    current_editor_sync.get().wait_for_source(path, expected, timeout)


def settle() -> None:
    """Wait for the editor to run its expired timers and deferred `changed` handlers.

    Slint runs both from its event loop, not while it serves system-testing requests,
    so a test can otherwise act before a popup is placed or a field takes focus.
    Does nothing until the editor initializes its preview, such as on the startup page.
    """
    sync = current_editor_sync.get(None)
    if sync is not None:
        sync.settle()
