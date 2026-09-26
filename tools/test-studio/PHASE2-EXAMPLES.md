# Phase 2: Twenty Real Test Comparisons


**Revised design:** See [Generic Testing And Visual Editor Helpers](PHASE2-DESIGN.md) for the layer boundaries and current Playwright review.
The after snippets below are the initial mixed-layer sketches.
Their proposed `set_value`, `get_by_label`, `double_click`, and editor-derived `fill` contracts need the corrections defined there.
Each example now identifies its generic and editor-specific parts.

These examples compare 20 existing Visual Editor test functions with proposed Phase 2 Python syntax.
The originals were verified against `nigel/slint-test-studio` at `b5f30c154b`.
They include parameterized families, not just 20 individual parameter cases.

**The after code is a design proposal, not an implemented or runnable API.**
Original imports, constants, fixtures, and test-specific helper definitions remain available unless explicitly replaced.
Every original parameter decorator is retained.
The proposed snippets were parsed and Ruff-formatted; they have not been executed.
No existing tests or application code were changed for this comparison.

The aim is clearer intent, fresh locators, better assertion failures, and a useful action timeline.
Some difficult tests should remain detailed.
Their independent expected values, raw event ordering, and pixel checks are the specification.

## Reading The Proposed API

| Layer | Proposed Syntax | Contract |
| --- | --- | --- |
| Launch fixture | `with open_editor(path) as editor:` | Reuse configured binary/environment and existing lifecycle cleanup; expose the first window; await initial source acknowledgment when a file is supplied. No path opens startup. |
| Locators | `get_by_role`, `get_by_label`, `canvas.element` | Resolve fresh, with exact names and strict single-target actions. Missing and ambiguous matches produce different errors. Element IDs are scoped to the loaded component. |
| Input | `click`, `double_click`, `activate`, `set_value`, `fill` | Pointer clicks, accessibility activation, accessibility value assignment, and keyboard editing remain distinct operations. |
| Keyboard fill | `field.fill("99")` | Select the field text through its existing accessibility activation, delete the current characters, and type the replacement. Do not press Return or blur. Preserve the current staging helper's input path in the pilot. |
| Inspector fields | `inspector.field(label)` | Locate the field using the existing inspector scrolling policy; do not focus it during an assertion. |
| Assertions | `expect(locator).to_have_value(...)` | Re-resolve and poll reads until the assertion passes or its deadline expires. Do not replay dispatched input. |
| Arbitrary reads | `expect.poll(read).to_equal(...)` | Retry a read-only probe and report the last observed value. Exception policy must distinguish invalid handles from genuine application errors. |
| Sustained assertions | `expect.poll(read).to_remain(..., for_ms=250)` | Check immediately, then sample throughout the interval. Fail on the first mismatch; never wait for a bad initial value to become good. This is sampled evidence, not continuous event monitoring. |
| Gestures | `with handle.drag() as drag:` | Press once; explicit moves and release. A key change does not move the pointer. Cleanup releases held input after failures without retrying the gesture. |
| Coordinates | `space="window"`, `origin="start"` | Logical window coordinates. `origin="start"` makes moves cumulative from the initial press. Otherwise a delta is relative to the current pointer. Never clamp outside-window coordinates. |
| Geometry | `bounds`, `window_geometry`, `selection_frame` | Read coherent current state without input. Retry invalidated reads. Frame snapshots are immutable and hashable. Handle centers account for ancestor rotation. |
| Source assertions | Existing `SourceSnapshot` methods | Keep exact project-wide source checks, preview acknowledgment, immediate checks, and 250 ms quiescence checks distinct. |
| Adapter operations | `select`, `center_selection`, `zoom_to`, outline and palette targets | Wrap the existing UI interactions and coordinate policies. Never implement these by directly editing document state. |
| Picker scope | `picker.button`, `picker.field`, `picker.mode` | Fresh button, text-input, and combobox locators scoped to the active fill session and its child popup. `.open()` uses the existing accessibility activation. |
| Window identity | `window.identity()` | Snapshot the handle and physical size. A separate process assertion checks that the editor remains alive. |
| Reporting | `with editor.step("..."):` | Optional grouping above automatic action/assertion events. Plain pytest remains supported. |

Keep launch configuration in pytest fixtures rather than repeating binary and environment arguments in every test.
`SourceSnapshot`, `replace_once`, and specialized geometry helpers remain ordinary Python.
The proposal deliberately reuses those established checks rather than inventing a second source assertion language.

Every dispatched action should emit its locator, arguments, source line, duration, and result.
Failures should show expected/actual values and relevant source or image differences.
Readiness and assertion waits need their own timing in the action detail.
Timing-sensitive drag and reload tests must not acquire synchronous screenshots between individual inputs by default.

`raw_window` and `resolve()` are explicit migration escape hatches in examples 12, 13, 14, 17, 18, 19, and 20.
Retained raw helpers keep their current assertions and event sequences.
They initially provide only a named group in the timeline; individual internal actions need instrumentation when those helpers migrate.

## The Twenty Tests

### 01. Startup Actions And Absent Editor Panes
[test_startup_page_shows_project_actions_without_editor_panes](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_startup.py:47) · Everyday

Retain absence checks, not just invisibility. Both startup buttons must be enabled. No project is opened.

**Generic library:** Role/name locators, count/enabled assertions.

**Visual Editor/test-specific:** Startup labels and launch configuration.

**Before — exact existing function**

```python
def test_startup_page_shows_project_actions_without_editor_panes(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Startup wizard", slint_testing.AccessibleRole.Region
        )
        assert not elements_with_label(window.root_element, "Editor canvas")
        assert not elements_with_label(window.root_element, "Project and elements")
        assert not elements_with_label(window.root_element, "Inspector and outline")

        create = window_element_with_label(
            window, "Create New Project...", slint_testing.AccessibleRole.Button
        )
        assert create.accessible_enabled
        open_existing = window_element_with_label(
            window, "Open Existing Project...", slint_testing.AccessibleRole.Button
        )
        assert open_existing.accessible_enabled
```

**After — proposed API**

```python
def test_startup_page_shows_project_actions_without_editor_panes(open_editor):
    with open_editor() as editor:
        expect(editor.get_by_role("region", name="Startup wizard")).to_exist()
        for label in ("Editor canvas", "Project and elements", "Inspector and outline"):
            expect(editor.get_by_label(label)).to_have_count(0)
        expect(
            editor.get_by_role("button", name="Create New Project...")
        ).to_be_enabled()
        expect(
            editor.get_by_role("button", name="Open Existing Project...")
        ).to_be_enabled()
```

### 02. Rename A File Through The Inline Editor
[test_file_tree_renames_file_inline](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_navigation.py:25) · Everyday

Preserve the platform rename key, Backspace, typing, and Return. Renaming on disk would bypass the behavior under test.

**Generic library:** Activation, keyboard input, condition waiting.

**Visual Editor/test-specific:** File rows, rename shortcut, filesystem expectations.

**Before — exact existing function**

```python
def test_file_tree_renames_file_inline(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    target = fixture_project / "Renamed.slint"
    expected = source.read_text()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        file_row(window, source).invoke_accessible_default_action()
        press_key(window, keys.Return if sys.platform == "darwin" else keys.F2)
        window_element_with_label(
            window, "Rename Main.slint", slint_testing.AccessibleRole.TextInput
        )

        press_key(window, keys.Backspace)
        press_keys(window, "Renamed")
        press_key(window, keys.Return)

        wait_until(lambda: True if target.is_file() and not source.exists() else None)
        assert target.read_text() == expected
```

**After — proposed API**

