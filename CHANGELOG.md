# Changelog
All notable changes to this project will be documented in this file.

## Unreleased

### Added

 - C++ interpreter API: added a `Value::Value(int)` constructor
 - Globals Singleton may now refer to other global singletons
 - Added `input-type` property to `TextInput` and `LineEdit` that allows for characters to be replaced in password fields

### Fixed

 - `TouchArea::has-hover` is now properly set to false when the mouse leaves the window.

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
 - Fixed panic when caling `hide()` on a `sixtyfps::Window` from within a callback triggered by keyboard/mouse input
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
[0.2.0]: https://github.com/slint-ui/slint/releases/tag/v0.0.2
