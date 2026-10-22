# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getcolors

import json
import socketserver
import threading
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

        element(window, "Stop preview").single_click(left)
        expect(query(window, "Off")).to_be_visible()
        element(window, "Viewer controls").single_click(left)
        element(window, "Choose viewer").single_click(left)
        element(
            window, "Slint Viewer (local)", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(query(window, "Connected")).to_be_visible()


@pytest.mark.parametrize("version", ["1.18.1", "", None])
def test_manual_viewer_failure_and_version_mismatch(
    editor_binary, editor_environment, radial_scene, tmp_path, version
):
    requests = []

    class RejectedViewer(socketserver.BaseRequestHandler):
        def handle(self):
            self.request.settimeout(10)
            headers = b""
            while b"\r\n\r\n" not in headers:
                data = self.request.recv(4096)
                if not data:
                    return
                headers += data
            requests.append(headers)
            version_header = "" if version is None else f"Slint-Version: {version}\r\n"
            self.request.sendall(
                (
                    "HTTP/1.1 400 Bad Request\r\n"
                    + version_header
                    + "Content-Length: 0\r\nConnection: close\r\n\r\n"
                ).encode()
            )

    with socketserver.TCPServer(("127.0.0.1", 0), RejectedViewer) as server:
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            with launch_editor(
                editor_binary, editor_environment, radial_scene
            ) as editor:
                window = first_window(editor)
                left = slint_testing.PointerEventButton.Left
                expect(query(window, "Connected")).to_be_visible()
                for label in [
                    "Viewer controls",
                    "Manage devices...",
                    "Pair nearby device",
                    "Enter IP address...",
                ]:
                    element(
                        window, label, role=slint_testing.AccessibleRole.Button
                    ).single_click(left)
                expect(
                    element(window, "Connect", role=slint_testing.AccessibleRole.Button)
                ).to_be_enabled(False)
                port = element(
                    window, "9000", role=slint_testing.AccessibleRole.TextInput
                )
                port.accessible_value = str(server.server_address[1])
                element(
                    window, "192.168.1.42", role=slint_testing.AccessibleRole.TextInput
                ).accessible_value = "127.0.0.1"
                title = (
                    "Couldn’t connect to this address"
                    if version is None
                    else "Incompatible Slint version"
                )
                expect(query(window, title)).to_be_visible()
                button = element(
                    window,
                    "Search again" if version is None else "Connect",
                    role=slint_testing.AccessibleRole.Button,
                )
                expect(button).to_be_enabled(version is None)
                if version is not None:
                    details = query(
                        window,
                        f"Slint {version or 'Unknown'} · Editor 1.19.0\nUpdate Slint Preview on this device.",
                    )
                    expect(details).to_be_visible()
                    button.single_click(left)
                    expect(details).to_be_visible()
                    assert len(requests) == 1
                else:
                    button.single_click(left)
                    expect.poll(lambda: len(requests)).to_equal(2)
                    expect(query(window, title)).to_be_visible()
                screenshot(window).save(tmp_path / "manual-viewer-status.png")
                port.accessible_value = ""
                expect(query(window, "Enter an address and port")).to_be_visible()
                expect(query(window, title)).to_be_hidden()
                expect(
                    element(window, "Connect", role=slint_testing.AccessibleRole.Button)
                ).to_be_enabled(False)
        finally:
            server.shutdown()
            worker.join(timeout=10)


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


@pytest.mark.parametrize("dismiss", ["close", "outside"])
def test_closing_device_manager_cancels_pending_search(
    editor_binary, editor_environment, radial_scene, dismiss
):
    requested = threading.Event()
    disconnected = threading.Event()

    class PendingViewer(socketserver.BaseRequestHandler):
        def handle(self):
            self.request.settimeout(10)
            headers = b""
            while b"\r\n\r\n" not in headers:
                data = self.request.recv(4096)
                if not data:
                    return
                headers += data
            requested.set()
            # Keep the WebSocket handshake pending until the editor cancels it.
            while self.request.recv(4096):
                pass
            disconnected.set()

    with socketserver.TCPServer(("127.0.0.1", 0), PendingViewer) as server:
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            with launch_editor(
                editor_binary, editor_environment, radial_scene
            ) as editor:
                window = first_window(editor)
                left = slint_testing.PointerEventButton.Left
                expect(query(window, "Connected")).to_be_visible()
                element(window, "Viewer controls").single_click(left)
                element(
                    window,
                    "Manage devices...",
                    role=slint_testing.AccessibleRole.Button,
                ).single_click(left)
                element(
                    window,
                    "Pair nearby device",
                    role=slint_testing.AccessibleRole.Button,
                ).single_click(left)
                element(
                    window,
                    "Enter IP address...",
                    role=slint_testing.AccessibleRole.Button,
                ).single_click(left)
                element(
                    window, "9000", role=slint_testing.AccessibleRole.TextInput
                ).accessible_value = str(server.server_address[1])
                element(
                    window, "192.168.1.42", role=slint_testing.AccessibleRole.TextInput
                ).accessible_value = "127.0.0.1"
                expect.poll(requested.is_set).to_equal(True)
                if dismiss == "close":
                    element(window, "Close device manager").single_click(left)
                else:
                    element(
                        window, "Project", role=slint_testing.AccessibleRole.Text
                    ).single_click(left)
                expect(query(window, "Close device manager")).to_be_hidden()
                expect.poll(disconnected.is_set).to_equal(True)
                expect(query(window, "Connected")).to_be_visible()
        finally:
            server.shutdown()
            worker.join(timeout=10)