```python
def test_file_tree_renames_file_inline(open_editor, fixture_project):
    source = fixture_project / "Main.slint"
    target = fixture_project / "Renamed.slint"
    original = source.read_text()
    with open_editor(source) as editor:
        editor.files.row("Main.slint").activate()
        editor.keyboard.press(keys.Return if sys.platform == "darwin" else keys.F2)
        expect(editor.get_by_role("text-input", name="Rename Main.slint")).to_exist()
        editor.keyboard.press(keys.Backspace)
        editor.keyboard.type("Renamed")
        editor.keyboard.press(keys.Return)
        expect.poll(lambda: target.is_file() and not source.exists()).to_equal(True)
        assert target.read_text() == original
```

### 03. Search Preserves Independent Library Collapse States
[test_library_search_restores_independent_collapse_states](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_palette.py:200) · Stateful

Keep both independent collapse states, ordering, empty results, hidden groups, and the final source-quiescence check. Search uses accessibility value changes.

**Generic library:** Value assignment, scoped collections, ordered name assertions.

**Visual Editor/test-specific:** Library groups and collapse-state expectations.

**Before — exact existing function**

```python
def test_library_search_restores_independent_collapse_states(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        search = window_element_with_label(window, "Search elements")
        for label, collapsed_labels in [
            ("Visual", ["TouchArea"]),
            ("Input & interaction", []),
        ]:
            window_element_with_label(
                window, label, slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            expect_library(window, collapsed_labels)
            search.accessible_value = "t"
            expect_library(window, ["Rectangle", "Text", "TouchArea"])
            search.accessible_value = "missing"
            expect_library(window, [])
            search.accessible_value = ""
            expect_library(window, collapsed_labels)
        window_element_with_label(
            window, "Visual", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        expect_library(window, ["Image", "Rectangle", "Text"])
        search.accessible_value = "touch"
        expect_library(window, ["TouchArea"])
        assert not elements_with_label(
            window.root_element, "Visual", slint_testing.AccessibleRole.Button
        )
        search.accessible_value = ""
        expect_library(window, ["Image", "Rectangle", "Text"])
        snapshot.assert_unchanged()
```

**After — proposed API**

```python
def test_library_search_restores_independent_collapse_states(
    open_editor, fixture_project
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(fixture_project / "Palette.slint") as editor:
        search = editor.get_by_label("Search elements")
        for group, collapsed in [
            ("Visual", ["TouchArea"]),
            ("Input & interaction", []),
        ]:
            editor.get_by_role("button", name=group).activate()
            expect(editor.library.items).to_have_labels(collapsed)
            for query, expected in [
                ("t", ["Rectangle", "Text", "TouchArea"]),
                ("missing", []),
                ("", collapsed),
            ]:
                search.set_value(query)
                expect(editor.library.items).to_have_labels(expected)
        editor.get_by_role("button", name="Visual").activate()
        expect(editor.library.items).to_have_labels(["Image", "Rectangle", "Text"])
        search.set_value("touch")
        expect(editor.library.items).to_have_labels(["TouchArea"])
        expect(editor.get_by_role("button", name="Visual")).to_have_count(0)
        search.set_value("")
        expect(editor.library.items).to_have_labels(["Image", "Rectangle", "Text"])
        snapshot.assert_unchanged()
```

### 04. Delete In An Inspector Field Does Not Delete The Element
[test_focused_inspector_field_consumes_delete_key](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_selection.py:180) · Everyday

Retain both Delete and Backspace cases. Click the field center without replacing or selecting its contents; the test checks focus routing.

**Generic library:** Real click, keys, presence assertions.

**Visual Editor/test-specific:** Canvas selection and unchanged project source.

**Before — exact existing function**

```python
@pytest.mark.parametrize(
    "key", [keys.Backspace, keys.Delete], ids=["backspace", "delete"]
)
def test_focused_inspector_field_consumes_delete_key(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        field = window_element_with_label(
            window, FIELDS["x"], slint_testing.AccessibleRole.TextInput
        )
        target = slint_testing.LogicalPosition(
            x=field.absolute_position.x + field.size.width / 2,
            y=field.absolute_position.y + field.size.height / 2,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        press_key(window, key)
        window_element_with_label(
            window, "Selected Rectangle", slint_testing.AccessibleRole.Region
        )
        snapshot.assert_unchanged()
```

**After — proposed API**

```python
@pytest.mark.parametrize(
    "key", [keys.Backspace, keys.Delete], ids=["backspace", "delete"]
)
def test_focused_inspector_field_consumes_delete_key(open_editor, fixture_project, key):
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(fixture_project / "Main.slint") as editor:
        editor.canvas.element("root-rectangle").select()
        editor.inspector.field(FIELDS["x"]).click()
        editor.keyboard.press(key)
        expect(editor.get_by_role("region", name="Selected Rectangle")).to_exist()
        snapshot.assert_unchanged()
```

### 05. Inspector Geometry: Exact Source And Recreated Preview
[test_geometry_field_writes_exact_source](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_inspector.py:115) · Stateful

Keep the original x/y/width/height parameters and exact byte replacements. Fresh locator reads must survive preview replacement. Absolute-position offsets remain explicit.

**Generic library:** Fresh reads, value/bounds assertions.

**Visual Editor/test-specific:** Inspector scope, preview offset, exact source.

**Before — exact existing function**

```python
@pytest.mark.parametrize(
    ("property_name", "original_value", "value", "old", "new"),
    [
        ("x", 32, "44", b"        x: 32px;", b"        x: 44px;"),
        ("y", 32, "48", b"        y: 32px;", b"        y: 48px;"),
        ("width", 160, "176", b"        width: 160px;", b"        width: 176px;"),
        (
            "height",
            96,
            "112",
            b"        width: 160px;\n        height: 96px;",
            b"        width: 160px;\n        height: 112px;",
        ),
    ],
    ids=("x", "y", "width", "height"),
)
def test_geometry_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    property_name: str,
    original_value: float,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    label = FIELDS[property_name]
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")

        def rendered_value():
            elements = window.find_elements_by_id("InspectorCases::inspect-rectangle")
            if len(elements) != 1:
                return None
            rectangle = elements[0]
            geometry = (
                rectangle.absolute_position
                if property_name in ("x", "y")
                else rectangle.size
            )
            # Preview replacement can invalidate the handle during the property read.
            return getattr(geometry, property_name) if rectangle.is_valid else None

        expected = float(value)
        if property_name in ("x", "y"):
            # Absolute positions include the preview's offset in the editor window.
            preview_offset = wait_until(rendered_value) - original_value
            expected += preview_offset

        edit_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_exact(
            replace_once(baseline, old, new), relative_path=INSPECTOR_SOURCE
        )
        wait_for_field(
            window,
            label,
            value,
            slint_testing.AccessibleRole.TextInput,
        )

        # Source and field updates can precede preview replacement.
        # The existing rectangle must reflect the edit, not merely exist.
        def geometry_matches():
            actual = rendered_value()
            return actual if actual == pytest.approx(expected) else None

        wait_until(geometry_matches)
```

**After — proposed API**

```python
@pytest.mark.parametrize(
    ("property_name", "original_value", "value", "old", "new"),
    [
        ("x", 32, "44", b"        x: 32px;", b"        x: 44px;"),
        ("y", 32, "48", b"        y: 32px;", b"        y: 48px;"),
        ("width", 160, "176", b"        width: 160px;", b"        width: 176px;"),
        (
            "height",
            96,
            "112",
            b"        width: 160px;\n        height: 96px;",
            b"        width: 160px;\n        height: 112px;",
        ),
    ],
    ids=("x", "y", "width", "height"),
)
def test_geometry_field_writes_exact_source(
    open_editor, fixture_project, property_name, original_value, value, old, new
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        rectangle = editor.canvas.element("inspect-rectangle")
        rectangle.select()
        expected = float(value)
        if property_name in ("x", "y"):
            expected += rectangle.window_geometry()[property_name] - original_value
        field = editor.inspector.field(FIELDS[property_name])
        field.set_value(value)
        snapshot.wait_for_exact(replace_once(baseline, old, new), INSPECTOR_SOURCE)
        expect(field).to_have_value(value)
        expect.poll(lambda: rectangle.window_geometry()[property_name]).to_equal(
            pytest.approx(expected)
        )
```

