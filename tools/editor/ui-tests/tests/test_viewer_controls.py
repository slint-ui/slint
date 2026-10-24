# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getcolors

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from ui_assertions import expect
from ui_driver import element, first_window, launch_editor, query, screenshot


def test_viewer_transport_and_popup(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        left = slint_testing.PointerEventButton.Left
        expect(query(window, "Off")).to_be_visible()
        element(window, "Start preview").single_click(left)
        expect(query(window, "Stop preview")).to_be_visible()
        element(window, "Viewer controls").single_click(left)
        element(window, "Choose viewer").single_click(left)
        element(
            window, "Slint Viewer (local)", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(query(window, "Connected")).to_be_visible()
        element(window, "Viewer controls").single_click(left)

        launcher = element(window, "Viewer controls")
        assert launcher.size.width == pytest.approx(252)
        assert launcher.size.height == pytest.approx(112 / 3)
        transport = element(window, "Stop preview")
        assert transport.absolute_position.x > center(launcher).x
        window.dispatch_event(
            slint_testing.PointerMoveEvent(center(window.root_element))
        )
        idle = screenshot(window)
        scale = idle.width / window.root_element.size.width
        bounds = launcher.absolute_rect
        popup_area = (
            round(bounds.x * scale),
            round(bounds.y * scale),
            round(center(launcher).x * scale),
            round((bounds.y + bounds.height) * scale),
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(center(launcher)))
        hovered = screenshot(window)
        assert idle.crop(popup_area) != hovered.crop(popup_area)
        window.dispatch_event(slint_testing.PointerMoveEvent(center(transport)))
        transport_hovered = screenshot(window)
        assert idle.crop(popup_area) == transport_hovered.crop(popup_area)
        transport_hovered.save(tmp_path / "viewer-transport-hover.png")
        element(window, "Stop preview").single_click(left)
        expect(query(window, "Off")).to_be_visible()
        element(window, "Start preview").single_click(left)
        expect(query(window, "Connected")).to_be_visible()

        element(window, "Viewer controls").single_click(left)
        popup = element(window, "Viewer options")
        assert popup.size.width == pytest.approx(368.8)
        assert popup.size.height == pytest.approx(105.6)
        bounds = popup.absolute_rect
        selector = element(window, "Choose viewer")
        manager = element(
            window, "Manage devices...", role=slint_testing.AccessibleRole.Button
        )
        assert selector.size.width == manager.size.width
        assert selector.absolute_position.x == manager.absolute_position.x
        expect(
            query(window, "Viewer", role=slint_testing.AccessibleRole.Text)
        ).to_be_hidden()
        expect(query(window, "Animations")).to_be_hidden()
        expect(query(window, "Animation speed")).to_be_hidden()
        inset = selector.absolute_position.x - popup.absolute_position.x
        assert selector.size.width == pytest.approx(popup.size.width - 2 * inset)
        assert (
            selector.absolute_position.y - popup.absolute_position.y
            == pytest.approx(inset, abs=0.01)
        )
        bottom_gap = (
            popup.absolute_position.y
            + popup.size.height
            - manager.absolute_position.y
            - manager.size.height
        )
        assert bottom_gap == pytest.approx(inset, abs=0.01)
        selector.single_click(left)
        expect(
            query(
                window, "Slint Viewer (local)", role=slint_testing.AccessibleRole.Button
            )
        ).to_be_visible()
        assert popup.absolute_rect == bounds
        viewer_list = screenshot(window)
        viewer_list.save(tmp_path / "viewer-list.png")
        selected = element(
            window, "Slint Viewer (local)", role=slint_testing.AccessibleRole.Button
        )
        scale = viewer_list.width / window.root_element.size.width
        # Nested popup bounds are popup-relative, so count the selected fill in the window capture.
        colors = viewer_list.getcolors(viewer_list.width * viewer_list.height) or []
        selected_pixels = sum(
            count for count, color in colors if color == (227, 241, 252)
        )
        assert (
            selected_pixels
            >= selected.size.width * selected.size.height * scale**2 * 0.75
        )

        element(
            window, "Slint Viewer (local)", role=slint_testing.AccessibleRole.Button
        ).single_click(left)

        assert popup.absolute_rect == bounds
        screenshot(window).save(tmp_path / "viewer-controls.png")
        element(
            window, "Manage devices...", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(
                window, "Close device manager", role=slint_testing.AccessibleRole.Button
            )
        ).to_be_visible()
        devices_panel = element(window, "Device manager")
        assert devices_panel.size.width == pytest.approx(368.8)
        assert devices_panel.size.height == pytest.approx(331.2)
        assert devices_panel.absolute_position.x == pytest.approx(bounds.x)
        heading = element(window, "Devices", role=slint_testing.AccessibleRole.Text)
        assert (
            heading.absolute_position.x - devices_panel.absolute_position.x
            == pytest.approx(19.2)
        )
        assert (
            heading.absolute_position.y - devices_panel.absolute_position.y
            == pytest.approx(18.4)
        )
        screenshot(window).save(tmp_path / "device-manager-saved.png")
        element(
            window, "Pair nearby device", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        screenshot(window).save(tmp_path / "device-manager-nearby.png")
        element(
            window, "Enter IP address...", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(window, "Connect", role=slint_testing.AccessibleRole.Button)
        ).to_be_visible()
        screenshot(window).save(tmp_path / "device-manager-address.png")
        element(
            window, "Close device manager", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(
                window, "Close device manager", role=slint_testing.AccessibleRole.Button
            )
        ).to_be_hidden()
