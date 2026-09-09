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

    def wait_for_source(self, path: Path, expected: bytes, timeout: float = 15) -> None:
        self.request_id += 1
        request = {
            "id": self.request_id,
            "sources": {path.resolve().as_uri(): expected.decode("utf-8")},
        }
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
                    and last_response["ready"]
                ):
                    return
                if time.monotonic() >= deadline:
                    raise AssertionError(
                        f"Preview did not apply {path} within {timeout}s: "
                        f"{last_response!r}. Build with system-testing enabled."
                    )
                time.sleep(0.02)
        finally:
            (self.directory / "request.json").unlink(missing_ok=True)


current_editor_sync: ContextVar[EditorSync] = ContextVar("current_editor_sync")


def wait_for_source(path: Path, expected: bytes, timeout: float = 15) -> None:
    """Wait for the requested source in the installed preview and completed history work."""
    current_editor_sync.get().wait_for_source(path, expected, timeout)