### 06. Inline Text Commits On Return And Escape
[test_inline_text_key_commit](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_inline_text.py:47) · Everyday

Escape commits in this existing test. Preserve double-click entry and the assertion that the original rendered text disappears during editing.

**Generic library:** Double-click, keys, absence assertions.

**Visual Editor/test-specific:** Move handle and editor-specific Escape-commits behavior.

**Before — exact existing function**

```python
@pytest.mark.parametrize("commit_key", [keys.Return, keys.Escape])
def test_inline_text_key_commit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    commit_key: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = edited_source(source_file, "Edited")

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        begin_inline_edit(window)
        press_keys(window, "Edited")
        press_key(window, commit_key)
        snapshot.wait_for_applied(expected)
        assert not elements_with_label(window.root_element, "Inline text editor")
        window_element_with_label(window, "Edited", slint_testing.AccessibleRole.Text)
```

**After — proposed API**

```python
@pytest.mark.parametrize("commit_key", [keys.Return, keys.Escape])
def test_inline_text_key_commit(open_editor, fixture_project, commit_key):
    source = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = edited_source(source, "Edited")
    with open_editor(source) as editor:
        editor.canvas.element("root-text").select()
        editor.get_by_label("Text move handle").double_click()
        expect(editor.get_by_role("text-input", name="Inline text editor")).to_exist()
        expect(editor.get_by_role("text", name="Fixture text")).to_have_count(0)
        editor.keyboard.type("Edited")
        editor.keyboard.press(commit_key)
        snapshot.wait_for_applied(expected)
        expect(editor.get_by_label("Inline text editor")).to_have_count(0)
        expect(editor.get_by_role("text", name="Edited")).to_exist()
```

### 07. Reparent Through The Outline With Exact Source
[test_outline_changes_element_parent_with_exact_source](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_outline.py:155) · Stateful

Keep both golden files and all row ordering, hierarchy levels, and selection flags. The adapter must perform the same pointer drag, not modify the document directly.

**Generic library:** Drag-to, scoped list observations.

**Visual Editor/test-specific:** Outline targets, hierarchy and source goldens.

**Before — exact existing function**

```python
@pytest.mark.parametrize(
    "source,target,golden",
    [
        (
            "sibling-a",
            "container",
            "OutlineCases.reparent.sibling-a.slint",
        ),
        (
            "child-a",
            "<outline-root>",
            "OutlineCases.reparent.child-a-root.slint",
        ),
    ],
    ids=["child", "root"],
)
def test_outline_changes_element_parent_with_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
    golden: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        drag_row(window, source, target, "onto")
        snapshot.wait_for_exact((GOLDENS / golden).read_bytes(), "OutlineCases.slint")
        expected = (
            [
                ("container", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 3", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-a", "Hierarchy level 3", False),
                ("sibling-b", "Hierarchy level 2", False),
            ]
            if source == "sibling-a"
            else [
                ("container", "Hierarchy level 2", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-a", "Hierarchy level 2", False),
                ("sibling-b", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 2", False),
            ]
        )
        wait_for_outline_state(window, expected)
```

**After — proposed API**

```python
@pytest.mark.parametrize(
    "source,target,golden",
    [
        (
            "sibling-a",
            "container",
            "OutlineCases.reparent.sibling-a.slint",
        ),
        (
            "child-a",
            "<outline-root>",
            "OutlineCases.reparent.child-a-root.slint",
        ),
    ],
    ids=["child", "root"],
)
def test_outline_changes_element_parent_with_exact_source(
    open_editor, fixture_project, source, target, golden
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(fixture_project / "OutlineCases.slint") as editor:
        editor.outline.row(source).drag_to(editor.outline.drop_target(target, "onto"))
        snapshot.wait_for_exact((GOLDENS / golden).read_bytes(), "OutlineCases.slint")
        expected = (
            [
                ("container", 2),
                ("child-a", 3),
                ("child-b", 3),
                ("sibling-a", 3),
                ("sibling-b", 2),
            ]
            if source == "sibling-a"
            else [
                ("container", 2),
                ("child-b", 3),
                ("sibling-a", 2),
                ("sibling-b", 2),
                ("child-a", 2),
            ]
        )
        expect(
            editor.outline.rows.named(*(name for name, _ in expected))
        ).to_have_state(
            [(name, f"Hierarchy level {level}", False) for name, level in expected],
            timeout_ms=15000,
        )
```

### 08. One Palette Drag Crosses Canvas And Outline, Then Cancels
[test_palette_preview_switches_between_canvas_and_outline_and_cancels](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_palette_outline.py:126) · Hard

This is one held gesture, not three drag-and-drop operations. Release still happens after Escape. Preserve the outline ghost height and mutual exclusion of previews.

**Generic library:** Held gesture, movement, Escape, release.

**Visual Editor/test-specific:** Palette, canvas and outline previews.

**Before — exact existing function**

```python
@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_preview_switches_between_canvas_and_outline_and_cancels(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        canvas = canvas_drop_position(window)
        outline = drop_position(window, "container", "onto")
        begin_palette_drag(window, kind, canvas)
        window_element_with_label(window, f"{kind} drag preview")
        window.dispatch_event(slint_testing.PointerMoveEvent(outline))
        ghost = window_element_with_label(window, "Outline drag preview")
        assert not elements_with_label(window.root_element, f"{kind} drag preview")
        assert ghost.size.height == 32
        window.dispatch_event(slint_testing.PointerMoveEvent(canvas))
        window_element_with_label(window, f"{kind} drag preview")
        assert not elements_with_label(window.root_element, "Outline drag preview")
        window.dispatch_event(slint_testing.PointerMoveEvent(outline))
        press_key(window, keys.Escape)
        release_palette_drag(window, outline)
        assert not elements_with_label(window.root_element, "Outline drag preview")
        snapshot.assert_unchanged()
```

**After — proposed API**

```python
@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_preview_switches_between_canvas_and_outline_and_cancels(
    open_editor, fixture_project, kind
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(fixture_project / "OutlineCases.slint") as editor:
        canvas = editor.canvas.drop_point()
        outline = editor.outline.drop_target("container", "onto").point()
        preview = editor.get_by_label(f"{kind} drag preview")
        ghost = editor.get_by_label("Outline drag preview")
        with editor.library.item(kind).drag() as drag:
            drag.move_by(16, 16, space="window")
            drag.move_to(canvas)
            expect(preview).to_exist()
            drag.move_to(outline)
            expect(ghost).to_have_size(height=32)
            expect(preview).to_have_count(0)
            drag.move_to(canvas)
            expect(preview).to_exist()
            expect(ghost).to_have_count(0)
            drag.move_to(outline)
            editor.keyboard.press(keys.Escape)
            drag.release()
        expect(ghost).to_have_count(0)
        snapshot.assert_unchanged()
```

### 09. Move A Nested Rotated Element In Window Coordinates
[test_nested_rotated_element_move_writes_exact_local_source](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_canvas.py:982) · Hard

Preserve a 20-by-16 window-space drag becoming a 16-by-minus-20 local-source edit. Keep the three transient frame samples and absence of source writes before release.

**Generic library:** Explicit window deltas and sampled frames.

