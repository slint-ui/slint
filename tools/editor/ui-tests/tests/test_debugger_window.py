# Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from editor_sync import wait_for_source
from ui_assertions import expect
from ui_driver import element, launch_editor, query, window_of, windows_of

LEFT = slint_testing.PointerEventButton.Left


def test_window_menu_opens_the_debugger(
    editor_binary, editor_environment, radial_scene
):
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = window_of(editor, "EditorUi")
        assert not windows_of(editor, "DebuggerUi")
        element(window, "Window").single_click(LEFT)
        element(window, "Slint Debugger").single_click(LEFT)

        debugger = window_of(editor, "DebuggerUi")
        expect(query(debugger, "Slint Debugger")).to_be_visible()
        expect(query(debugger, "Viewer controls")).to_be_visible()


def test_debugger_and_editor_share_the_viewer(debugger_editor, radial_scene):
    wait_for_source(radial_scene, radial_scene.read_bytes())
    editor = window_of(debugger_editor, "EditorUi")
    debugger = window_of(debugger_editor, "DebuggerUi")
    windows = (editor, debugger)

    for window in windows:
        expect(query(window, "Off")).to_be_visible()
        expect(query(window, "Slint Viewer (local)")).to_be_visible()

    element(debugger, "Start preview").single_click(LEFT)
    for window in windows:
        expect(query(window, "Stop preview")).to_be_visible()
        expect(query(window, "Off")).to_be_hidden()

    element(editor, "Stop preview").single_click(LEFT)
    for window in windows:
        expect(query(window, "Off")).to_be_visible()
        expect(query(window, "Start preview")).to_be_visible()
