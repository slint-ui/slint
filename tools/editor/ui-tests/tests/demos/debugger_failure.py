# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Intentional failure demo; collect this file explicitly to run it."""

from slint_test import expect


def test_width_mismatch(editor_factory, fixture_project):
    with editor_factory(fixture_project / "Main.slint") as editor:
        editor.canvas.element("root-rectangle").select()
        editor.inspector.set_geometry(width=200)

        expect(
            editor.inspector.field("width"), "Intentional width mismatch"
        ).to_have_value("250", timeout=500)