**Visual Editor/test-specific:** Rotated handle geometry and local-source conversion oracle.

**Before — exact existing function**

```python
def test_nested_rotated_element_move_writes_exact_local_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, "nested-rotated-text")
        manual_drag(
            window,
            window_element_with_label(window, "Text move handle"),
            20,
            16,
            snapshot,
        )
        expected = replace_once(
            baseline,
            b"                x: 44px;\n                y: 52px;",
            b"                x: 60px;\n                y: 32px;",
        )
        snapshot.wait_for_exact(expected, "RotatedCanvasCases.slint")
```

**After — proposed API**

```python
def test_nested_rotated_element_move_writes_exact_local_source(
    open_editor, fixture_project
):
    source = fixture_project / "RotatedCanvasCases.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        text = editor.canvas.element("nested-rotated-text")
        text.select()
        initial = text.selection_frame()
        with text.handle("move").drag() as drag:
            frames = []
            for step in range(1, 4):
                drag.move_by(
                    20 * step / 3, 16 * step / 3, space="window", origin="start"
                )
                frames.append(text.selection_frame())
            assert frames[-1] != initial
            assert len(set(frames)) >= 2
            snapshot.assert_unchanged_now()
            drag.release()
        expected = replace_once(
            baseline,
            b"                x: 44px;\n                y: 52px;",
            b"                x: 60px;\n                y: 32px;",
        )
        snapshot.wait_for_exact(expected, source.name)
```

### 10. Press Or Release Shift Mid-Resize Without Moving The Pointer
[test_resize_modifier_changes_during_drag](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_canvas.py:1261) · Hard

Keep both modifier directions. The frame must change from the key event alone, while source stays unchanged. Commit only on pointer release.

**Generic library:** Held modifier changes and geometry assertions.

**Visual Editor/test-specific:** Rectangle resize constraints and commit boundary.

**Before — exact existing function**

```python
@pytest.mark.parametrize(
    "press_shift_during_drag",
    [pytest.param(True, id="press-shift"), pytest.param(False, id="release-shift")],
)
def test_resize_modifier_changes_during_drag(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    press_shift_during_drag: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        live_modifier_resize(
            window,
            snapshot,
            press_shift_during_drag=press_shift_during_drag,
        )
        geometry = (
            b"        width: 200px;\n        height: 200px;"
            if press_shift_during_drag
            else b"        width: 200px;\n        height: 136px;"
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                geometry,
            ),
        )
```

**After — proposed API**

```python
@pytest.mark.parametrize(
    "press_shift_during_drag",
    [pytest.param(True, id="press-shift"), pytest.param(False, id="release-shift")],
)
def test_resize_modifier_changes_during_drag(
    open_editor, fixture_project, press_shift_during_drag
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        rectangle = editor.canvas.element("root-rectangle")
        rectangle.select()
        if not press_shift_during_drag:
            editor.keyboard.down(keys.Shift)
        with rectangle.handle("resize bottom-right").drag() as drag:
            drag.move_by(20, 16, space="window")
            before_modifier = rectangle.selection_frame()
            snapshot.assert_unchanged_now()
            if press_shift_during_drag:
                editor.keyboard.down(keys.Shift)
            else:
                editor.keyboard.up(keys.Shift)
            expect.poll(rectangle.selection_frame).not_to_equal(before_modifier)
            frame = rectangle.selection_frame()
            assert (frame.width == frame.height) == press_shift_during_drag
            snapshot.assert_unchanged_now()
            drag.release()
        if press_shift_during_drag:
            editor.keyboard.up(keys.Shift)
        height = 200 if press_shift_during_drag else 136
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                f"        width: 200px;\n        height: {height}px;".encode(),
            )
        )
```

### 11. Resize Past The Window Boundary And Commit On Release
[test_resize_continues_outside_window_and_commits_on_release](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_canvas.py:1968) · Hard

Keep the existing distance constant, ceil rounding, three transient samples, and no source writes before release. Pointer coordinates must not be clamped to the window.

**Generic library:** Window size and unclamped pointer movement.

**Visual Editor/test-specific:** Resize handles and resulting source.

**Before — exact existing function**

```python
def test_resize_continues_outside_window_and_commits_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        handle = window_element_with_label(window, "Rectangle resize bottom-right")
        start = center(handle)
        size = window.root_element.size
        dx = math.ceil(size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x)
        dy = math.ceil(size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y)
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(window, handle, dx, dy, snapshot)
        expected = replace_once(
            baseline,
            b"        width: 180px;\n        height: 120px;",
            f"        width: {180 + dx}px;\n        height: {120 + dy}px;".encode(),
        )
        snapshot.wait_for_applied(expected)
```

**After — proposed API**

```python
def test_resize_continues_outside_window_and_commits_on_release(
    open_editor, fixture_project
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        rectangle = editor.canvas.element("root-rectangle")
        rectangle.select()
        handle = rectangle.handle("resize bottom-right")
        start = handle.center()
        size = editor.window.logical_size()
        dx = math.ceil(size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x)
        dy = math.ceil(size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y)
        initial = rectangle.selection_frame()
        with handle.drag() as drag:
            frames = []
            for step in range(1, 4):
                drag.move_by(
                    dx * step / 3, dy * step / 3, space="window", origin="start"
                )
                frames.append(rectangle.selection_frame())
            assert frames[-1] != initial
            assert len(set(frames)) >= 2
            snapshot.assert_unchanged_now()
            drag.release()
        snapshot.wait_for_applied(
            replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                f"        width: {180 + dx}px;\n        height: {120 + dy}px;".encode(),
            )
        )
```

### 12. Zoomed Move And Resize Still Write Document Units
[test_zoomed_drag_writes_document_units](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_canvas_zoom.py:184) · Hard

Retain all six zoom/operation combinations and undo/redo. Explicit window deltas keep the coordinate conversion under test. Retain manual_drag while migrating its shared assertions.

**Generic library:** Pointer deltas, steps and assertions.

**Visual Editor/test-specific:** Zoom, document units, undo/redo source checks.

**Before — exact existing function**

```python
@pytest.mark.parametrize("percent", [50, 125, 200])
@pytest.mark.parametrize("operation", ["move", "resize"])
def test_zoomed_drag_writes_document_units(
    editor_binary, editor_environment, fixture_project, percent, operation
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        zoom_canvas(window, percent)
        label = (
            "Rectangle move handle"
            if operation == "move"
            else "Rectangle resize bottom-right"
        )
        handle = window_element_with_label(window, label)
        manual_drag(window, handle, 20 * percent / 100, 16 * percent / 100, original)
        old, new = (
            (b"x: 40px;\n        y: 40px;", b"x: 60px;\n        y: 56px;")
            if operation == "move"
            else (
                b"width: 180px;\n        height: 120px;",
                b"width: 200px;\n        height: 136px;",
            )
        )
        expected = replace_once(baseline, old, new)
        original.wait_for_applied(expected)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(baseline)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(expected)
```

**After — proposed API**

```python
@pytest.mark.parametrize("percent", [50, 125, 200])
@pytest.mark.parametrize("operation", ["move", "resize"])
def test_zoomed_drag_writes_document_units(
    open_editor, fixture_project, percent, operation
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        rectangle = editor.canvas.element("root-rectangle")
        rectangle.select()
        editor.canvas.center_selection()
        editor.canvas.zoom_to(percent)
        handle = rectangle.handle(
            "move" if operation == "move" else "resize bottom-right"
        )
        with editor.step(f"{operation} at {percent}% zoom"):
            manual_drag(
                editor.raw_window,
                handle.resolve(),
                20 * percent / 100,
                16 * percent / 100,
                snapshot,
            )
        old, new = (
            (b"x: 40px;\n        y: 40px;", b"x: 60px;\n        y: 56px;")
            if operation == "move"
            else (
                b"width: 180px;\n        height: 120px;",
                b"width: 200px;\n        height: 136px;",
            )
        )
        expected = replace_once(baseline, old, new)
        snapshot.wait_for_applied(expected)
        editor.undo()
        snapshot.wait_for_applied(baseline)
        editor.redo()
        snapshot.wait_for_applied(expected)
```

