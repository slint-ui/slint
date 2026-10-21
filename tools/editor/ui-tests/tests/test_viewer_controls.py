# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getcolors

import json
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from ui_assertions import expect
from ui_driver import element, first_window, launch_editor, query, screenshot


def test_viewer_autostart_transport_and_popup(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        left = slint_testing.PointerEventButton.Left
        expect(query(window, "Connected")).to_be_visible()

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
            count for count, color in colors if color == (232, 243, 255)
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
        screenshot(window).save(tmp_path / "device-manager-saved.png")
        element(
            window, "Pair nearby device", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        screenshot(window).save(tmp_path / "device-manager-nearby.png")
        element(
            window, "Enter IP address...", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(window, "Searching…", role=slint_testing.AccessibleRole.Button)
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

        element(window, "Stop preview").single_click(left)
        expect(query(window, "Off")).to_be_visible()
        element(window, "Viewer controls").single_click(left)
        element(window, "Choose viewer").single_click(left)
        element(
            window, "Slint Viewer (local)", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(query(window, "Connected")).to_be_visible()


def test_saved_device_removal_requires_confirmation(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        expect(query(first_window(editor), "Connected")).to_be_visible()
        settings = slint_testing.wait_until(
            lambda: next(
                Path(editor_environment["HOME"]).parent.rglob(
                    "springboard-settings.json"
                ),
                None,
            )
        )
    data = json.loads(settings.read_text())
    data["devices"] = [
        {
            "id": f"saved-{index}",
            "name": f"Saved phone {index}",
            "addresses": [f"192.0.2.{index + 1}"],
            "port": 9000,
            "manual": False,
        }
        for index in range(5)
    ]
    settings.write_text(json.dumps(data))
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        left = slint_testing.PointerEventButton.Left
        expect(query(window, "Connected")).to_be_visible()
        element(window, "Viewer controls").single_click(left)
        element(
            window, "Manage devices...", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(query(window, "Remove Slint Viewer (local)")).to_be_hidden()
        screenshot(window).save(tmp_path / "saved-devices.png")
        element(window, "Remove Saved phone 0").single_click(left)
        expect(
            query(window, "Confirm", role=slint_testing.AccessibleRole.Button)
        ).to_be_visible()
        screenshot(window).save(tmp_path / "confirm-removal.png")
        element(window, "Devices", role=slint_testing.AccessibleRole.Text).single_click(
            left
        )
        expect(
            query(window, "Confirm", role=slint_testing.AccessibleRole.Button)
        ).to_be_hidden()
        expect(query(window, "Remove Saved phone 0")).to_be_visible()
        element(window, "Remove Saved phone 0").single_click(left)
        element(
            window, "Confirm", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(window, "Saved phone 0", role=slint_testing.AccessibleRole.Text)
        ).to_be_hidden()
        expect(query(window, "Built-in")).to_be_visible()
        slint_testing.wait_until(
            lambda: len(json.loads(settings.read_text())["devices"]) == 4
        )
