# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    screenshot,
)


def canvas_source(root: Path) -> tuple[Path, bytes]:
    source = root / "Main.slint"
    original = b"export component Main inherits Window { background: #47a87b; }"
    source.write_bytes(original)
    return source, original


def test_canvas_presets_follow_project_file_changes(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    source, original = canvas_source(tmp_path)
    project_file = tmp_path / "slint-project.json"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
        expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
            (390, 720)
        )
        assert not project_file.exists()
        for width, height in [(480, 320), (240, 180), (300, 200)]:
            project_file.write_text(
                json.dumps(
                    {
                        "slint-edition": "1.19",
                        "metadata": {
                            "visual-editor": {
                                "canvas-presets": [{"width": width, "height": height}]
                            }
                        },
                    }
                )
            )
            expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
                (width, height)
            )
            assert source.read_bytes() == original
            project_file.unlink()
            expect(
                element(
                    window,
                    "Canvas settings status",
                    role=slint_testing.AccessibleRole.Text,
                )
            ).to_have_value("No project file. Settings are not saved.")
            expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
                (width, height)
            )


def type_preset_dimension(window: slint_testing.Window, label: str, value: str) -> None:
    field = element(window, label, role=slint_testing.AccessibleRole.TextInput)
    field.single_click(slint_testing.PointerEventButton.Left)
    press_shortcut(window, keys.Control, "a")
    press_keys(window, value)
    expect(field).to_have_value(value)


def enter_preset_dimension(
    window: slint_testing.Window, label: str, value: str, key: str = keys.Return
) -> None:
    type_preset_dimension(window, label, value)
    press_key(window, key)


def wait_for_project_file(project_file: Path, expected: bytes) -> None:
    expect.poll(project_file.read_bytes).to_equal(expected)
    wait_for_source(project_file, expected)
    assert project_file.read_bytes() == expected


