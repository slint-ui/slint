# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from ui_assertions import expect
from ui_driver import element, first_window, launch_editor, query, screenshot


def test_viewer_autostart_transport_and_popup(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        left = slint_testing.PointerEventButton.Left
        expect(query(window, "Connected")).to_be_visible()
        element(window, "Stop preview").single_click(left)
        expect(query(window, "Off")).to_be_visible()
        element(window, "Start preview").single_click(left)
        expect(query(window, "Connected")).to_be_visible()

        element(window, "Viewer controls").single_click(left)
        popup = element(window, "Viewer options")
        bounds = popup.absolute_rect
        selector = element(window, "Choose viewer")
        manager = element(
            window, "Device manager", role=slint_testing.AccessibleRole.Button
        )
        assert selector.size.width == manager.size.width
        assert selector.absolute_position.x == manager.absolute_position.x
        assert (
            element(window, "Viewer").absolute_position.x
            == selector.absolute_position.x
        )
        animations = element(window, "Animations")
        speed = element(window, "Animation speed")
        assert animations.absolute_position.x == selector.absolute_position.x
        assert animations.absolute_position.y == speed.absolute_position.y
        assert animations.size.height == speed.size.height
        inset = selector.absolute_position.x - popup.absolute_position.x
        bottom_gap = (
            popup.absolute_position.y
            + popup.size.height
            - manager.absolute_position.y
            - manager.size.height
        )
        assert bottom_gap == inset
        selector.single_click(left)
        expect(
            query(window, "Local viewer", role=slint_testing.AccessibleRole.Button)
        ).to_be_visible()
        assert popup.absolute_rect == bounds
        screenshot(window).save(tmp_path / "viewer-list.png")
        element(
            window, "Local viewer", role=slint_testing.AccessibleRole.Button
        ).single_click(left)

        element(window, "Animation speed").single_click(left)
        element(window, "0.5×", role=slint_testing.AccessibleRole.Button).single_click(
            left
        )
        expect(
            query(window, "0.5×", role=slint_testing.AccessibleRole.Text)
        ).to_be_visible()
        assert popup.absolute_rect == bounds
        screenshot(window).save(tmp_path / "viewer-controls.png")
        element(
            window, "Device manager", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(window, "Done", role=slint_testing.AccessibleRole.Button)
        ).to_be_visible()
        element(window, "Done", role=slint_testing.AccessibleRole.Button).single_click(
            left
        )
        expect(
            query(window, "Done", role=slint_testing.AccessibleRole.Button)
        ).to_be_hidden()

        element(window, "Stop preview").single_click(left)
        expect(query(window, "Off")).to_be_visible()
        element(window, "Viewer controls").single_click(left)
        expect(element(window, "Animation speed")).to_be_enabled(False)
        element(window, "Choose viewer").single_click(left)
        element(
            window, "Local viewer", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(query(window, "Connected")).to_be_visible()
