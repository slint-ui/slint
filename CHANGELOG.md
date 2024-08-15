<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Changelog
All notable changes to this project are documented in this file.

## Unreleased

### Widgets

 - Fixed `TextEdit` not invoking `edited` callbacks (#5848).

## [1.7.2] - 2024-08-14

### General

 - Added linux arm binary in the release.
 - Skia renderer: Fixed opacity not working on Image. (#5706)
 - Skia renderer: Fixed `SLINT_DEBUG_PERFORMANCE="overlay"`. (#5764)
 - Android: workaround `PollEvent::Wake` not being received (#5699)
 - LinuxKMS backend: Fixed EGL initialization with Mali drivers found on i.MX 95.
 - Winit backend: don't forward synthetic key pressed events to the application.
 - Fixed panic when pressing tab in a PopupWindow. (#5826)

### Slint Language

 - `debug()` is now consistently routed through Platform's `debug_log`, meaning it uses stderr by default instead of stdout previously with C++ and Rust code generator. (#5718)
 - Comparison between float is now using less precision so to float compare equal if they are approximately the same.
 - Conversion from float to a `int` property now always truncates (instead of rounding with the interpreter, or being inconsistant with the code generators)

### Widgets

 - TimePickerPopup: Fixed text input on 24hour time picker. (#5723)

### C++

 - Added `assert_main_thread` in a few more functions.

### Rust

 - Fixed case when the rust generated code panics trying to access a destroyed parent. (part of #3464)

### LSP and tooling

 - LSP: report error opening the preview back to the editor instead of panicking. (#204)
 - LSP: don't suggest private properties or functions as bindgins.
 - LSP: implement goto-definition on re-export
 - LSP: Fix goto-definition on properties with underscores
 - Design mode: fix dragging component on the last component of a file without trailing newline (#5695)
 - VSCode extension: add a language icon for .slint file.

## [1.7.1] - 2024-07-25

### General

 - Winit backend: When running under WSL, always connect to X11 to avoid triggering compositor crashes.
 - Winit backend: Fix showing a previously hidden Window on macOS.
 - Android: Fix build with older Java versions.

### Widgets

 - `ComboBox`: Fix size of popup when there are less than six items.

### Rust

 - Fix warning in generated code for unused enums.

### Node API

 - Improve error reporting by including the diagnostic in the exception's message.

### LSP and Tooling

 - Various fixes to Live-preview's edit mode:
   - Recognize `GridLayout` for dropping widgets into.
   - Remove `PopupWindow`, `Window`, and `Dialog` from the component library.
   - Add support for compnent libraries.
   - Fix panic when moving around widgets. (#5642)
   - Don't show unrelated accessibility properties.

## [1.7.0] - 2024-07-18

### General

 - Several compiler bugfixes. (#5260, #5246, #5220, #5259, #5249, #5430)
 - Android: fix cursor handle being visible despite inut loses focus. (#5233)
 - Android: fix keyboard poping up when application moved to foreground without input focus. (#5235)
 - Gettext translation: clear internal gettext cache when changing translations at runtime.
 - Winit backend: Fixed setting the size with set_size before showing the window. (#6489)
 - Winit backend: upgraded to winit 0.30, accesskit 0.16, glutin.
 - Winit backend: Fixed Window::hide on Wayland.
 - Qt backend: fix PopupWindow exiting the application with recent Qt6.
 - LinuxKMS backend: Added support for software rendering and legacy framebuffers.
 - Software renderer: Added `set_rendering_rotation()` to rotate the rendering by multiple of 90 degrees.
 - Software renderer: Fixed dirty region returned by `render()` to not have any overlaps.
 - Skia renderer: Fix quality of control rendering when using `cache-rendering-hint: true;`.
 - Skia renderer: Fix dithering of gradients (PR #5482)

### Slint Language

 - `Window`: Added `resize-border-width` property.
 - Support several exported component that inherits Window pr Dialog from the main .slint file
 - Mark exported component that doesn't inherit from Window or Dialog as deprecated
 - Deprecated generating the last import if there is no component in a file
 - Added `export { ... } from "...";` syntax
 - Add the `accessible-placeholder-text` property (PR #5464)
 - Added `char-wrap` variant to the `TextWrap` enum (PR #5381)
 - `Text` can be rotated
 - In `PopupWindow` permit access to own properties in bindings to `x`/`y`.

### Widgets

 - `TextEdit`: Added `placeholder-text` property.
 - `ComboBox`: Fixed `current-value` not updating when updating the model.
 - `ComboBox`: Fixed `current-value` not resetting when setting `current-index` to -1.
 - `ComboBox`: Added scrolling support.
 - `SpinBox`: Fixed issue where the text is not updated after value is changed from outside.
 - `SpinBox`: Added `step-size` property.
 - Added `TimePickerPopup` and `DatePickerPopup`.
 - Fixed accessible value and actions on `ProgressIndicator`, `Spinner`, `Spinbox`, `CheckBox`, `Switch`.

### C++ API

 - Added `LIBRARY_PATHS` multi-value argument to `slint_target_sources` that takes a list of
   `name=path` items, to allow for the use of component libraries.
 - Fixed compilation with Rust 1.81, caused by extra spaces in `strignify!`.
 - ESP-IDF: Added support for making RGB565 byte swap configurable.
 - Fix build with Rust 1.79.

### Rust API

 - Added missing implementation of the `Error` for some of the errors.
 - Allow all clippy warnings in generated code.
 - Added `slint::Image::to_rgb8()/to_rgba8()/to_rgba8_premultiplied()` to obtain pixels for a `slint::Image` if available.
 - Fix panic in `slint::Timer` when a new timer is started while stopping another.
 - Added `slint::Window::take_snapshot()`.

### Interpreter

 - Track model length changes when accessing a model out of bounds.
 - Added API to obtain list of functions.
 - Deprecated `slint_interpreter::ComponentCompiler` in favor of `slint_interpreter::Compiler` which
   supports compiling multiple components.

### Node API

 - Functions declared in Slint can now be invoked from JavaScript.

### LSP and Tooling

 - Added suggestion for imported globals in expression auto-completion.
 - Added ability to rename components.
 - Design mode: only allow to move element within the same component.
 - Design mode: Added the property editor pane.
 - viewer: added the `--component` command line arg to preview a specific component.


## [1.6.0] - 2024-05-13

### General

 - The minimum Rust version is now 1.73.
 - When the Skia renderer is enabled at compile time, it is picked as the default at run-time.
 - FemtoVG renderer: Fixed selection of italic font styles. (#5056)
 - Fixed color animation involving transparency. (#5063)
 - Android: Fixed support for Android 10 to 12 and Java 1.8.
 - Android: Fixed build if the locale is not UTF-8.
 - Android: Added cursor handle on TextInput and selection handle with cut/copy/paste menu.
 - Android: added `backend-android-activity-06` feature.
 - Software renderer: Dirty regions can now be composed of multiple rectangles.
 - Added a function to mark all translations as dirty.

### Slint Language

 - Text: Added `stroke`, `stroke-width`, and `stroke-style` properties.
 - Added `Colors.hsv()` method to create colors in the HSV color space.
 - Added `to-hsv()` function to color.
 - Throw an error when `rgb()` or `argb()` have too many arguments.
 - Added support for `accessible-action-*`.
 - Fixed insertion point of `@children` when it is not the last child. (#4935)
 - Deprecated usage of internal `StyleMetrics` in the public API.
 - Fixed compiler panic with state property change involving a state in a parent component. (#5038)
 - Fixed interpreter overwriting property named `index`. (#4961)
 - Fixed compiler panic when a callback aliases itself. (#4938)
 - Fixed compiler panic when an init from a repeater is inlined into the parent component (#5146)
 - Added `clear-focus()` function to focusable elements, to allow for programmatic focus clearing.

### Widgets

 - Palette: Added `color-scheme` in-out property for accessing the style's color scheme.
 - Accessibility: Annotated more widgets with accessible properties and actions.
 - Qt style: Fixed rendering of focused or hovered ComboBox and CheckBox.
 - Qt style: Fixed widget that would react to every mouse button instead of only the left button.
 - SpinBox: Fixed scroll direction.
 - Allow scrolling through tabs.
 - Updated TabWidget in Cosmic style.
 - Added a `released` callback to `Slider`.
 - Fixed text and selection color of TextEdit and LineEdit.
 - Spinbox and Slider: The value now defaults to the minimum.

### Rust API

 - Added conversion of Color to and from HSV.
 - Added getter to the `raw-window-handle` of a window using the `raw-window-handle-06` feature.

### C++ API

 - Workaround breaking change in the build with Rust 1.79 (see https://github.com/corrosion-rs/corrosion/issues/501)
 - Added conversion of Color to and from HSV.
 - Fixed code generation of functions that don't return.
 - Fixed the `MapModel::reset` function. (#4968)
 - Fixed compilation of the generated code when an animated brush property is set in a sling callback.
 - Added include guard to the generated header.

### LSP and tooling

 - Design mode of the live preview can now drag into and from layout. With a drop marker when dragging an element.
 - Fixed formatting of function declarations.
 - Added `-L` command line args to `slint-lsp` to specify path of external libraries (#5144)
 - VSCode extension highlights Slint code blocks in markdown files.
 - `slint-viewer` will properly reload files saved with neovim, which renames and replaces files (#3641)

## [1.5.1] - 2024-03-20

 - Fix clipping with a border-radius. (#4854)
 - Fix panic in the preview when showing a PopupWindow whose parent is optimized out. (#4884)
 - Fix compiler panic when the `focus` function is called with arguments. (#4883)
 - Fix panic when loading unsupported images.
 - LSP: Fixed formatting of states, transitions, and functions.
 - LSP preview: Avoid double scroll bar in the preview when showing errors.
 - LSP preview: Don't handle delete shortcut when showing errors.
 - LSP preview: Improved appearence of the element selection in the design mode.
 - LSP preview: Never set the with or height to NaN. (#4848)

## [1.5.0] - 2024-03-14

## General

 - Added support for Android via the `backend-android-activity-05` feature.
 - Added API for maximized/minimized window.
 - TextInput: Added undo/redo support.
 - ListView: Fixed redraw when model changes. (#4538)
 - Disabled Qt backend by default on Windows and Mac even when Qt is found.
 - Qt: Explicitly hide PopupWindow instead of relying on destructor.

### Slint Language

 - Rectangle: Added `border-{top,bottom}-{left,right}-radius`
 - Image: Added `ImageFit.preserve`
 - Image: Added `horizontal-` and `vertical-alignment`
 - Image: Added support for 9 slice scaling
 - Image: Added `horizontal-` and `vertical-tiling`
 - Flickable: Added `flicked` callback
 - Slint: Expose `.red`, `.green`, `.blue`, and `.alpha` properties on `color`

### Widgets

 - Fixed edited callback of SpinBox for Cupertino and Material style.
 - Cupertino TabWidget: Tweaked visual appearance

### Rust

 - Fixed ReverseModel and FilterModel model not always forwarding notification correctly.
 - Re-export more type in the slint-interpreter crate.
 - Added `SharedVector::pop`.
 - Use const generics for construction of SharedVector from array.

### C++

 - Fixed binary package that used to require Qt.
 - Added `Window::set_fullscreen`.
 - Fixed error in generated code when struct or enum has an underscore. (#4659)
 - Added `slint::interpreter::ComponentCompiler::set_translation_domain`.
 - Added `NAMESPACE` modifier in the `slint_target_sources` cmake macro to generate in a namespace.

### JavaScript

 - Fixed MapModel rowData() calling map function even if the source model returned undefined.
 - Better error reporting when the backend cannot be created.
 - Reading model properties now always returns a `Model<T>`, regardless of whether an array was previously assigned.
 - `Model<T>` now implements `Iterable<T>`.

### LSP

 - Added support for code formatting.
 - Sort properties first in auto-completion.
 - Fixed completion in two way bindings
 - Preview: Design mode with drag and drop
 - Fixed wasm embedded preview on Windows

## [1.4.1] - 2024-02-02

 - Skia: Update skia binding dependency.
 - Slintpad: Fixed initial rendering of the live-preview.
 - Qt backend: fix crash when closing popup on Wayland. (#4500)
 - Fixed rendering of linear gradient of 90deg. (#4495)
 - C++: Permit passing a value to VectorModel::set_vector by value (#4491)
 - slint-viewer: re-render after reloading when using software renderer.
 - Fixed panic in the software renderer when using the Qt style.
 - Rust: fix slint-build's formater when source contains `\"` in a string. (#4520)

## [1.4.0] - 2024-01-31

### General

 - Winit backend: Fixed `key-released` in `FocusScope` not being invoked when releasing the space bar key.
 - Fix `PopupWindow` close behavior: Close on release when the mouse is on the popup, and close on press when
   it's outside - to match standard behavior.
 - Fixed focus behavior on click in a TextInput
 - Fixed ListView not updating when model changes (#3125).
 - Fixed TextInput on Plasma/Wayland receiving many empty events causing selection to be cleared (#4148)
 - Added API to programmatically show a window in fullscreen mode (C++/Rust: `Window::set_fullscreen(bool)`, Node.js: `window.fullscreen`).
 - Added API to keep the event loop alive when the last window is closed (#1499). (Rust: `slint::run_event_loop_until_quit()`; C++: argument to `slint::run_event_loop()`; Node: argument to `runEventLoop`).
 - Fixed linear gradiant rendering in non square rectangles (#3730).
 - LinuxKMS backend: Added support rendering output rotation via the `SLINT_KMS_ROTATION` environment variable.
 - LinuxKMS backend: Added support for `backend-linuxkms-noseat` feature to compile without libseat.
 - LinuxKMS backend: Added support for software rendering with Skia.
 - LinuxKMS backend: Added frame throttling.

### Slint Language

 - `if` statements no longer require parentheses.
 - Added a `double-clicked` callback in `TouchArea`, which is triggered when a `TouchArea`
   is clicked twice in rapid succession.
 - The `pointer-event` callback in `TouchArea` is now triggered on mouse move
   as well.
 - Errors are thrown when trying to modify properties that must be known at compile time.
 - Fixed property wrongly considered as const if it is modified through an alias (#4241).
 - Fixed missing invocation of init callbacks due to inlining (#4317).
 - Added `Key.Space`` to `Key` namespace.

### Widgets

 - Fixed SpinBox not being enabled by default.
 - Fixed wrong text input in cupertino SpinBox.
 - Added focus state to `StandardListView`.
 - Added `colorize-icon` property to `Button`.
 - Added `set-selection-offsets(int, int)` to `TextInput`, `LineEdit`, and `TextEdit`.
 - Added `Palette` global singleton.
 - Added `Cosmic` style.
 - Improved `Slider` drag and click behaviour.

### C++

 - Added `ComponentInstance::definition()` getter to retrieve the `ComponentDefinition` for an instance.
 - Added `slint::VectorModel::clear()` and `slint::VectorModel::set_vector()` to conveniently clear or replace the underlying data.

### Rust

 - Compile-time improvements.
 - Fixed compilation when component has the same name as internal name (#4419).

### JavaScript

 - Pre-built binaries in the npm package.

### LSP

 - Added selection mode to select elements in the preview.
 - Implement code action to add missing import.
 - Fix error when going to the definition of builtin items (#4126).
 - Preserve underscores in property auto-completion.

## [1.3.2] - 2023-12-01

### General

 - Fixed `accepted` and `edited` callbacks in `LineEdit` not being invoked
   with Fluent, Cupertino, and Material styles.
 - Fixed coordinate of events within PopupWindow. (#4036)
 - Fixed ComboBox not selecting entries. (#4033)
 - Fixed singleshot timers started via `start(...)` to not stay in running state.
 - Fluent style: Fixed color of disabled `LineEdit`.

### Slint Language

 - Added `KeyEvent.repeat` to detect repeated key press events.

### LSP

 - Added support for resizing the UI under preview without resizing the window,
   by providing resize handles and scrollbars.
 - Close previous PopupWindow before refreshing the preview. (#4035)

### C++

 - Fixed compilation on Windows when cross-compiling (for example when using esp-idf).

## [1.3.1] - 2023-11-28

### General

 - Bump various dependencies.
 - Fixed `has-hover` and `mouse-cursor` when opening a `PopupWindow`. (#3934)
 - Fluent style: fixed scrollbar size. (#3939 / #3932)
 - Skia Vulkan renderer: Fixed crash when resizing windows on X11.
 - Fixed cursor of LineEdit with right alignment (#4016)

### Slint Language

 - Added `clamp` function that takes a `value`, `minimum` and `maxium` and will return
   `maximum` if `value > maximum`, `minimum` if `value < minimum` or `value` otherwise.
 - Throw Slint error when returning no value when one is expected instead of generating invalid code. (#3962)
 - Fixed compiler panic when a component is called `Window`. (#3916)

### Rust API

 - Implement `std::error::Error` for `LoadImageError`.

### JavaScript API

 - Added `loadSource` function (#3971)
 - Added `requestRedraw` to Window (#3940)

### C++ API

 - Fixed undefined behavior in `SharedString::end()`

### LSP

 - Fix "recursion detected" panic in the preview with `forward-focus`. (#3950)
 - Don't expose empty name in the outline, this caused error in vscode. (#3979)
 - Fix enum ranges in the outline.
 - Added `--fullscreen` command line option.

## [1.3.0] - 2023-11-10

### General

 - The minimum Rust version is now 1.70.
 - The `SLINT_DEBUG_PERFORMANCE` environment variable is now also compatible with the software renderer.
 - Fixed issues with text rendering and cursor positioning in elided or aligned multi-line text.
 - The default style has been changed to `fluent` on Windows and `cupertino` on macOS.
 - LinuxKMS backend: Added support for absolute motion pointer events, fixed support for touch input on scaled screens, and improved encoder/CRTC handling for EGL rendering.
 - Skia renderer / winit backend: Fall back to Skia software rendering when GPU acceleration is not available.
 - Fixed a bug where accessing model data in a callback after changing it within the same callback did not reflect the update. (#3740)

### Slint Language

 - Added `Number`, `Decimal` variants to the `InputType` enum.
 - Added `spacing-horizontal` and `spacing-vertical` to `GridLayout`.
 - Fixed conversion in an array of an array of structs (#3574).
 - Added `scroll-event` callback to `TouchArea`.
 - Added support for `protected` functions.
 - `ComboBox` selection can now be changed by a scroll event.
 - `SpinBox` value can now be incremented and decremented by a scroll event.
 - Added `focus-changed-event` callback to `FocusScope`.
 - Added many new easing curves.
 - Added `Spinner`.
 - Added `Palette` global.

### JavaScript

 - The system has been ported to napi-rs.
 - The API has been refreshed.

### Rust

 - Improved support for the `slint!` macro for rust-analyzer.
 - Added `source_model()` to `MapModel`, `FilterModel`, `SortModel`, `ReverseModel` to access the inner model.

### C++

 - Removed the need for C++ exceptions in generated code.
 - Added the ability to only build the Slint compiler or use an external compiler.
 - ESP-IDF: Wait for vsync before swapping frame buffers.
 - Fixed a crash when accessing an empty model from Slint.

### LSP

 - Added "Wrap in element", "Remove element", "Repeat element", and "Make conditional" code actions.
 - Added a toolbar with a style picker in the preview.

## [1.2.2] - 2023-10-02

### General

 - Skia renderer: Fixed the `source` property of `Image` elements sometimes not changing when setting dynamically loaded images. (#3510)
 - Fix compiler panic with `popup.close()` from outside of the popup. (#3513)
 - Fixed native style (Qt) not finishing its animations with Breeze. (#3482)
 - Fixed native style not clipping correctly GroupBox. (#3541)
 - Fixed native style ComboBox not always being shown. (#3527)
 - Winit backend: Fixed window resizing on macOS Sonoma. (#3559)
 - Skia / FemtoVG renderers: Default to vsync swap interval when rendering with OpenGL to avoid excessive CPU usage. (#3516)

### C++

 - Fixed cross-compilation with CMake (against Yocto SDKs)

## [1.2.1] - 2023-09-19

### General

 - Fixed generated C++ and Rust code in conversion from unnamed to named struct in complex expressions. (#2765)
 - Improved wasm preview in the documentation, especially on mobile. (#3389)
 - Improved `StandardTableView` to use `ListView` optimization for all styles. (#3425)
 - Fixed in WSL against a distro that uses X11 instead of Wayland. (#3406)
 - Added documentation about different renderers and their relation to backends.
 - Added support for partial rendering to the software renderer when used with the winit backend. (#3457)
 - Fixed builds of the Skia renderer on Windows when built from source within `vcvars.bat` environments.
 - Updated to Skia milestone 117.
 - Fixed panic when using `SLINT_FULLSCREEN=1` in comination with the Skia OpenGL renderer. (#3472)
 - Native style: Fixed scroll bar handle size for `ScrollView`. (#3489)
 - FemtoVG renderer: Fixed empty lines and vertical alignment when eliding text. (#3481)
 - Skia renderer: Fixed eliding of multi-line text. (#3481)
 - `StandardTableView`: Fixed scrollbar only responding to the second click.
 - Polished the experimental Cupertino style further.

### Rust API

 - Derive serde traits for `slint::Color`, when the feature is active. (#3411)

### C++

 - Fixed crash when code in a `clicked` handler in a sub-component would cause a conditional
   or repeated element to disappear. (#3465)
 - Fixed Skia renderer builds finding clang correctly.

### VS Code Extension

 - Fixed "Show Preview" command when invoked from the command palette. (#3412)

### Language Server

 - Fixed automatic indentation within named elements. (#3409)
 - Fixed panic when editing path in import statements. (#3468)


## [1.2.0] - 2023-09-04

### General

 - Fixed accessibility tree on Linux when UI has no repeaters.
 - Fixed native style animations.
 - Fixed setting rotation-angle and opacity from a callback.
 - Fixed touch in the `Flickable` not resulting in a click.
 - Added support for a new experimental backend that renders fullscreen on Linux using KMS (`backend-linuxkms`).
 - Calling `show()` on a component (or its window) now keeps the component alive for as long as the window
   is visible.

### Slint Language

 - Improve reporting of diagnostics when there are errors, by attempting to run more passes.
 - Fixed compiler panic when an unresolved alias has a binding.
 - Added `edited` callback to `SpinBox`.
 - Added `row-pointer-event` callback to `StandardTableView`.
 - Fixed enabled property with `ComboBox` in Fluent Design style.
 - Fixed duplicated import when importing file relative to the project instead of the current path. Deprecated importing files relative to the project path.
 - Added `current-item-changed` to `StandardListView`.
 - Added `current-row-changed` to `StandardTableView`.
 - Added `item-pointer-event` to `StandardListView`.
 - Added `orientation` property to `Slider`.
 - Added experimental `cupertino` style.

### Rust API

 - Implemented `Default` for `slint::Weak`.
 - Added `ReverseModel` and `ModelExt::reverse`.
 - Added `fn set_visible(&self, _visible: bool)` to the `slint::platform::WindowAdapter` trait.
 - Added ways to create a `SoftwareRenderer` without a `MinimalSoftwareWindow`.
 - The features `renderer-winit-*` were renamed to `renderer-*`.
 - Added `BorrowedOpenGLTextureBuilder` to configure more aspects of borrowed OpenGL textures.

### C++

 - Added Platform API to write your own platform that drives its own event loop.
 - Added `SLINT_LIBRARY_CARGO_FLAGS` cmake variable.
 - Added `ReverseModel`.
 - Added functions in Window to dispatch pointer events.
 - The `slint_interpreter.h` file was renamed `slint-interpreter.h`, a deprecated header was added.
 - The features `SLINT_FEATURE_RENDERER_WINIT_*` were renamed to `SLINT_FEATURE_RENDERER_*`.
 - Extended `slint::Image::create_from_borrowed_gl_2d_rgba_texture` with an option to configure more aspects
   of texture rendering.
 - Fixed cmake dependencies of the generated header so it is generated if and only if the .slint files have changed

### LSP

 - Fixed termination of the lsp process.

## [1.1.1] - 2023-07-10

### General

 - Fixed panic in accesskit at startup on Linux. (#3055)
 - Fixed compiler panics when some complex expressions are used for the model expression in `for` (#2977)
 - Native style: Fixed support for floating point ranges in Slider.
 - Fixed panics in the software renderer related to text rendering.

### Slint Language

- Added `clear-selection()` to `TextInput`, `LineEdit`, and `TextEdit`.
- The `PointerEvent` struct now has the `modifiers: KeyboardModifiers` field.

### C++

 - Added `slint::Window::scale_factor()` as getter to read the system device pixel ratio.

### LSP

 - Correctly use the CARGO_MANIFEST_DIR as the base for import and image in `slint!` macro


## [1.1.0] - 2023-06-26

### General

 - Fixed missing items compilation error in the generated code related to public functions (#2655).
 - Added support for Window transparency on supported platforms.
 - Fixed TabWidget not filling the parent in non-native style.
 - Skia: Add support for rendering with Vulkan.
 - Wasm: Added copy and paste support.
 - Fixed TouchArea::has-hover not being reset in some cases involving multiple `TouchArea` or `Flickable` elements.
 - Fixed ListView panic when model reset in some cases. (#2780)
 - Fixed read-only `TextInput` reporting input method access. (#2812)
 - Fixed `LineEdit`'s placeholder text not being rendered with the same font attributes as regular `LineEdit` text.
 - Fixed rendering of SVGs with text. (#2646)
 - Software renderer: Show the cursor in TextInput

### Slint Language

 - Added support for declaring enumerations.
 - Added support negative numbers in `cubic-bezier(...)` function.
 - Added `ProgressIndicator` widget.
 - Added `Switch` widget.
 - Added boolean `font-italic` property to `Text` and `TextInput`.
 - Added `select-all()`, `cut()`, `copy()`, and `paste()` to `TextInput`, `LineEdit`, and `TextEdit`.
 - Added functions on color: `transparentize`, `mix`, and `with-alpha`.
 - Added a `close()` function and a `close-on-click` boolean property to `PopupWindow`.
 - Added basic translation infrastructure with `@tr("...")`.
 - Added `absolute-position` property to every element, for computing window-absolute positions.
 - Added `primary` boolean property to `Button` to configure its visual appearance.
 - Added `current-row` to `StandardTableView`.

### Rust

 - Added `slint::Image::load_from_svg_data(buffer: &[u8])` to load SVGs from memory.
 - Added `slint::platform::WindowEvent::ScaleFactorChanged` to allow backends to report the current window scale factor.
 - Added `slint::Image::from_borrowed_gl_2d_rgba_texture` to create images from borrowed OpenGL textures.
 - In the Slint language, struct can be annotated with `@rust-attr(...)` that is forwarded as a Rust attribute (`#[...]`) for the generated struct.
 - Added a `serde` feature to enable serialization of some Slint data structures.
 - Added convenience `From` conversions for `ModelRc` from slices and arrays.
 - `slint-viewer` gained the ability to preview .rs files with a `slint!` macro.
 - Added a `spawn_local` function to run async code in the Slint event loop.

### C++

 - Added `slint::Image::create_from_borrowed_gl_2d_rgba_texture` to create images from borrowed OpenGL textures.
 - Added `[[nodiscard]]` in a function signatures.
 - Experimental: the `slint::platform::WindowAdapter` no longer takes a template parameter and has a different constructor signature.

### LSP

 - Fixed auto-completion of local properties or arguments in callbacks or functions.
 - Fixed panic when the editor tries to open non-local urls.

### VSCode extension

 - Make a visible error message when the `slint-lsp` panics.

## [1.0.2] - 2023-04-26

### General

 - Fixed the compiler embedding images unnecessarily multiple times when referenced via different relative paths. (#2608)
 - Winit backend: Adjust the window size automatically when an update minimum or maximum size would constrain the existing size.
 - Winit backend: Added internal API in the `i-slint-backend-winit` crate to access the winit window
 - Fix focussing element whose base is focusable (#2622)
 - Fix infinite loop in the compiler when there is a loop in `forward-focus`
 - Skia renderer: Add support for password input fields.

### C++

 - Fix build against macOS deployment target 10.10

### VSCode extension

 - Fixed preview not working (#2609)
 - Added design mode commands
 - Browser extension: fix preview not previewing specific component

### SlintPad

 - Fix editing in non-main files was getting ignored (#2630)


## [1.0.1] - 2023-04-20

### General

 - Fixed compiler panic when binding `Path`'s `commands` property to the field of a model entry.
 - Qt renderer: Fixed support for horizontal alignment in `TextInput`.
 - Winit backend: Fix detect of dark color scheme in some circumstances.
 - ListView: fix resizing a ListView to empty height would make all items invisible even if resized back (#2545)
 - Fixed compiler panic when accessing unset layout properties such as `spacing` or `alignment` (#2483)
 - Fixed compiler panic when accessing property from parent context in a `init =>` callback
 - Load fontconfig with dlopen instead of dynamic linking.
 - Software renderer: renders the text in TextInput
 - Fixed `TouchArea::has-hover` not becoming false when items become invisible

### Slint Language

 - Fixed parent `FocusScope` objects stealing the focus from inner `FocusScope`s when clicked.
 - Added `TextInputInterface.text-input-focused` to detect when a virtual keyboard should open
 - Added `always-on-top` property of a `Window` to show the window above others
 - The error message when referring to an id reports a suggestion if there is a enum value or a property with the same name.

### C++

 - macOS: Fixed `install_name` for `libslint_cpp.dylib` use `@rpath` instead of absolute paths to the build directory.
 - Fixed memory corruption when embedding images in generated C++ code.
 - Add support for dispatching key press and key release events to `slint::Window` via `dispatch_key_*_event`. This replaces
   the experimental `slint::experimental::platform::WindowAdapter::dispatch_key_event`.
 - MSVC: /bigobj is enabled by default as compile option when linking against the Slint CMake target, removing the need
   for users who run into linking issues to add that to their build.

### LSP

 - Don't throw a protocol error when using the rename function on a symbol that can't be renamed
 - Always auto-complete widgets from the style even if no widgets is imported
 - Don't auto-complete reserved properties or sub components for globals
 - Auto-completion in the document root (component, import, global)

### VSCode extension

 - Added an option to show the preview with wasm in a vscode tab (just like in the online version)
 - Make code lenses work better on Windows

### SlintPad

 - Warn when ServiceWorker could not get set up (e.g. in private browsing mode on Firefox)
 - Add design mode


## [1.0.0] - 2023-04-03

### General

 - `TextEdit` performs better with the FemtoVG renderer when showing many lines.
 - Software renderer: Added support for linear-gradients.
 - Software renderer: Fixed artifacts when components are deleted or elements become invisible.
 - Fixed Infinite loop in the accessibility backend on Windows (#2195).
 - Skia renderer: Enable anti-aliasing for rectangles with border radius.
 - Fluent style: Adjust disabled scrollbar background color.
 - Fixed several panics in the compiler (#2312, #2274, #2319).
 - Winit backend: Fix rendering when moving windows between monitors with a different scale factor (#2282).

### Slint Language

 - The old syntax that declares component with `:=` is now deprecated
 - `Flickable`: don't forward events if the flickable is dragged in a direction that can be dragged, even if at the bounds.
 - The `TextEdit` and `LineEdit` elements now correctly handle double click and tripple click to select words or lines,
   as well as support for the "primary" clipboard on X11 and wayland (select to copy, and middle click to paste).

### Rust

 - Minimum Rust version is now 1.66.
 - **Breaking:** Deprecated functions and enums were removed from the API.
 - **Breaking:** `PointerEventButton::None` was renamed `PointerEventButton::Other`
 - **Breaking:** More functions now return `Result`, and the return value needs to be `unwrap()`'ed.
 - **Breaking:** A lifetime parameter was added to `slint::ModelPeer`.
 - **Breaking:** `StandardListViewItem` and `TableColumn` are now marked as `#[non_exhaustive]`.
 - **Breaking:** Removed the `stride()` function from `slint::Image` - use `width()` instead.
 - **Breaking:** In `slint::platform::WindowEvent::KeyPressed` and `KeyReleased`, the `text` field is now
   a `slint::SharedString`.
 - **Breaking:** `slint::platform::WindowEvent` does not derive from `Copy` anymore. You must `clone()` it
   explicitly if you want to create a copy.
 - **Breaking:** The `MAX_BUFFER_AGE` const parameter of `slint::platform::software_renderer::MinimalSoftwareWindow`
   has been removed and replaced by an argument to the `new()` function.
 - **Breaking:** The `compat-0-3-0` mandatory cargo feature flag was renamed to `compat-1-0`.
 - Added a `software-renderer-systemfonts` feature to the Rust crate, to enable the use of fonts from the operating system
   for text rendering with the software renderer.
 - Fixed some clippy warnings in the generated Rust code.

### C++

 - Functions that take a functor as argument are now using C++ 20 concepts.
 - **Breaking:** In the C++ API, the type for row indexes in models was changed from `int` to `size_t`.
   This includes arguments of virtual functions in `slint::Model` that needs to be adjusted in
   derived classes.
 - **Breaking:** The order of C++ generated struct members is now preserved from the .slint source file.
 - Add constructors to `slint::Image` to create images from raw pixel data.
 - In C++ builds, support all image formats the Rust image-rs crate supports, not just png and jpeg.
 - Added the `SLINT_FEATURE_RENDERER_WINIT_SOFTWARE` option to enable the software renderer.

### Tooling

 - LSP: don't add spaces when auto-completing elements or callbacks, leads to better formatting.
 - The online editor was renamed to SlintPad.

## [0.3.5] - 2023-02-21

### Fixed

 - FemtoVG: Fix cursor placement at end of lines
 - Windows: Fix panic when minimizing windows
 - Fixed some panics in the software renderer when using text
 - Fixed panic when parsing invalid linear gradient syntax
 - Qt: Fixed mapping of the control key
 - Linux: Fix panic when using GLX
 - Fixed const detection when using two-way bindings
 - Fix run-time panic when combining forward-focus with text rendering in no_std environments
 - Rust: Fix compile issues when generated code clashes with user types
 - C++: Fix compilation with structs that have a field with the same name as the struct
 - Fix visible property on the root of a component
 - Fix compiler panic when an component's base is not inlined
 - C++: fix including the generated file in several translation units
 - C++: Fix "make install" on Windows

### Added

 - The StandardListView provides now a set-current-item function to set the selected item and bring it into view.

## [0.3.4] - 2023-01-25

### Changed

 - A new syntax is available that declares the components with `component Xxx { ... }`  instead of `Xxx := ...`.
   The old syntax continues to work in this release.
 - A warning is shown when calling non-pure function or callback from pure context (eg: a property binding).
   Callbacks and functions can be annotated with `pure`.
 - On an `Image`, the default value of `source-clip-width` and `source-clip-height` is now set to
   the size of the image minus the `source-clip-{x,y}`. The source clip size is now used to compute
   the default aspect ratio of the image.
 - Deprecated `invoke_callback` functions in the slint interpreter in favor of `invoke`, which can also
   invoke functions in addition to callbacks.
 - Warn if the last component or singleton in a file is implicitly marked for exporting.

### Added

 - `StandardTableView` widget.
 - LSP: support of Slint features (auto-complete, preview, ...) in `slint!{}` macro in Rust files.
 - The software renderer can now load fonts at run-time, without the need to pre-render glyphs
   at compile time.
 - The preview in the online editor, VS Code extension, and the VS Code web extension highlight
   components while the cursor is on the component type name.

### Fixed

 - Compiler panic for files containing no component (#2005).
 - Calling public functions from native code.
 - Fixed crash when using repeaters in C++ on 32-bit architectures.
 - Conversion of array literal containing struct with array literal (#2023).
 - Structs exported by the main file are always generated in native code (#594).

## [0.3.3] - 2022-12-16

### Changed

 - Minimum rust version is now 1.64.

### Added

 - Added support for declaring functions in the language.
 - Added support for re-exporting types from other files using `export * from "./other_file.slint";`.
 - LSP: highlight and renaming of element id.
 - online editor: Add zoom to the preview.
 - VSCode and online editor: Added UI to edit and remove property bindings.

### Fixed

 - Fixed privacy rules in global properties. #1951
 - Fixed drawing of images with the software renderer.
 - Fixed `visible` property on element with drop shadow. #1460
 - Warn on bad use of ListView. #860
 - Fixed two way bindings between globals.
 - Fixed scrolling of preview area in online editor.
 - Fixed the CMake build to respect `CMAKE_*_OUTPUT_DIRECTORY` variables. #1979
 - Fixed build when using `renderer-winit-skia-opengl` on macOS or Windows.


## [0.3.2] - 2022-11-28

### Changed

 - When using a two way binding `foo <=> bar`, the default value will always be the one of `bar`.
   There was a warning about this change in previous versions (#1394).
 - Disallow overrides or duplicated declarations of callbacks. Previously they were silently overwritten,
   now an error is produced.
 - The name `init` is now a reserved name in callbacks and properties.
 - In the interpreter, calling `set_property` or `get_property` on properties of the base returns an error.
 - Deprecated `slint::WindowEvent` and `slint::PointerEventButton` and moved them to the `slint::platform` module.
 - Renamed the `Keys` namespace for use in `key-pressed`/`key-released` callbacks to `Key`. The
   old name continues to work.
 - The style or the backend now always set a value for the `Window`'s `default-font-size` property.
 - In the Rust API, the `GraphicsAPI`'s `NativeOpenGL` variant uses a function signature for `get_proc_address` that
   takes a `&std::ffi::CStr` instead of a `&str` to match the native APIs and avoid unnecessary conversions.

### Added

 - Added new `material` style based on Material Design, with `material-light` and `material-dark` as variants.
 - Added `Window::is_visible` in Rust and C++.
 - Added `From<char>` for `SharedString` in Rust.
 - Added `KeyPressed` and `KeyReleased` variants to `slint::WindowEvent` in Rust, along
   with `slint::platform::Key`, for use by custom platform backends.
 - Added support for the implicitly declared `init` callback that can be used to run code when
   an element or component is instantiated.
 - Properties can be annotated with `in`, `out`, `in-out`, or `private`.
 - Transitions can now be declared directly within the state.
 - Online editor: The property view can now edit properties.
 - LSP preview: When the cursor is on the name of an element, the element's location is highlighted in the preview.
 - LSP: Added a setting to change the style and the include paths.
 - VSCode extension: added the property view.
 - Added `rem` as unit that represents a relative font size and is multiplied with the `Window.default-font-size` when used.
 - Added `relative-font-size` as type that holds values of `rem`.

### Fixed

 - Fluent style: SpinBox gets focus when buttons are clicked.
 - Software renderer: Fix drawing the background color.
 - Fix Rust code generation when comparing percentages (#1837).
 - Fix `visible` property in `for` and `if` (#1846).

## [0.3.1] - 2022-10-28

### Changed

 - The property `Window::background` is now a brush instead of a color (allowing gradients).
 - Switch to yeslogic-fontconfig-sys from servo-fontconfig dependency. This allows for fontconfig
   to be a run-time dependency via dlopen.
 - Skia renderer: Improvements to text input.

### Added

 - Added `slint::FilterModel`, `slint::MapModel` to the C++ API.
 - Added `slint::SortModel` to Rust and C++ API.
 - Added `VecModel::extend` and `VecModel::extend_from_slice`.
 - Online editor: Added "Properties" and "Outline" tabs.
 - Added initial support for input methods with pre-edit text.
 - Added a dark theme for the Fluent style, which is automatically selected if the system
   color scheme is dark.
 - Added `fluent-light` and `fluent-dark` as explicit styles to select a light/dark variant,
   regardless of the system color scheme setting.

### Fixed

 - TextInput now shows the text mouse cursor.
 - In Flickable, added a small delay before passing the Press pointer event to the children.
 - Online editor: Fixed "go to definition" across files.
 - Fixed a panic in the slint compiler when visiting layout properties for loop analysis (#1659).
 - Fixed compilation error in the generated code (#1733, #1735).

## [0.3.0] - 2022-09-14

### Breaking Changes

 - `mod` now works on any numeric type, not only integers.
 - Minimum rust version is now 1.60.
 - The "backend-gl-*" Rust crate features for configuring the GL renderer have been
   changed and split by renderer.
 - `VecModel::remove` now returns the removed element.
 - `slint::invoke_from_event_loop` and `slint::quit_event_loop` now return a Result.

### Added

 - Added the `platform` module providing API to use slint on bare metal with a software renderer.
 - Added an experimental Skia renderer.
 - `Button`: Add a `checkable` property that turns the button into a toggle
   button. Use the new `checked` property to query whether the toggle button
   is pressed down or not.
 - Added support for `slint::Window::set_position` and `slint::Window::position` to set and get the
   placement of the window on the screen.
 - Added `slint::Window::scale_factor()` as getter to read the system device pixel ratio.
 - Added support for `slint::Window::set_size` and `slint::Window::size` to set and get the
   size of the window on the screen.
 - Added `slint::Window::dispatch_event` and `slint::WindowEvent` to be able to manually
   send a mouse or touch event to a window.
 - Added `animation-tick()`.
 - `SharedString` implements `std::fmt::Write` and added `slint::format!`.
 - `Image` can now be rotated with the `rotation-*` properties.
 - Use docking widgets and integration of slint-lsp into the [Online Code Editor](https://slint.dev/editor).


### Fixed
 - Fixed Ctrl+Backspace/Ctrl+Del not deleting words in text input elements.
 - Resizing of live-preview window in the IDE integrations.
 - Preferred size of the TabWidget in the fluent style take in account the size of the tabs (#1363).
 - Fixed cursor behavior when typing the Enter key at the end of a TextEdit (#1318).
 - Fixed a memory leak of images when using the GL backend.
 - Fixed starting and stopping `slint::Timer` from withing their callback (#1532).

## [0.2.5] - 2022-07-06

### Changed

 - Interpreter: Implement `TryFrom<Value>` instead of `TryInto for Value` (#1258)

### Added

 - Added the Model Adapters `FilterModel` and `MapModel`.
 - Added `@radial-gradient(circle, ...)`
 - Added `read-only` property to `TextInput`, `TextEdit` and `LineEdit`.
 - VSCode extension can be installed as a web extension. (eg, from https://vscode.dev)
 - LSP: completion of `@` macros
 - LSP: completion of element that require an import
 - Basic accessibility support using the `accessible-` properties

### Fixed

 - GL backend: Fixed animation sometimes not starting from input event (#1255)
 - C++ fix compilation when writing to the model data
 - Fix mouse exit events not triggered by scrolling a Flickable (#1107)

## [0.2.4] - 2022-05-09

 - Fixed crash when opening a native (Qt) ComboBox

## [0.2.3] - 2022-05-09

### Fixed

 - Fixed crashes with the Qt backend in release mode. (#1230)
 - Fixed panic when drop-shadow is used in a ListView (#1233)
 - Fixed combining a brush and a color to always convert to brush, to avoid loosing gradient information (#1235)
 - Fixed properties not having the right default value when set by some states (#1237)
 - Fixed properties with multiples aliases, and default values.
 - Enable fontdb's fontconfig feature to fix finding some fonts (#1240)

## [0.2.2] - 2022-05-04

### Changed
 - On wasm, the input event are handled via a hidden `<input>` element, allowing the keyboard
   to show on mobile platform
 - The size of the window is kept when reloading a window in the preview (instead of being reset to the preferred size)
 - Minimum rust version is now 1.59

### Added

 - Support for keyboard focus with the tab key
 - Support more keyboard shortcut in the editing element
 - Added `From<&str>` and `From<SharedString>` to `StandardListViewItem` to make creation and modification of `StandardListView`'s models easier.
 - Added `on_close_requested` function to `Window` to register callbacks that are emitted when the user tries to close a window.
 - Added `VecModel::set_vec` to replace the entire contents with new data.
 - Added a `cache-rendering-hint` boolean property that can be applied to any element, to hint to the renderer that it should cache the element and its children
   into a cached layer. This may speed up rendering of complex sub-trees if they rarely change.
 - The `brighter` and `lighter` functions also work on values of type brush.
 - Added a `reset` function to C++'s `Model`, Rust's `ModelNotify` and JS's `ModelPeer`
 - Added a `row_data_tracked` function to `ModelExt` (an extension to the Model trait)

### Fixed

 - Fixed application of the `opacity` property evenly to child elements (#725).
 - Windows: Fixed font lookup of strings including several scripts (eg, containing asian characters)
 - Fixed PopupWindow in a repeater (#1113, #1132)
 - LSP: do not always resize the preview window to its preferred each time the code is modified

## [0.2.1] - 2022-03-10

### Added

 - C++ interpreter API: added a `Value::Value(int)` constructor
 - Global singletons in `.slint` files may now refer to other global singletons
 - Added `input-type` property to `TextInput` and `LineEdit` that allows for characters to be replaced in password fields
 - The `SpinBox` widget now handles up and down key events

### Fixed

 - `TouchArea::has-hover` is now properly set to false when the mouse leaves the window
 - Fixed some cases of panics with 'Constant property being changed'
 - Fixed `Flickable` animation
 - Better compilation error when selecting unknown styles
 - Fixed duplicated key event for some keys (such as tab) with the GL backend
 - Improved property optimizations by inlining some bindings and remove more unused properties

## [0.2.0] - 2022-02-10

This version changes some APIs in incompatible ways. For details how to migrate your application code, see the [C++ migration guide](api/cpp/docs/cpp_migration.md)
as well as the [Rust migration guide for the `sixtyfps` crate](api/rs/slint/migration.md) and for the
[`slint-interpreter` crate](internal/interpreter/migration.rs).

### Changed

 - Minimum rust version is now 1.56
 - C++ compiler requires C++20
 - In the C++ interpreter API `std::span` is used for callbacks arguments, instead of `sixtyfps::Slice`
 - `Model::row_data` will now return a `Option<T>` / `std::optional<T>` instead of a plain `T`.
 - `Model::model_tracker` no longer has a default implementation.
 - The deprecated methods `Model::attach_peer` and `ModelNotify::attach` were removed.
 - The interpreter does not differentiate anymore between `Value::Array` and `Value::Model`
   everything is a `Value::Model`, which now contains a `ModelRc`
 - In Rust, `slint::SharedPixelBuffer` and `slint::SharedImageBuffer` now use a `u32` instead of `usize` for `width`, `height` and `stride`.
 - In Rust and C++, `slint::Image::size()` now returns an integer size type.
 - `sixtyfps::interpreter::CallCallbackError` was renamed to `slint::interpreter::InvokeCallbackError`
 - Some deprecation warning in .60 became hard errors
 - Replace `ModelHandle` with `ModelRc`
 - `slint::interpreter::ComponentInstance` in Rust now implements `slint::ComponentHandle`. This removes `sixtyfps_interpreter::WeakComponentInstance` in favor
   of `slint_interpreter::Weak<slint_interpreter::ComponentInstance>`.
 - For the Rust API crate, the Rust Interpreter API crate, the `backend-gl`, `x11`, and `wayland` features were renamed to `backend-gl-all`, `backend-gl-x11`, and `backend-gl-wayland`.
 - For the C++ CMake interface, the `SIXTYFPS_BACKEND_GL`, `SIXTYFPS_FEATURE_X11`, and `SIXTYFPS_FEATURE_WAYLAND` options were renamed to `SLINT_BACKEND_GL_ALL`, `SLINT_FEATURE_BACKEND_GL_X11`, and `SLINT_FEATURE_BACKEND_GL_WAYLAND`.
 - The animation `loop-count` property was replaced by `iteration-count` (which is the same as `loop-count` plus one)

### Added

 - `TextEdit::font-size` and `LineEdit::font-size` have been added to control the size of these widgets.
 - Added `slint::Window::set_rendering_notifier` to get a callback before and after a new frame is being rendered.
 - Added `slint::Window::request_redraw()` to schedule redrawing of the window contents.

### Fixed

 - Models initialized from arrays are now also mutable when run in the interpreter.
 - Fixed compilation error when accessing object members of array through the array index syntax

## [0.1.6] - 2022-01-21

### Changed

 - **Breaking:** The internal key code for the keys left, right, home and end
   has changed. This was undocumented, but if you were handling this in the
   `FocusScope` event, these keys will now be ignored. Use the `Keys.LeftArrow`
   and other code exposed in the `Keys` namespace instead.
 - For `sixtyfps::Timer` (C++ and Rust), it's now possible to call `restart()` after
   a timer has been stopped previously by calling `stop()`.
 - Property access in `.60` was optimized by doing more constant propagation.

### Added

 - Color names can now be accessed through the `Colors` namespace (in `.60`).
 - Math function are now available through the `Math` namespace (in `.60`).
 - `TouchArea` gained a `mouse-cursor` property to change the mouse cursor.
 - C++: Added `SIXTYFPS_VERSION_MAJOR`/`SIXTYFPS_VERSION_MINOR`/`SIXTYFPS_VERSION_PATCH` version macros.
 - More special keyboard key codes are provided in the `FocusScope`, and
   special keys are handled
 - `start()`, `stop()`, `running()` and a default constructor for C++ `sixtyfps::Timer`
 - Added math functions `log`, and `pow`.
 - Property animations now have a `delay` property, which will delay the start
   of the animation. Use this to create sequential animations.
 - Rust: Added `sixtyfps::VecModel::insert(&self, index, value)`.
 - C++: Added `sixtyfps::VecModel::insert(index, value)`.
 - Added ability to access elements of a model with the `[index]` syntax.

### Fixed

 - Memory leak in C++.
 - Native style: Colors are updated automatically when the Windowing system switches to and from dark mode (#687)
 - Ctrl/Command+X in text fields copies the selected text to the clipboard and deletes it (cut).
 - Improved native ComboBox look.
 - Fixed panics or compilation error when using two way bindings on global properties.

## [0.1.5] - 2021-11-24

### Changed

 - The sixtyfps compiler no longer "inline" all the elements, resulting in faster compilation
   time and smaller binaries.
 - Implemented basic constant propagation in the sixtyfps compiler

### Fixed

 - The Slider's changed callback was not being called with the fluent style (#621).
 - Fixed compilation error in C++'s `sixtyfps::blocking_invoke_from_main_loop` when the callable returns `void` (#623).
 - Improve rendering quality on High-DPI screens on Windows when using Qt.
 - Fixed linking errors when selecting the native style on Windows with C++.
 - Fixed the maximization button in the window decoration when a window has a fixed size.

## [0.1.4] - 2021-10-22

### Changed

 - The TouchArea now grabs the mouse for every button instead of just the left button.
 - The ScrollView's default viewport size is no longer hardcoded to 1000px but depends on the contents.
 - In Rust, the `sixtyfps::Model` trait deprecates the `attach_peer` function in favor of `model_tracker`, where all
   you need to do is return a reference to your `sixtyfps::ModelNotify` field.

### Added

 - Enable support for compressed SVG (.svgz).
 - Viewer: added possibility to set shell callback handler with `--on <callback> <handler>`.
 - It is now possible to query the length of a model with `.length`.

### Fixed

 - Fixed the `PointerEvent.kind` always being down.
 - `LineEdit.has-hocus` with the native style

## [0.1.3] - 2021-10-06

### Changed

 - Due to changes in the build system, the C++ build now requires CMake >= 3.19.
 - Fluent style: The Slider and ScrollBar now updates as the mouse moves.
 - Parentheses around callable expression is now deprecated.
 - Naming a callback without calling it is now a hard error instead of producing error in the generated code.

### Added

 - New `no-frame` property of a `Window` which changes it to borderless/frameless
 - sixtyfps-compiler and slint-viewer can read the .60 file content from stdin by passing `-`
 - slint-viewer gained ability to read or save the property values to a json file with `--save-data` and `--load-data`
 - New `StandardButton` widget
 - New `Dialog` element
 - `sixtyfps::Image` has now a `path()` accessor function in Rust and C++ to access the optional path
   of the file on disk that's backing the image.
 - New `moved` and `pointer-event` callback in `TouchArea`
 - New `AboutSixtyFPS` widget

### Fixed

 - Fixed panic when using `TabWidget` with `Text` elements and the native style.
 - Fixed panic when calling `hide()` on a `sixtyfps::Window` from within a callback triggered by keyboard/mouse input
   when using the GL backend.
 - Rust: The implementation of <code>ModelModel::set_row_data</code> now forward the call to the inner model

## [0.1.2] - 2021-09-09

### Changed

 - Apply the default text color from the style for the `color` of `Text` and `TextInput` elements, to contrast
   correctly with the application of `Window`'s `background` property.
 - LineEdit scrolls to keep the cursor visible
 - The `clicked` callbacks are now only emitted if the release event is within the TouchArea's geometry
 - parentheses around the condition are no longer needed in `if` elements: `if condition : Foo { ... }`

### Added

 - One can now set an alias from the root to a global callback
 - It is now possible to access properties and callback of exported global objects from the native code (#96)
 - C++ API: `blocking_invoke_from_event_loop`: a blocking version of `invoke_from_event_loop`
 - TextInput can support multiple line by setting single-line to false
 - The CMake integration now allows enabling/disabling SixtyFPS library features, such as Wayland support
   or the dynamic run-time interpreter.
 - Added `image-rendering` property to Image to control how the image is scaled
 - `TextEdit` widget
 - Support for semantic tokens in LSP

### Fixed
 - The interpreter API correctly return an error instead of panicking when setting properties or calling callbacks
   that don't exist
 - The `has-hover` property is correctly reset the false when releasing the mouse outside the touch area

## [0.1.1] - 2021-08-19

### Changed
 - Fixed lookup scope when resolving model expression in `for` or `if` constructs:
   the `self` element now refers to the correct element instead of the root.
 - Rust: default to the native style if Qt is available
 - Rust: deprecated `SharedVector::as_slice_mut()`. Use `SharedVector::make_mut_slice()` instead.
 - The default non-native widget style is now the new "fluent" style.
 - The internal normalization of identifiers is using `-` instead of `_`, this is an internal
   change, but it can be seen in error messages. When listing properties the identifiers are
   preserved. For fields in structures they are normalized.
- Show a compilation error when there are duplicated element ids.
 - The `clip` property can now be any expression.

### Added

 - `ComboBox` now has a `selected` callback.
 - `Window` now has an `icon` property.
 - Added `sixtyfps::Weak::upgrade_in_event_loop` in the Rust API.
 - Added `sixtyfps::Model::as_any()` in the Rust API.
 - Added conversion between `sixtyfps::Image` and `sixtyfps::interpreter::Value` in the C++ API.
 - Properties of type `angle` and `duration` are allowed in the public API.
 - Every element now has a `visible` property.
 - `Slider` now has a `changed` callback.
 - Added `TabWidget` widget.
 - Rust: `sixtyfps::Image` can now be constructed from image data provided by `sixtyfps::SharedPixelBuffer`.
   This enables integrating with other low-level software rendering or the the popular Rust image crate.
 - VSCode extension: added an option to specify command line arguments for the LSP.

### Fixed

 - GridLayout cells with colspan and rowspan respect properly their constraints.
 - Don't panic when replacing programmatically text in a `TextInput` and then editing it.
 - The default height of elements in a ListView no longer defaults to 100%.
 - Fixed support for `*=` and `/=` on types with unit such as length.
 - Don't panic when using a self assignment operator on an invalid type - this produces a compiler error
   instead.
 - Fixed angle conversion for values specified in radians, gradians and turns.
 - Fixed SharedVector sometimes not allocating big enough storage.

## [0.1.0] - 2021-06-30

### Changed

 - Layouts are now conceptually their own elements, meaning that the `x` and `y` properties of items
   within layouts are relative to the layout and no longer to the parent element of layouts.
 - The default spacing and padding of layouts is now 0 instead of being specific to the style.
   There are now `HorizontalBox`, `VerticalBox`, `GridBox` widgets which have default spacing and padding.
 - Setting the window `height` and `width` properties results in a fixed size. The `preferred-width`
   and `preferred-height` property can be used to set the initial size and the window remains resizable
   by the user, if the window manager permits.
 - Binding loops are now detected at compile-time instead of panic at runtime.
 - The `viewer` binary was renamed to `slint-viewer` and is now available via `cargo install` from crates.io.
 - The layout properties `minimum-width`, `maximum-height`, etc. were renamed to a shorter version `min-width`,
   `max-height`. The old names are still supported as a deprecated alias.

### Added

 - Warnings are now shown in the online code editor.
 - `sixtyfps::invoke_from_event_loop` was added to the C++ and Rust APIs, to run a function in the UI thread from any thread.
 - `sixtyfps::run_event_loop()` and `sixtyfps::quit_event_loop()` were added to the Rust and C++ APIs to start and quit the main event loop.
 - `z` property on items.
 - The type in two-way bindings can now be omitted.
 - It's possible to declare aliases for callbacks (`callback clicked <=> other.clicked;`)
 - `abs()` function to get the absolute value
 - The root element of an `if` or `for` can be given a name (`if (condition) name := Rectangle {}`)
 - `sixtyfps::Image` is a new type in the public Rust and C++ API to load images from a path.
 - The width and height of images is now accessible via the `width`or `height` of an `image` type property in .60 files (`some-image.source.width`)

### Fixed

 - Fixed Mouse wheel to work on the `Flickable` element and derivatives.
 - Fixed generated C++ code on Windows.
 - Calling `debug(...)` no longer breaks the LSP.
 - `ComponentDefinition::properties` only exposes public properties as documented.
 - Many more bugfixes

## [0.0.6] - 2021-04-27

### Changed
 - `Rectangle::color` was deprecated and replaced by `Rectangle::background`, same for `Window::color`
 - `Path::fill-color` was renamed to `fill`, and `Path::stroke-color` was renamed to `stroke`, which are now brushes instead of color
 - Many other color property became brush in order to support gradients
 - the `resource` type was renamed to `image`
 - Calling a callback is done from C++/Rust with `invoke_<name>` instead of `call_<name>`

### Added
 - `@linear-gradient` can be used to have gradients on rectangle and paths
 - `Image::colorize` allows to apply a color filter on image
 - `0` can be converted to anything with units
 - Support power of unit in intermediate expression. (eg: `3px * width / height` is now supported but used to be an error)
 - Support for `else if`
 - The path fill rule can now be specified using `Path::fill-rule`.
 - Support for `letter-spacing` in `Text`/`TextInput` elements.
 - `rgb()` / `rgba()`
 - Layout in Flickable
 - LSP server with Live Preview and basic auto completion
 - The viewer tool gained the `--auto-reload` argument
 - `Window.default-font-weight`
 - Added `opacity` property that can be applied to elements
 - Added `clip` property in Rectangle, including clipping for rounded rectangle
 - API to load dynamically .60 files from C++ and Rust, including a way to embed sixtyfps widgets in Qt applications
 - Preferred size in Layouts
 - Math functions such as `sin`, `cos`, `sqrt`, ...
 - New printer demo design
 - Ability to load custom fonts using `import` statements

### Fixed
 - `Image::image-fit`'s `cover` and `contains` variant are fixed to match the CSS spec
 - Flickable without scrollbar
 - Multiplying and dividing different units.
 - Many more bugfixes

## [0.0.5] - 2021-01-29

### Changed
 - Renamed "signal" to "callback"
 - And calling a callback is done from C++/Rust with `call_<name>` instead of `emit_<name>`
 - Renamed "SharedArray" to "SharedVector" in the C++/Rust API
 - Renamed Slider min and max property to minimum and maximum
 - The graphics rendering backend was changed to use femtovg or Qt
 - Renamed `initial-focus` to `forward-focus` as focus related property on elements
 - The "align-" prefix was removed from `TextHorizontalAlignment` and `TextVerticalAlignment`. Either
   change `align-left` to `left` or qualify with `TextHorizontalAlignment.left`.
 - `img!"..."` is replaced by `@image-url("...")`
 - `$children` is replaced by `@children`

### Added
 - `title` property to the Window element
 - `color` property to the Window element
 - `maximum`/`minimum` properties to the `SpinBox`
 - strings can contain escape codes
 - `FocusScope` to handle key events
 - `return` statements
 - `Text` word wrap and elide
 - `drop-shadow-*` properties (limited to `Rectangle` at the moment)
 - `Color.brighter` / `Color.darker`
 - `Window.default-font-family` and `Window.default-font-size`


## [0.0.4] - 2020-12-14

### Changed
 - Changed Rust `build.rs` API to use an opaque compiler configuration type
 - With Rust, image resources are embedded in the binary by default.
 - Updated winit version
 - Updated Neon Version

### Fixed
 - Fixed animations sometimes stopping mid-way.
 - Fixed rendering of C++ components
 - Fixed disabled widgets

## [0.0.3] - 2020-12-09

### Changed
 - In C++, the generated component is now wrapped by a `ComponentHandle` smart pointer that acts like `std::shared_ptr`. New instances
   are created using `T::create()`.
 - In Rust, the generated component implements `Clone` and acts like an `Rc`. `sixtyfps::Weak` can be used to hold weak references.
 - `ARGBColor` was renamed `RgbaColor`
 - `width and `height` of some builtin elements now default to 100% of the parent element.

### Added
 - Allow dashes in identifiers (#52)
 - VerticalLayout / HorizontalLayout
 - Placeholder text in `LineEdit`
 - global components (#96)
 - `Clip` element
 - `ComboBox` element
 - `PopupWindow` element
 - `Image` element: New source-clip-{x, y, width, height} properties
 - `sixtyfps::Timer` in Rust API
 - Transitions are now implemented
 - `round`/`ceil`/`floor`/`mod`/`max`/`min`/`cubic-bezier` functions
 - Signals can have return a value
 - `has_hover` property in `TouchArea`
 - `font-weight` property on Text
 - `viewbox-x/y/width/height` and `clip` properties for `Path`


## [0.0.2] - 2020-10-22

### Changed
 - Default to the native style in the `viewer`, if available.
 - Changed the name of the common logical pixel unit from `lx` to `px`. The less
   often used physical pixel has now the `phx` suffix.

### Added
 - Add support for more keyboard shortcuts to `TextInput`.
 - Added a `current_item` to `StandardListView`.
 - API cleanup in sixtyfps-node

### Fixed
 - Fix occasional hang when navigating in `TextInput` fields with the cursor keys.
 - Fix access to aliased properties from within `for` and `if` expressions.
 - Fix `ScrollView` being scrollable when it shouldn't.
 - Fix appearance of natively styled scrollbars.
 - Allow converting an object type to another even if it is missing some properties.
 - Add missing frame drawing around `ScrollView`.
 - Fix Clipping in scroll views in WASM builds.
 - Fix resizing of `ListView`.
 - Many more bugfixes

## [0.0.1] - 2020-10-13
 - Initial release.

[0.0.1]: https://github.com/slint-ui/slint/releases/tag/v0.0.1
[0.0.2]: https://github.com/slint-ui/slint/releases/tag/v0.0.2
[0.0.3]: https://github.com/slint-ui/slint/releases/tag/v0.0.3
[0.0.4]: https://github.com/slint-ui/slint/releases/tag/v0.0.4
[0.0.5]: https://github.com/slint-ui/slint/releases/tag/v0.0.5
[0.0.6]: https://github.com/slint-ui/slint/releases/tag/v0.0.6
[0.1.0]: https://github.com/slint-ui/slint/releases/tag/v0.1.0
[0.1.1]: https://github.com/slint-ui/slint/releases/tag/v0.1.1
[0.1.2]: https://github.com/slint-ui/slint/releases/tag/v0.1.2
[0.1.3]: https://github.com/slint-ui/slint/releases/tag/v0.1.3
[0.1.4]: https://github.com/slint-ui/slint/releases/tag/v0.1.4
[0.1.5]: https://github.com/slint-ui/slint/releases/tag/v0.1.5
[0.1.6]: https://github.com/slint-ui/slint/releases/tag/v0.1.6
[0.2.0]: https://github.com/slint-ui/slint/releases/tag/v0.2.0
[0.2.1]: https://github.com/slint-ui/slint/releases/tag/v0.2.1
[0.2.2]: https://github.com/slint-ui/slint/releases/tag/v0.2.2
[0.2.3]: https://github.com/slint-ui/slint/releases/tag/v0.2.3
[0.2.4]: https://github.com/slint-ui/slint/releases/tag/v0.2.4
[0.2.5]: https://github.com/slint-ui/slint/releases/tag/v0.2.5
[0.3.0]: https://github.com/slint-ui/slint/releases/tag/v0.3.0
[0.3.1]: https://github.com/slint-ui/slint/releases/tag/v0.3.1
[0.3.2]: https://github.com/slint-ui/slint/releases/tag/v0.3.2
[0.3.3]: https://github.com/slint-ui/slint/releases/tag/v0.3.3
[0.3.4]: https://github.com/slint-ui/slint/releases/tag/v0.3.4
[0.3.5]: https://github.com/slint-ui/slint/releases/tag/v0.3.5
[1.0.0]: https://github.com/slint-ui/slint/releases/tag/v1.0.0
[1.0.1]: https://github.com/slint-ui/slint/releases/tag/v1.0.1
[1.0.2]: https://github.com/slint-ui/slint/releases/tag/v1.0.2
[1.1.0]: https://github.com/slint-ui/slint/releases/tag/v1.1.0
[1.1.1]: https://github.com/slint-ui/slint/releases/tag/v1.1.1
[1.2.0]: https://github.com/slint-ui/slint/releases/tag/v1.2.0
[1.2.1]: https://github.com/slint-ui/slint/releases/tag/v1.2.1
[1.2.2]: https://github.com/slint-ui/slint/releases/tag/v1.2.2
[1.3.0]: https://github.com/slint-ui/slint/releases/tag/v1.3.0
[1.3.1]: https://github.com/slint-ui/slint/releases/tag/v1.3.1
[1.3.2]: https://github.com/slint-ui/slint/releases/tag/v1.3.2
[1.4.0]: https://github.com/slint-ui/slint/releases/tag/v1.4.0
[1.4.1]: https://github.com/slint-ui/slint/releases/tag/v1.4.1
[1.5.0]: https://github.com/slint-ui/slint/releases/tag/v1.5.0
[1.5.1]: https://github.com/slint-ui/slint/releases/tag/v1.5.1
[1.6.0]: https://github.com/slint-ui/slint/releases/tag/v1.6.0
[1.7.0]: https://github.com/slint-ui/slint/releases/tag/v1.7.0
[1.7.1]: https://github.com/slint-ui/slint/releases/tag/v1.7.1
[1.7.2]: https://github.com/slint-ui/slint/releases/tag/v1.7.2