def test_canvas_preset_inspector_edits_and_selection_are_persistent(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    source, original = canvas_source(tmp_path)
    project_file = tmp_path / "slint-project.json"
    prefix = b'{\r\n  "slint-edition": "1.19",\r\n  "main": "Main.slint", // keep\r\n  "metadata": {"other-tool": [1,2], "visual-editor" /*keep*/: '
    suffix = b', /*keep*/ "another-tool": {"x":true}}\r\n}\r\n'
    presets = [{"width": 480, "height": 320}, {"width": 600, "height": 400}]

    def project_bytes() -> bytes:
        return (
            prefix
            + json.dumps({"canvas-presets": presets}, separators=(",", ":")).encode()
            + suffix
        )

    project_file.write_bytes(
        prefix
        + json.dumps({"manual": "discard", "canvas-presets": presets}).encode()
        + suffix
    )
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        status = element(
            window, "Canvas settings status", role=slint_testing.AccessibleRole.Text
        )
        expect(status).to_have_value("Project file: slint-project.json")
        enter_preset_dimension(window, "Canvas preset 1 width", "520")
        presets[0]["width"] = 520
        wait_for_project_file(project_file, project_bytes())
        expect(
            element(
                window,
                "Canvas preset 1 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("520")
        enter_preset_dimension(window, "Canvas preset 1 height", "360", keys.Tab)
        presets[0]["height"] = 360
        wait_for_project_file(project_file, project_bytes())
        baseline = project_file.read_bytes()
        for invalid in ["0", "-1", "1.5", "oops"]:
            enter_preset_dimension(window, "Canvas preset 1 width", invalid)
            expect(
                element(
                    window,
                    "Canvas preset 1 width",
                    role=slint_testing.AccessibleRole.TextInput,
                )
            ).to_have_value("520")
            assert project_file.read_bytes() == baseline
        element(
            window, "Add preset", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        presets.append({"width": 390, "height": 720})
        wait_for_project_file(project_file, project_bytes())
        expect(
            element(
                window,
                "Canvas preset 3 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("390")
        row = element(
            window, "Canvas preset 2", role=slint_testing.AccessibleRole.Region
        )
        element(
            row, "2", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
        expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
            (600, 400)
        )
        assert project_file.read_bytes() == project_bytes()
        screenshot(window).save(tmp_path / "canvas-presets-inspector.png")
        assert source.read_bytes() == original
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
        expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
            (600, 400)
        )
        presets[1] = {"width": 640, "height": 420}
        project_file.write_bytes(project_bytes())
        wait_for_project_file(project_file, project_bytes())
        expect(
            element(
                window,
                "Canvas preset 2 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("640")
        expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
            (640, 420)
        )
        type_preset_dimension(window, "Canvas preset 2 width", "999")
        comment = b"// external comment\n"
        updated = project_bytes() + comment
        project_file.write_bytes(updated)
        wait_for_project_file(project_file, updated)
        expect(
            element(
                window,
                "Canvas preset 2 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("999")
        press_key(window, keys.Return)
        presets[1]["width"] = 999
        wait_for_project_file(project_file, project_bytes() + comment)
        type_preset_dimension(window, "Canvas preset 1 width", "888")
        presets[0]["width"] = 530
        updated = project_bytes() + comment
        project_file.write_bytes(updated)
        wait_for_project_file(project_file, updated)
        expect(
            element(
                window,
                "Canvas preset 1 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("530")
        wait_for_project_file(project_file, updated)
        press_key(window, keys.Return)
        assert project_file.read_bytes() == updated


def test_canvas_preset_inspector_without_project_file_stays_in_memory(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    source, original = canvas_source(tmp_path)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        expect(
            element(
                window, "Canvas settings status", role=slint_testing.AccessibleRole.Text
            )
        ).to_have_value("No project file. Settings are not saved.")
        enter_preset_dimension(window, "Canvas preset 1 width", "480")
        artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
        expect.poll(lambda: artboard.size.width).to_equal(480)
        element(
            window, "Add preset", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        expect(
            element(
                window,
                "Canvas preset 2 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("390")
        assert source.read_bytes() == original
        assert not (tmp_path / "slint-project.json").exists()


def test_canvas_preset_inspector_switches_projects(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    environment = editor_environment | {"SLINT_NO_MUDA": "1"}
    projects = []
    for name, width in [("FirstProject", 480), ("OtherProject", 600)]:
        root = tmp_path / name
        root.mkdir()
        source = root / "Main.slint"
        source.write_text(
            "export component Main inherits Window { background: #47a87b; }"
        )
        project_file = root / "slint-project.json"
        project_file.write_text(
            json.dumps(
                {
                    "slint-edition": "1.19",
                    "metadata": {
                        "visual-editor": {
                            "canvas-presets": [{"width": width, "height": 320}]
                        }
                    },
                }
            )
        )
        projects.append((source, project_file))
    other_source, other_project_file = projects[1]
    other_contents = other_project_file.read_bytes()
    with launch_editor(editor_binary, environment, other_source) as editor:
        wait_for_source(other_source, other_source.read_bytes())
        expect(
            element(
                first_window(editor),
                "Canvas preset 1 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("600")
    with launch_editor(editor_binary, environment, projects[0][0]) as editor:
        window = first_window(editor)
        wait_for_source(projects[0][0], projects[0][0].read_bytes())
        enter_preset_dimension(window, "Canvas preset 1 width", "999")
        wait_for_project_file(
            projects[0][1],
            b'{"slint-edition": "1.19", "metadata": {"visual-editor": {"canvas-presets":[{"width":999,"height":320}]}}}',
        )
        element(window, "File").single_click(slint_testing.PointerEventButton.Left)
        element(window, "Open Recent").single_click(
            slint_testing.PointerEventButton.Left
        )
        element(window, "OtherProject").single_click(
            slint_testing.PointerEventButton.Left
        )
        wait_for_source(other_source, other_source.read_bytes())
        expect(
            element(
                window,
                "Canvas preset 1 width",
                role=slint_testing.AccessibleRole.TextInput,
            )
        ).to_have_value("600")
        assert other_project_file.read_bytes() == other_contents
