# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from source_snapshot import SourceSnapshot
from ui_assertions import expect
from ui_driver import element, elements, first_window, launch_editor, query, screenshot


@pytest.mark.parametrize("dark", [False, True], ids=["light", "dark"])
@pytest.mark.parametrize(
    ("scenario", "state", "description"),
    [
        ("Current", None, ""),
        ("Warnings", "Preview warnings", "warnings"),
        ("ManyWarnings", "Preview warnings", "warnings"),
        ("Stale", "Stale preview", "errors"),
        ("Unavailable", "Preview unavailable", "errors"),
        ("NoComponent", "No component to preview", ""),
        ("ManyErrors", "Stale preview", "errors"),
        ("Imports", "Stale preview", "errors"),
    ],
)
def test_diagnostics_gallery_uses_preview_only_mock_data(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
    dark: bool,
    scenario: str,
    state: str | None,
    description: str,
) -> None:
    gallery = Path(__file__).resolve().parents[2] / "gallery" / "diagnostics.slint"
    source = tmp_path / "Mock.slint"
    source.write_text(
        f'import {{ DiagnosticsGalleryWindow, DiagnosticMockScenario }} from "{gallery}";\n'
        "export component Main inherits DiagnosticsGalleryWindow {\n"
        f"    scenario: DiagnosticMockScenario.{scenario};\n"
        f"    dark-theme: {'true' if dark else 'false'};\n"
        "}\n"
    )
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        pane = element(
            window, "Diagnostics mock gallery", role=slint_testing.AccessibleRole.Region
        )
        row = element(pane, "/mock/project", role=slint_testing.AccessibleRole.ListItem)
        expect.poll(
            lambda: (
                description in row.accessible_description
                if description
                else row.accessible_description == ""
            )
        ).to_equal(True)
        if state:
            card = element(pane, state, role=slint_testing.AccessibleRole.Region)
        else:
            expect(query(pane, "Stale preview")).to_be_hidden()
            expect(query(pane, "Preview unavailable")).to_be_hidden()
        if scenario in {"Stale", "Unavailable", "ManyErrors", "Imports"}:
            title = "components/Card.slint" if scenario == "Imports" else "Main.slint"
            element(card, title, role=slint_testing.AccessibleRole.Text)
            message = (
                "An error in this file is preventing the preview from being displayed."
                if scenario == "Unavailable"
                else "An error in this file is preventing the preview from updating. Showing the last working preview."
            )
            element(card, message, role=slint_testing.AccessibleRole.Text)
            copy = query(
                card,
                "Copy preview diagnostics",
                role=slint_testing.AccessibleRole.Button,
            )
            expect(copy).to_be_hidden()
            advanced = element(
                card,
                "Toggle advanced error details",
                role=slint_testing.AccessibleRole.Button,
            )
            advanced.invoke_accessible_default_action()
            expect(copy).to_be_visible()
            advanced.invoke_accessible_default_action()
            expect(copy).to_be_hidden()
            advanced.invoke_accessible_default_action()
            expect(copy).to_be_visible()
            expect(
                query(pane, "Open in editor", role=slint_testing.AccessibleRole.Button)
            ).to_be_hidden()
        if scenario == "Imports":
            folder = element(
                pane,
                "/mock/project/components",
                role=slint_testing.AccessibleRole.ListItem,
            )
            expect.poll(lambda: "errors" in folder.accessible_description).to_equal(
                True
            )
            element(
                pane,
                "components/Card.slint:12:9",
                role=slint_testing.AccessibleRole.Text,
            )
        if scenario == "ManyErrors":
            element(
                pane,
                "2 more errors. Copy diagnostics to see all messages.",
                role=slint_testing.AccessibleRole.Text,
            )
        if scenario in {"Stale", "Unavailable", "ManyErrors", "Imports"}:
            element(
                pane,
                "Copy preview diagnostics",
                role=slint_testing.AccessibleRole.Button,
            ).invoke_accessible_default_action()
            element(pane, "Copies: 1", role=slint_testing.AccessibleRole.Text)
        if scenario in {"Warnings", "ManyWarnings"}:
            warnings = element(
                pane, "Preview warnings", role=slint_testing.AccessibleRole.Region
            )
            element(
                warnings,
                "Toggle preview warning details",
                role=slint_testing.AccessibleRole.Button,
            ).invoke_accessible_default_action()
            count = 3 if scenario == "ManyWarnings" else 1
            expect.poll(
                lambda: len(
                    elements(
                        warnings,
                        "Main.slint:1:17",
                        role=slint_testing.AccessibleRole.Text,
                    )
                )
            ).to_equal(count)
            expect.poll(
                lambda: len(
                    elements(
                        warnings,
                        "Component declaration syntax is deprecated",
                        role=slint_testing.AccessibleRole.Text,
                    )
                )
            ).to_equal(count)
            if scenario == "ManyWarnings":
                element(
                    warnings,
                    "2 more warnings. Copy diagnostics to see all messages.",
                    role=slint_testing.AccessibleRole.Text,
                )
            element(
                warnings,
                "Copy preview diagnostics",
                role=slint_testing.AccessibleRole.Button,
            ).invoke_accessible_default_action()
            element(pane, "Copies: 1", role=slint_testing.AccessibleRole.Text)
        screenshot(window).save(
            tmp_path / f"diagnostics-{scenario}-{'dark' if dark else 'light'}.png"
        )
        snapshot.assert_unchanged()