### 13. Selection Clipping: Pixel Evidence On All Four Edges
[test_selection_overlays_are_clipped_to_canvas](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_canvas_clipping.py:38) · Hard

Keep exact pixels, scale/rounding rules, external source writes, and the visible-outline sanity check. Geometry alone cannot prove clipping. Protected regions reuse the current helper.

**Generic library:** Screenshot capture and image-diff support.

**Visual Editor/test-specific:** Protected pane regions and independent pixel oracle.

**Before — exact existing function**

```python
@pytest.mark.parametrize("edge", ["left", "right", "top", "bottom"])
def test_selection_overlays_are_clipped_to_canvas(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    edge: str,
) -> None:
    source = fixture_project / "BoundsCases.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        canvas = window_element_with_label(window, "Editor canvas")
        artboard = window_element_with_label(window, "Artboard")
        select_outline_row(window, "bounds-rectangle")
        initial_frame = window_element_with_label(window, "Selected Rectangle")
        before = screenshot(window)
        scale = before.width / window.root_element.size.width
        border_x = round(
            (initial_frame.absolute_position.x + initial_frame.size.width / 2) * scale
        )
        border_y = math.floor(initial_frame.absolute_position.y * scale) - 1
        outline_color = before.getpixel((border_x, border_y))
        assert outline_color != before.getpixel((border_x, border_y - 2))
        x, y = artboard.absolute_position.x + 96, artboard.absolute_position.y + 80
        if edge == "left":
            x = canvas.absolute_position.x - 40
        elif edge == "right":
            x = canvas.absolute_position.x + canvas.size.width - 80
        elif edge == "top":
            y = canvas.absolute_position.y - 20
        else:
            y = canvas.absolute_position.y + canvas.size.height - 60
        expected = (
            source.read_text()
            .replace("x: 96px;", f"x: {x - artboard.absolute_position.x}px;")
            .replace("y: 80px;", f"y: {y - artboard.absolute_position.y}px;")
        )
        source.write_text(expected)
        wait_for_source(source, expected.encode())
        select_outline_row(window, "bounds-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        assert frame.absolute_position.x == pytest.approx(x)
        assert frame.absolute_position.y == pytest.approx(y)
        after = screenshot(window)
        for name, region in protected_regions(window, after).items():
            assert after.crop(region).tobytes() == before.crop(region).tobytes(), name
        scale = after.width / window.root_element.size.width
        region = (
            math.ceil(canvas.absolute_position.x * scale),
            math.ceil(canvas.absolute_position.y * scale),
            math.floor((canvas.absolute_position.x + canvas.size.width) * scale),
            after.height,
        )
        visible = after.crop(region)
        colors = visible.getcolors(visible.width * visible.height) or []
        assert sum(count for count, color in colors if color == outline_color) > 20
```

**After — proposed API**

```python
@pytest.mark.parametrize("edge", ["left", "right", "top", "bottom"])
def test_selection_overlays_are_clipped_to_canvas(open_editor, fixture_project, edge):
    source = fixture_project / "BoundsCases.slint"
    with open_editor(source) as editor:
        rectangle = editor.canvas.element("bounds-rectangle")
        rectangle.select()
        canvas = editor.get_by_label("Editor canvas").bounds()
        artboard = editor.get_by_label("Artboard").bounds()
        frame = rectangle.selection_bounds()
        before = editor.window.screenshot()
        scale = before.width / editor.window.logical_size().width
        bx = round((frame.x + frame.width / 2) * scale)
        by = math.floor(frame.y * scale) - 1
        outline_color = before.getpixel((bx, by))
        assert outline_color != before.getpixel((bx, by - 2))
        x, y = {
            "left": (canvas.x - 40, artboard.y + 80),
            "right": (canvas.right - 80, artboard.y + 80),
            "top": (artboard.x + 96, canvas.y - 20),
            "bottom": (artboard.x + 96, canvas.bottom - 60),
        }[edge]
        expected = (
            source.read_text()
            .replace("x: 96px;", f"x: {x - artboard.x}px;")
            .replace("y: 80px;", f"y: {y - artboard.y}px;")
        )
        source.write_text(expected)
        wait_for_source(source, expected.encode())
        rectangle.select()
        expect(rectangle.selection).to_have_position(
            x=pytest.approx(x), y=pytest.approx(y)
        )
        after = editor.window.screenshot()
        for name, region in protected_regions(editor.raw_window, after).items():
            assert after.crop(region).tobytes() == before.crop(region).tobytes(), name
        scale = after.width / editor.window.logical_size().width
        visible = after.crop(
            (
                math.ceil(canvas.x * scale),
                math.ceil(canvas.y * scale),
                math.floor(canvas.right * scale),
                after.height,
            )
        )
        colors = visible.getcolors(visible.width * visible.height) or []
        assert sum(count for count, color in colors if color == outline_color) > 20
```

### 14. Preview Reload Retargets Hover Without Pointer Motion
[test_preview_reload_updates_hover_target_without_pointer_motion](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_hover_refresh.py:166) · Stateful

Retain the current hover setup. After the external write, only observe state; a convenience action that moves the pointer would invalidate the test.

**Generic library:** Pointer control and read-only observations.

**Visual Editor/test-specific:** External source reload and preview hover labels.

**Before — exact existing function**

```python
def test_preview_reload_updates_hover_target_without_pointer_motion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        hover_rectangle(window)
        expected = baseline.replace(b"x: 180px;", b"x: 40px;").replace(
            b"y: 56px;", b"y: 80px;"
        )
        source.write_bytes(expected)
        wait_for_source(source, expected)
        wait_until(
            lambda: (
                True
                if elements_with_label(window.root_element, "Hovered Text")
                and not elements_with_label(window.root_element, "Hovered Rectangle")
                else None
            )
        )
```

**After — proposed API**

```python
def test_preview_reload_updates_hover_target_without_pointer_motion(
    open_editor, fixture_project
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with open_editor(source) as editor:
        hover_rectangle(editor.raw_window)
        expected = baseline.replace(b"x: 180px;", b"x: 40px;").replace(
            b"y: 56px;", b"y: 80px;"
        )
        source.write_bytes(expected)
        wait_for_source(source, expected)
        expect.poll(
            lambda: (
                editor.get_by_label("Hovered Text").count() > 0,
                editor.get_by_label("Hovered Rectangle").count(),
            )
        ).to_equal((True, 0))
```

### 15. An External Revision Invalidates An Uncommitted Field Edit
[test_stale_revision_commit_is_rejected](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_source_safety.py:177) · Hard

Retain keyboard staging, all four fields, exact project contents, and the 15-second refresh timeout. Return goes to current focus; it must not refocus a freshly found field.

**Generic library:** Keyboard staging, current-focus keys, read-only waits.

**Visual Editor/test-specific:** Source revisions and invalidated inspector edit.

**Before — exact existing function**

