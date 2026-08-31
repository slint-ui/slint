# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from dataclasses import dataclass
from difflib import unified_diff
from pathlib import Path


def slint_sources(project: Path) -> dict[Path, bytes]:
    return {
        path.relative_to(project): path.read_bytes()
        for path in sorted(project.rglob("*.slint"))
    }


def exact_source_mismatch(
    current: dict[Path, bytes], expected: dict[Path, bytes]
) -> str:
    differences = []
    for path in sorted(current.keys() | expected.keys()):
        actual = current.get(path, b"").decode(errors="backslashreplace").splitlines()
        wanted = expected.get(path, b"").decode(errors="backslashreplace").splitlines()
        if actual == wanted:
            continue
        differences.extend(
            unified_diff(
                wanted,
                actual,
                fromfile=f"expected/{path}",
                tofile=f"actual/{path}",
                lineterm="",
            )
        )
    return "exact .slint source mismatch:\n" + "\n".join(differences)


@dataclass(frozen=True)
class SourceSnapshot:
    project: Path
    sources: dict[Path, bytes]

    @classmethod
    def capture(cls, project: Path) -> "SourceSnapshot":
        return cls(project=project, sources=slint_sources(project))

    def assert_unchanged_now(self) -> None:
        current = slint_sources(self.project)
        assert current == self.sources, exact_source_mismatch(current, self.sources)

    def assert_unchanged(
        self, quiescence: float = 0.25, poll_interval: float = 0.02
    ) -> None:
        deadline = time.monotonic() + quiescence
        while True:
            self.assert_unchanged_now()
            if time.monotonic() >= deadline:
                return
            time.sleep(poll_interval)

    def wait_for_exact(
        self,
        expected: bytes,
        relative_path: Path | str = "Main.slint",
        timeout: float = 5,
        poll_interval: float = 0.02,
    ) -> None:
        relative_path = Path(relative_path)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            current = slint_sources(self.project)
            expected_sources = self.sources | {relative_path: expected}
            if current == expected_sources:
                return
            time.sleep(poll_interval)

        current = slint_sources(self.project)
        expected_sources = self.sources | {relative_path: expected}
        assert current == expected_sources, exact_source_mismatch(
            current, expected_sources
        )
