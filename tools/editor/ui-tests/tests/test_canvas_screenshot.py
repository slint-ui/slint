# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import base64
from io import BytesIO

import pytest
from editor_mcp import call_editor_tool
from editor_sync import wait_for_source
from PIL import Image
from ui_driver import first_window, launch_editor


def canvas_image(editor_binary, project):
    response = call_editor_tool(
        editor_binary, project, "screenshot_visual_editor_canvas"
    )
    return Image.open(
        BytesIO(base64.b64decode(response["content"][0]["data"]))
    ).convert("RGB")


def assert_canvas_color(image, expected, previous):
    colors = {
        color: count for count, color in image.getcolors(image.width * image.height)
    }
    assert colors.get(expected, 0) > 1000
    assert colors.get(previous, 0) == 0


@pytest.mark.parametrize(
    "change", ["main", "import", "new_import", "created_import", "image"]
)
def test_canvas_screenshot_waits_for_immediate_file_edits(
    editor_binary, editor_environment, fixture_project, tmp_path, change
):
    source = fixture_project / "Main.slint"
    imported = fixture_project / "Color.slint"
    imported.write_text(
        "export component Color inherits Rectangle { background: #ff0000; }"
    )
    image_path = fixture_project / "color.png"
    Image.new("RGB", (200, 160), "red").save(image_path)
    if change in ("import", "new_import", "created_import"):
        initial = 'import { Color } from "Color.slint"; export component Main inherits Window { width: 200px; height: 160px; Color {} }'
    elif change == "image":
        initial = 'export component Main inherits Window { width: 200px; height: 160px; Image { source: @image-url("color.png"); } }'
    else:
        initial = "export component Main inherits Window { width: 200px; height: 160px; background: #ff0000; }"
    source.write_text(initial)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, initial.encode())
        first_window(editor)
        if change == "main":
            source.write_text(initial.replace("#ff0000", "#00ff00"))
        elif change == "import":
            imported.write_text(imported.read_text().replace("#ff0000", "#00ff00"))
        elif change == "created_import":
            source.write_text(initial.replace("Color.slint", "Missing.slint"))
            response = call_editor_tool(
                editor_binary,
                fixture_project,
                "screenshot_visual_editor_canvas",
                expected_error="latest preview failed to compile",
            )
            assert "Missing.slint" in response["content"][0]["text"]
            (fixture_project / "Missing.slint").write_text(
                imported.read_text().replace("#ff0000", "#00ff00")
            )
        elif change == "new_import":
            new_import = fixture_project / "NewColor.slint"
            new_import.write_text(imported.read_text().replace("#ff0000", "#00ff00"))
            source.write_text(initial.replace("Color.slint", "NewColor.slint"))
        else:
            Image.new("RGB", (200, 160), (0, 255, 0)).save(image_path)
        image = canvas_image(editor_binary, fixture_project)
        assert_canvas_color(image, (0, 255, 0), (255, 0, 0))
        image.save(tmp_path / f"canvas-latest-{change}.png")


def test_canvas_screenshot_reports_current_errors_then_captures_corrected_source(
    editor_binary, editor_environment, fixture_project, tmp_path
):
    source = fixture_project / "Main.slint"
    initial = "export component Main inherits Window { width: 200px; height: 160px; background: #ff0000; }"
    source.write_text(initial)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, initial.encode())
        first_window(editor)
        source.write_text(
            "export component Main inherits Window {\n    width: 200px;\n    background: invalid-color;\n}\n"
        )
        response = call_editor_tool(
            editor_binary,
            fixture_project,
            "screenshot_visual_editor_canvas",
            expected_error="Main.slint:3:",
        )
        assert "invalid-color" in response["content"][0]["text"]
        assert all(content["type"] != "image" for content in response["content"])
        corrected = initial.replace("#ff0000", "#0000ff").replace(
            "{ width", '{ in-out property <string> maximized: "shadowed"; width'
        )
        source.write_text(corrected)
        image = canvas_image(editor_binary, fixture_project)
        assert_canvas_color(image, (0, 0, 255), (255, 0, 0))
        image.save(tmp_path / "canvas-corrected.png")