```python
@pytest.mark.parametrize(
    ("label", "property_name", "original", "updated"),
    [
        (FIELDS["x"], "x", "32", "36"),
        (FIELDS["y"], "y", "32", "40"),
        (FIELDS["width"], "width", "160", "180"),
        (FIELDS["height"], "height", "96", "120"),
    ],
)
def test_stale_revision_commit_is_rejected(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    label: str,
    property_name: str,
    original: str,
    updated: str,
) -> None:
    source_file = fixture_project / "InspectorCases.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        stage_field_text(window, label, "99")
        snapshot.assert_unchanged_now()
        external = baseline.replace(
            f"        {property_name}: {original}px;".encode(),
            f"        {property_name}: {updated}px;".encode(),
            1,
        )
        assert external != baseline
        source_file.write_bytes(external)
        snapshot.wait_for_exact(external, relative_path="InspectorCases.slint")
        snapshot = SourceSnapshot.capture(fixture_project)
        wait_until(
            lambda: (
                field
                if (
                    field := window_element_with_label(
                        window, label, slint_testing.AccessibleRole.TextInput
                    )
                ).accessible_value
                == updated
                else None
            ),
            timeout=15,
        )
        press_key(window, keys.Return)
        snapshot.assert_unchanged()
```

**After — proposed API**

```python
@pytest.mark.parametrize(
    ("label", "property_name", "original", "updated"),
    [
        (FIELDS["x"], "x", "32", "36"),
        (FIELDS["y"], "y", "32", "40"),
        (FIELDS["width"], "width", "160", "180"),
        (FIELDS["height"], "height", "96", "120"),
    ],
)
def test_stale_revision_commit_is_rejected(
    open_editor, fixture_project, label, property_name, original, updated
):
    source = fixture_project / "InspectorCases.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        editor.canvas.element("inspect-rectangle").select()
        field = editor.inspector.field(label)
        field.fill("99")
        snapshot.assert_unchanged_now()
        external = baseline.replace(
            f"        {property_name}: {original}px;".encode(),
            f"        {property_name}: {updated}px;".encode(),
            1,
        )
        assert external != baseline
        source.write_bytes(external)
        snapshot.wait_for_exact(external, source.name)
        external_snapshot = SourceSnapshot.capture(fixture_project)
        expect(field).to_have_value(updated, timeout_ms=15000)
        editor.keyboard.press(keys.Return)
        external_snapshot.assert_unchanged()
```

### 16. Rapid Source Writes Settle On The Newest Revision
[test_rapid_root_writes_show_newest_revision](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_reload.py:64) · Hard

Three writes remain consecutive with no intervening synchronization. After convergence, fail on any sampled regression for 250 ms. Preserve process, window identity, and size checks.

**Generic library:** Sustained observations, process/window identity.

**Visual Editor/test-specific:** Unsynchronized source writes and newest-revision oracle.

**Before — exact existing function**

```python
def test_rapid_root_writes_show_newest_revision(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        handle, size = window.handle, window.size
        original = source_file.read_bytes()
        source_file.write_bytes(original.replace(b"Fixture text", b"Revision one"))
        source_file.write_bytes(original.replace(b"Fixture text", b"Revision two"))
        expected = original.replace(b"Fixture text", b"Newest revision")
        source_file.write_bytes(expected)
        snapshot.wait_for_exact(expected)
        window_element_with_label(
            window, "Newest revision", slint_testing.AccessibleRole.Text, timeout=15
        )
        deadline = time.monotonic() + 0.25
        while time.monotonic() < deadline:
            assert source_file.read_bytes() == expected
            assert not elements_with_label(window.root_element, "Revision one")
            assert not elements_with_label(window.root_element, "Revision two")
            time.sleep(0.02)
        assert_editor_stable(editor, window, handle, size)
```

**After — proposed API**

```python
def test_rapid_root_writes_show_newest_revision(open_editor, fixture_project):
    source = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        expect(editor.get_by_role("text", name="Fixture text")).to_exist()
        window_identity = editor.window.identity()
        original = source.read_bytes()
        source.write_bytes(original.replace(b"Fixture text", b"Revision one"))
        source.write_bytes(original.replace(b"Fixture text", b"Revision two"))
        expected = original.replace(b"Fixture text", b"Newest revision")
        source.write_bytes(expected)
        snapshot.wait_for_exact(expected)
        expect(editor.get_by_role("text", name="Newest revision")).to_exist(
            timeout_ms=15000
        )
        expect.poll(
            lambda: (
                source.read_bytes(),
                editor.get_by_label("Revision one").count(),
                editor.get_by_label("Revision two").count(),
            )
        ).to_remain((expected, 0, 0), for_ms=250, interval_ms=20)
        expect(editor.process).to_be_running()
        expect(editor.window).to_have_identity(window_identity)
```

### 17. Undo/Redo Matrix: Handles, Inspector, Rotation, And Radius
[test_rectangle_undo_redo](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_undo_redo.py:162) · Hard

Keep all ten CASES, the independent byte oracle, inspector checks, oriented frame tolerance, radius-handle geometry, and initial-state assertions. Existing edit/assert_visual helpers stay during migration.

**Generic library:** Fixtures, parameterization, steps and shared assertions.

**Visual Editor/test-specific:** Edit matrix, source byte oracle, oriented/radius geometry.

**Before — exact existing function**

```python
@pytest.mark.parametrize(
    "case,changes",
    CASES,
    ids=[case for case, _ in CASES],
)
def test_rectangle_undo_redo(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    case: str,
    changes: dict[str, int],
) -> None:
    source = fixture_project / SOURCE
    baseline = source.read_bytes()
    expected = baseline
    for name, value in changes.items():
        if name == "radius":
            expected = expected.replace(
                b"        transform-rotation: 0deg;",
                f"        border-bottom-left-radius: {value}px;\n"
                f"        border-bottom-right-radius: {value}px;\n"
                "        transform-rotation: 0deg;".encode(),
            ).replace(
                b"        border-radius: 12px;",
                "        border-radius: 12px;\n"
                f"        border-top-left-radius: {value}px;\n"
                f"        border-top-right-radius: {value}px;".encode(),
            )
            continue
        prop, unit = {
            "rotation": ("transform-rotation", "deg"),
        }.get(name, (name, "px"))
        old = f"        {prop}: {INITIAL[name]}{unit};".encode()
        assert expected.count(old) == 1
        expected = expected.replace(old, f"        {prop}: {value}{unit};".encode())
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        with replay_stage("initial"):
            select_fixture_element(window, "Rectangle")
            cx, cy, *_ = wait_until(
                lambda: oriented_selection_frame(window, "Rectangle")
            )
            origin = (
                cx - INITIAL["x"] - INITIAL["width"] / 2,
                cy - INITIAL["y"] - INITIAL["height"] / 2,
            )
            assert_visual(window, INITIAL, origin, case.endswith("radius"))
            snapshot.assert_unchanged_now()
        with replay_stage("initial edit"):
            edit(window, case, changes, snapshot)
            snapshot.wait_for_applied(expected, SOURCE)
            assert_visual(window, INITIAL | changes, origin, case.endswith("radius"))
        for name, content, values in [
            ("undo", baseline, INITIAL),
            ("redo", expected, INITIAL | changes),
        ]:
            with replay_stage(name):
                if case.startswith("inspector-"):
                    select_fixture_element(window, "Rectangle")
                shortcut(window, redo=name == "redo")
                snapshot.wait_for_applied(content, SOURCE)
                assert_visual(window, values, origin, case.endswith("radius"))
```

**After — proposed API**

