# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import contextlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest
from slint_testing import WaitTimeoutError, wait_until

# cspell:ignore pixman


@pytest.mark.skipif(sys.platform != "linux", reason="Requires Wayland")
def test_local_preview_presents_a_wayland_buffer(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    assert shutil.which("weston"), "Install Weston to test the Wayland preview"
    with contextlib.ExitStack() as processes:
        runtime_directory = processes.enter_context(
            tempfile.TemporaryDirectory(prefix="slint-wayland-")
        )
        environment = editor_environment | {
            "XDG_RUNTIME_DIR": runtime_directory,
            "WAYLAND_DISPLAY": "wayland-test",
            "SLINT_BACKEND": "winit-skia-software",
        }
        environment.pop("DISPLAY", None)
        environment.pop("WAYLAND_SOCKET", None)
        compositor_log = tmp_path / "weston.log"
        compositor_output = processes.enter_context(compositor_log.open("w"))
        compositor = processes.enter_context(
            subprocess.Popen(
                [
                    "weston",
                    "--backend=headless-backend.so",
                    "--use-pixman",
                    "--shell=kiosk-shell.so",
                    "--socket=wayland-test",
                    "--idle-time=0",
                    "--no-config",
                ],
                env=environment,
                stdout=compositor_output,
                stderr=subprocess.STDOUT,
            )
        )
        processes.callback(compositor.kill)

        def compositor_ready():
            assert compositor.poll() is None, compositor_log.read_text()
            return (Path(runtime_directory) / "wayland-test").exists()

        wait_until(compositor_ready, timeout=10)
        preview_log = tmp_path / "preview-wayland.log"
        protocol_log = tmp_path / "preview-protocol.log"
        preview_output = processes.enter_context(preview_log.open("w"))
        protocol_output = processes.enter_context(protocol_log.open("w"))
        preview = processes.enter_context(
            subprocess.Popen(
                [str(editor_binary), "--run-preview-child"],
                env=environment | {"WAYLAND_DEBUG": "client"},
                stdin=subprocess.PIPE,
                stdout=protocol_output,
                stderr=preview_output,
                text=True,
            )
        )
        processes.callback(preview.kill)
        assert preview.stdin is not None
        source_url = radial_scene.as_uri()
        for message in [
            {
                "SetContents": {
                    "url": {"url": source_url, "version": 1},
                    "contents": list(radial_scene.read_bytes()),
                }
            },
            {"ShowPreview": {"url": source_url, "component": None}},
        ]:
            preview.stdin.write(json.dumps({"Lsp": message}) + "\n")
        preview.stdin.flush()

        def preview_presented():
            trace = preview_log.read_text()
            assert preview.poll() is None, trace
            return re.search(
                r"(wl_surface[@#]\d+)\.attach\(wl_buffer[@#]\d+.*?\1\.commit\(\)",
                trace,
                re.DOTALL,
            )

        try:
            wait_until(preview_presented, timeout=20)
        except WaitTimeoutError:
            pytest.fail(
                "Local preview did not present a Wayland buffer\n"
                + protocol_log.read_text()
                + preview_log.read_text()
            )
