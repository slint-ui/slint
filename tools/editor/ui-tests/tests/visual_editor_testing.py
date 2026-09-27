# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import contextlib
import math
import sys
from collections.abc import Iterator
from pathlib import Path

import slint_testing
from canvas_interactions import center_canvas_selection, frame_rotation, zoom_canvas
from editor_sync import wait_for_source
from inspector_interactions import FIELDS, inspector_field
from slint_test import Drag, Locator, Point, Window, expect, inspection_sources, step
from source_snapshot import SourceSnapshot, wait_for_source_change
from ui_driver import launch_editor


class Inspector:
    def __init__(self, editor: Editor):
        self.editor = editor

    def field(self, name: str) -> Locator:
        pane = self.editor.window.get_by_role(
            "complementary", name="Inspector and outline"
        )
        locator = pane.get_by_role("text-input", name=FIELDS.get(name, name))

        def source_context():
            if name not in ("x", "y", "width", "height"):
                return {}
            if not self.editor.selected_identifier:
                return {}
            return {
                "path": str(self.editor.source.resolve()),
                "element": self.editor.selected_identifier,
                "property": name,
            }

        locator.source_context = source_context
        return locator

    def reveal(self, name: str) -> Locator:
        with step(
            "Reveal inspector field",
            layer="adapter",
            name=name,
            trace_coverage="group-only legacy scrolling",
        ):
            inspector_field(
                self.editor.window,
                FIELDS.get(name, name),
                slint_testing.AccessibleRole.TextInput,
            )
        return self.field(name)

    def set_geometry(self, **values: float) -> None:
        with step("Set geometry", layer="adapter", values=values):
            for name, value in values.items():
                if name not in ("x", "y", "width", "height"):
                    raise ValueError(f"Unknown geometry field: {name}")
                field = self.reveal(name)
                old = self.editor.source.read_bytes()
                if field.read(lambda e: e.accessible_value) == str(value):
                    continue
                field.set_accessible_value(str(value))
                wait_for_source_change(self.editor.source, old)
                expect(field).to_have_value(str(value))


class CanvasElement:
    def __init__(self, editor: Editor, identifier: str, kind: str):
        self.editor, self.identifier, self.kind = editor, identifier, kind

    def select(self) -> None:
        with step("Select canvas element", layer="adapter", identifier=self.identifier):
            row = self.editor.outline_row(self.identifier)
            row.activate()
            expect(row).to_be_selected()
            expect(self.selection).to_have_count(1)
            self.editor.selected_identifier = self.identifier

    @property
    def selection(self) -> Locator:
        return self.editor.window.get_by_role("region", name=f"Selected {self.kind}")

    def handle(self, name: str) -> Locator:
        return self.editor.window.get_by_accessible_name(f"{self.kind} {name}")

    def handle_center(self, name: str) -> Point:
        return self.handle(name).center(rotation_degrees=self.rotation_degrees())

    def rotation_degrees(self) -> float:
        return math.degrees(frame_rotation(self.editor.window, self.kind))

    def drag(self, handle: str = "move handle") -> Drag:
        return self.handle(handle).drag(rotation_degrees=self.rotation_degrees())

    def locator(self) -> Locator:
        return self.editor.window.get_by_id(
            f"{self.editor.source.stem}::{self.identifier}"
        )

    def move_by(self, x: float, y: float) -> None:
        with step("Move canvas element", layer="adapter", x=x, y=y, space="window"):
            self.select()
            baseline = self.editor.source.read_bytes()
            with self.drag() as drag:
                for i in range(1, 4):
                    drag.move_by(x * i / 3, y * i / 3, origin="start")
                drag.release()
            wait_for_source_change(self.editor.source, baseline)


class Canvas:
    def __init__(self, editor: Editor):
        self.editor = editor

    def element(self, identifier: str, *, kind: str = "Rectangle") -> CanvasElement:
        return CanvasElement(self.editor, identifier, kind)

    def zoom_to(self, percent: int) -> None:
        with step(
            "Zoom canvas",
            layer="adapter",
            percent=percent,
            trace_coverage="group-only legacy helper",
        ):
            zoom_canvas(self.editor.window, percent)

    def center_selection(self) -> None:
        with step(
            "Center canvas selection",
            layer="adapter",
            trace_coverage="group-only legacy helper",
        ):
            center_canvas_selection(self.editor.window)


class Files:
    def __init__(self, editor: Editor):
        self.editor = editor

    def row(self, name: str) -> Locator:
        return self.editor.window.get_by_role("tree", name="Files").get_by_role(
            "list-item", name=str(self.editor.source.parent / name)
        )

    def rename(self, name: str, basename: str) -> None:
        with step("Rename file", layer="adapter", name=name, basename=basename):
            self.row(name).activate()
            keyboard = self.editor.window.keyboard
            keyboard.press("Enter" if sys.platform == "darwin" else "F2")
            expect(
                self.editor.window.get_by_role("text-input", name=f"Rename {name}")
            ).to_have_count(1)
            keyboard.press("Backspace")
            keyboard.press_sequentially(basename)
            keyboard.press("Enter")


class Editor:
    def __init__(self, raw: slint_testing.Application, source: Path):
        self.source = source
        window = raw.first_window
        if window is None:
            raise RuntimeError("Editor has no window")
        self.raw_window: slint_testing.Window = window
        self.window = Window(window)
        self.selected_identifier = ""
        self.inspector, self.canvas, self.files = (
            Inspector(self),
            Canvas(self),
            Files(self),
        )

    def outline_row(self, name: str) -> Locator:
        return self.window.get_by_role("list", name="Current file outline").get_by_role(
            "list-item", name=name
        )

    def snapshot(self) -> SourceSnapshot:
        return SourceSnapshot.capture(self.source.parent)

    def undo(self) -> None:
        with step("Undo", layer="adapter"):
            self.window.keyboard.shortcut("Control", "z")

    def redo(self) -> None:
        with step("Redo", layer="adapter"):
            self.window.keyboard.shortcut(
                *(
                    ("Control", "y")
                    if sys.platform == "win32"
                    else ("Control", "Shift", "z")
                )
            )


@contextlib.contextmanager
def open_editor(
    binary: Path, environment: dict[str, str], source: Path
) -> Iterator[Editor]:
    with launch_editor(binary, environment, source) as raw, inspection_sources(source):
        wait_for_source(source, source.read_bytes())
        editor = Editor(raw, source)
        try:
            yield editor
        finally:
            with contextlib.suppress(Exception):
                editor.window.cleanup_input()