```python
@pytest.mark.parametrize(
    "case,changes",
    CASES,
    ids=[case for case, _ in CASES],
)
def test_rectangle_undo_redo(open_editor, fixture_project, case, changes):
    source = fixture_project / SOURCE
    baseline = source.read_bytes()
    expected = expected_rectangle_source(baseline, changes)
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(source) as editor:
        rectangle = editor.canvas.element("root-rectangle")
        with editor.step("Initial state"):
            rectangle.select()
            cx, cy, *_ = wait_until(
                lambda: oriented_selection_frame(editor.raw_window, "Rectangle")
            )
            origin = (
                cx - INITIAL["x"] - INITIAL["width"] / 2,
                cy - INITIAL["y"] - INITIAL["height"] / 2,
            )
            assert_visual(editor.raw_window, INITIAL, origin, case.endswith("radius"))
            snapshot.assert_unchanged_now()
        with editor.step(f"Edit using {case}"):
            edit(editor.raw_window, case, changes, snapshot)
            snapshot.wait_for_applied(expected, SOURCE)
            assert_visual(
                editor.raw_window, INITIAL | changes, origin, case.endswith("radius")
            )
        for action, content, values in [
            (editor.undo, baseline, INITIAL),
            (editor.redo, expected, INITIAL | changes),
        ]:
            with editor.step(action.__name__):
                if case.startswith("inspector-"):
                    rectangle.select()
                action()
                snapshot.wait_for_applied(content, SOURCE)
                assert_visual(
                    editor.raw_window, values, origin, case.endswith("radius")
                )
```

### 18. Undo While The Pointer Is Down Must Cancel The Later Release
[test_undo_while_dragging_cancels_release](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_inspector_transform.py:648) · Hard

Keep both empty and existing undo history. Press at 32 degrees even when the value is 42, then move to 62. Undo occurs while the pointer remains down.

**Generic library:** Held pointer and explicit keyboard shortcut.

**Visual Editor/test-specific:** Editor undo history and release cancellation.

**Before — exact existing function**

```python
@pytest.mark.parametrize("history", [False, True])
def test_undo_while_dragging_cancels_release(
    editor_binary, editor_environment, fixture_project, history
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        wait_for_source(fixture_project / SOURCE, baseline)
        select_element(window, "Rectangle")
        if history:
            edit_field(window, "Rotation", "42")
            snapshot.wait_for_applied(
                baseline.replace(b"32deg", b"42deg"), relative_path=SOURCE
            )
            wait_for_field(window, "Rotation", "42")
        knob = window_element_with_label(window, "Rotation knob")
        start, end = point(knob, 32), point(knob, 62)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Rotation", "72" if history else "62")
        press_shortcut(window, keys.Control, "z")
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        snapshot.wait_for_applied(baseline, relative_path=SOURCE)
        snapshot.assert_unchanged()
        wait_for_field(window, "Rotation", "32")
```

**After — proposed API**

```python
@pytest.mark.parametrize("history", [False, True])
def test_undo_while_dragging_cancels_release(open_editor, fixture_project, history):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with open_editor(fixture_project / SOURCE) as editor:
        editor.canvas.element("inspect-rectangle").select()
        rotation = editor.inspector.field("Rotation")
        if history:
            rotation.set_value("42")
            snapshot.wait_for_applied(baseline.replace(b"32deg", b"42deg"), SOURCE)
            expect(rotation).to_have_value("42")
        knob = editor.get_by_label("Rotation knob")
        start, end = point(knob.resolve(), 32), point(knob.resolve(), 62)
        with editor.pointer.drag_from(start) as drag:
            drag.move_to(end)
            expect(rotation).to_have_value("72" if history else "62")
            editor.keyboard.shortcut(keys.Control, "z")
            drag.release()
        snapshot.wait_for_applied(baseline, SOURCE)
        snapshot.assert_unchanged()
        expect(rotation).to_have_value("32")
```

### 19. Gradient Session: Mode Changes, Cancel, Undo/Redo, Reopen
[test_gradient_session_cancel_undo_redo_and_reopen](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_gradient_geometry.py:420) · Hard

Keep the private brush binding, alpha stop, mode transitions, restored radial geometry, first-session cancellation, whole-session undo, and reopening the picker. No new editor process is launched when reopening.

**Generic library:** Scoped popup locators, input and nested steps.

**Visual Editor/test-specific:** Gradient modes, brush binding, session commit/cancel.

**Before — exact existing function**

```python
def test_gradient_session_cancel_undo_redo_and_reopen(
    editor_binary, editor_environment, tmp_path
):
    from ui_driver import press_key, press_shortcut

    file = gradient_document(tmp_path, "root.paint")
    source = file.read_text().replace(
        "    width: 400px;",
        "    private property <brush> paint: @radial-gradient(circle 90px at 40px 60px, red 0%, blue 100%);\n    width: 400px;",
    )
    file.write_text(source)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        for cancel in [True, False]:
            open_gradient(window)
            picker_field(window, "No recent fills", slint_testing.AccessibleRole.Text)
            click_picker_button(window, "Add gradient stop")
            click_picker_button(window, "Edit stop 2 color")
            color = picker_field(window, "Hex color")
            color.accessible_value = "#12345680"
            wait_until(
                lambda color=color: (
                    color if color.accessible_value == "#12345680" else None
                )
            )
            click_picker_button(window, "Close Stop color")
            set_picker_mode(window, "Gradient type", "Conic")
            rotate_conic(window, 0, 37)
            set_picker_mode(window, "Gradient type", "Linear")
            click_picker_button(window, "Solid")
            assert picker_field(window, "Hex color").accessible_value == "#12345680"
            click_picker_button(window, "Gradient")
            set_picker_mode(window, "Gradient type", "Radial")
            assert radial_geometry(window) == pytest.approx((40, 60, 90), abs=0.001)
            original.assert_unchanged_now()
            if cancel:
                press_key(window, keys.Escape)
                original.assert_unchanged()
            else:
                click_picker_button(window, "Close Custom")
        saved = wait_for_source_change(file, source.encode())
        original.wait_for_applied(saved, file.name)
        assert b"#12345680 50%" in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(source.encode(), file.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, file.name)
        select_outline_row(window, "fill")
        open_gradient(window)
        assert radial_geometry(window) == pytest.approx((40, 60, 90), abs=0.001)
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#12345680"
```

**After — proposed API**

```python
def test_gradient_session_cancel_undo_redo_and_reopen(open_editor, tmp_path):
    source = gradient_document(tmp_path, "root.paint")
    baseline = source.read_text().replace(
        "    width: 400px;",
        "    private property <brush> paint: @radial-gradient(circle 90px at 40px 60px, red 0%, blue 100%);\n    width: 400px;",
    )
    source.write_text(baseline)
    snapshot = SourceSnapshot.capture(tmp_path)
    with open_editor(source) as editor:
        editor.canvas.element("fill").select()
        for cancel in [True, False]:
            with editor.step("Cancel session" if cancel else "Commit session"):
                picker = editor.inspector.fill_picker().open()
                expect(picker.get_by_role("text", name="No recent fills")).to_exist()
                picker.button("Add gradient stop").activate()
                picker.button("Edit stop 2 color").activate()
                color = picker.field("Hex color")
                color.set_value("#12345680")
                expect(color).to_have_value("#12345680")
                picker.button("Close Stop color").activate()
                picker.mode("Gradient type").set_value("Conic")
                rotate_conic(editor.raw_window, 0, 37)
                picker.mode("Gradient type").set_value("Linear")
                picker.button("Solid").activate()
                expect(color).to_have_value("#12345680")
                picker.button("Gradient").activate()
                picker.mode("Gradient type").set_value("Radial")
                expect.poll(lambda: radial_geometry(editor.raw_window)).to_equal(
                    pytest.approx((40, 60, 90), abs=0.001)
                )
                snapshot.assert_unchanged_now()
                if cancel:
                    editor.keyboard.press(keys.Escape)
                    snapshot.assert_unchanged()
                else:
                    picker.button("Close Custom").activate()
        saved = wait_for_source_change(source, baseline.encode())
        snapshot.wait_for_applied(saved, source.name)
        assert b"#12345680 50%" in saved
        editor.undo()
        snapshot.wait_for_applied(baseline.encode(), source.name)
        editor.redo()
        snapshot.wait_for_applied(saved, source.name)
        editor.canvas.element("fill").select()
        picker = editor.inspector.fill_picker().open()
        expect.poll(lambda: radial_geometry(editor.raw_window)).to_equal(
            pytest.approx((40, 60, 90), abs=0.001)
        )
        picker.button("Edit stop 2 color").activate()
        expect(picker.field("Hex color")).to_have_value("#12345680")
```

### 20. Conic Rotation Crosses 360 Degrees Without Losing The Turn
[test_conic_rotation_crosses_the_seam](/Users/nigelb/.codex/worktrees/slint-test-studio/gb-slint/tools/editor/ui-tests/tests/test_conic_gradient_canvas.py:227) · Hard

Keep endpoint and ray inputs, every seam-crossing sample, transformed handle centers, 367-degree serialization, implicit center syntax, undo/redo, and a byte-identical close after reopening.

**Generic library:** Ordered pointer path, transformed geometry reads.

**Visual Editor/test-specific:** Gradient seam winding, 367-degree source, implicit center.

**Before — exact existing function**

```python
@pytest.mark.parametrize("handle", ["endpoint", "ray"])
def test_conic_rotation_crosses_the_seam(
    editor_binary, editor_environment, conic_scene, tmp_path, handle
):
    conic_scene.write_text(
        conic_scene.read_text().replace("from 220deg", "from 350deg")
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 260)
        radius = 126 if handle == "endpoint" else 70
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(
            slint_testing.PointerPressEvent(around(c, radius, 350), button)
        )
        for angle in [355, 359, 1, 7]:
            window.dispatch_event(
                slint_testing.PointerMoveEvent(around(c, radius, angle))
            )
            actual = center(control(window, "Gradient rotation handle"), angle - 90)
            expected = around(c, 126, angle)
            assert actual.x == pytest.approx(expected.x, abs=0.01)
            assert actual.y == pytest.approx(expected.y, abs=0.01)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(around(c, radius, 7), button)
        )
        original.assert_unchanged_now()
        click(window, "Close Custom")
        baseline = original.sources[Path(conic_scene.name)]

        def applied_source() -> bytes | None:
            saved = conic_scene.read_bytes()
            if not saved or saved == baseline:
                return None
            try:
                original.wait_for_applied(saved, conic_scene.name, timeout=0.1)
            except AssertionError:
                return None
            return saved

        saved = wait_until(applied_source)
        angle = re.search(rb"from ([0-9.]+)deg", saved)
        assert angle is not None
        assert float(angle.group(1)) == pytest.approx(367, abs=0.001)
        assert b" at " not in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(conic_scene.name)], conic_scene.name
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, conic_scene.name)
        open_conic(window)
        reopened = center(control(window, "Gradient rotation handle"), 277)
        expected = around(c, 126, 367)
        assert reopened.x == pytest.approx(expected.x, abs=0.01)
        assert reopened.y == pytest.approx(expected.y, abs=0.01)
        click(window, "Close Custom")
        assert conic_scene.read_bytes() == saved
```

**After — proposed API**

```python
@pytest.mark.parametrize("handle", ["endpoint", "ray"])
def test_conic_rotation_crosses_the_seam(open_editor, conic_scene, tmp_path, handle):
    conic_scene.write_text(
        conic_scene.read_text().replace("from 220deg", "from 350deg")
    )
    snapshot = SourceSnapshot.capture(tmp_path)
    baseline = snapshot.sources[Path(conic_scene.name)]
    with open_editor(conic_scene) as editor:
        open_conic(editor.raw_window)
        center_handle = editor.get_by_label("Gradient center handle")
        rotation_handle = editor.get_by_label("Gradient rotation handle")
        c = center_handle.center(rotation_degrees=260)
        radius = 126 if handle == "endpoint" else 70
        with editor.pointer.drag_from(around(c, radius, 350)) as drag:
            for angle in [355, 359, 1, 7]:
                drag.move_to(around(c, radius, angle))
                actual = rotation_handle.center(rotation_degrees=angle - 90)
                expected = around(c, 126, angle)
                assert actual.x == pytest.approx(expected.x, abs=0.01)
                assert actual.y == pytest.approx(expected.y, abs=0.01)
            drag.release()
        snapshot.assert_unchanged_now()
        editor.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(conic_scene, baseline)
        snapshot.wait_for_applied(saved, conic_scene.name)
        angle = re.search(rb"from ([0-9.]+)deg", saved)
        assert angle is not None
        assert float(angle.group(1)) == pytest.approx(367, abs=0.001)
        assert b" at " not in saved
        editor.undo()
        snapshot.wait_for_applied(baseline, conic_scene.name)
        editor.redo()
        snapshot.wait_for_applied(saved, conic_scene.name)
        open_conic(editor.raw_window)
        actual = rotation_handle.center(rotation_degrees=277)
        expected = around(c, 126, 367)
        assert actual.x == pytest.approx(expected.x, abs=0.01)
        assert actual.y == pytest.approx(expected.y, abs=0.01)
        editor.get_by_role("button", name="Close Custom").activate()
        assert conic_scene.read_bytes() == saved
```

## Independent Source Oracle For Example 17

This helper extracts the existing byte-transformation logic without changing it.
Its shorter call site moves code; it does not remove implementation complexity.
Keep this test oracle independent of production serialization.

```python
def expected_rectangle_source(baseline: bytes, changes: dict[str, int]) -> bytes:
    expected = baseline
    for name, value in changes.items():
        if name == "radius":
            expected = expected.replace(
                b"        transform-rotation: 0deg;",
                f"        border-bottom-left-radius: {value}px;\n"
                f"        border-bottom-right-radius: {value}px;\n"
                "        transform-rotation: 0deg;".encode(),
            ).replace(
                b"        border-radius: 12px;",
                "        border-radius: 12px;\n"
                f"        border-top-left-radius: {value}px;\n"
                f"        border-top-right-radius: {value}px;".encode(),
            )
            continue
        prop, unit = {
            "rotation": ("transform-rotation", "deg"),
        }.get(name, (name, "px"))
        old = f"        {prop}: {INITIAL[name]}{unit};".encode()
        assert expected.count(old) == 1
        expected = expected.replace(old, f"        {prop}: {value}{unit};".encode())
    return expected
```

The ten existing cases still cover handle move, resize, rotation, and radius, plus inspector x, y, width, height, rotation, and radius.
The existing `assert_visual` checks oriented frame coordinates within 1.5 units, all four inspector geometry values, and the radius handle when applicable.
The existing `edit` helper keeps its intermediate frame, rotation-tooltip, and source-write assertions.

## What These Examples Change About The Plan

1. Build locators, input distinctions, and assertion diagnostics first.
   Examples 1–6 establish the common vocabulary.
2. Make held gestures and modifier changes first-class operations.
   Examples 8–12 and 18 require explicit press/move/key/release boundaries.
3. Preserve immediate, eventual, and sustained assertions as separate contracts.
   Examples 9–11 and 15–20 expose where automatic waiting can hide regressions.
4. Keep source and visual evidence independent.
   Examples 5, 13, 17, and 20 need more than a successful UI action.
5. Migrate existing helpers incrementally and report their trace limitations.
   Do not describe a raw-helper group as a complete action timeline.

I would pilot examples 2, 5, 10, 15, and 20 before converting the suite.
They exercise normal authoring, preview replacement, held input, revision races, and difficult gradient geometry.
Run old and new implementations independently against identical fixtures and compare source, visual checks, and event sequences.
Then add mutation checks proving the assertions catch wrong local coordinates, premature writes, stale commits, and a seam serialized as 7 degrees.
