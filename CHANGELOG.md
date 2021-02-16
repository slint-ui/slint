# Changelog
All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
 - `Rectangle::color` was deprecated and replaced by `Rectangle::background`, same for `Window::color`
 - `Path::fill-color` was renamed to `fill`, and `Path::stroke-color` was renamed to `stroke`, which are now brushes instead of color
 - Many other color property became brush in order to support gradients

### Added
 - `@linear-gradient` can be used to have gradients on rectangle and paths
 - `Image::colorize` allows to apply a color filter on image
 - `0` can be converted to anything with units
 - Support power of unit in intermediate expression. (eg: `3px * width / height` is now supported but used to be an error)
 - Support for `else if`
 - The path fill rule can now be specified using `Path::fill-rule`.
 - Support for `letter-spacing` in `Text`/`TextInput` elements.

### Fixed
 - `Image::image-fit`'s `cover` and `contains` varient are fixed to match the CSS spec

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
 - Fix appearance of natively styled scrollars.
 - Allow converting an object type to another even if it is missing some properties.
 - Add missing frame drawing around `ScrollView`.
 - Fix Clipping in scroll views in WASM builds.
 - Fix resizing of `ListView`.
 - Many more bugfixes

## [0.0.1] - 2020-10-13
 - Initial release.

[0.0.1]: https://github.com/sixtyfpsui/sixtyfps/releases/tag/v0.0.1
[0.0.2]: https://github.com/sixtyfpsui/sixtyfps/releases/tag/v0.0.2
[0.0.3]: https://github.com/sixtyfpsui/sixtyfps/releases/tag/v0.0.3
[0.0.4]: https://github.com/sixtyfpsui/sixtyfps/releases/tag/v0.0.4
[0.0.5]: https://github.com/sixtyfpsui/sixtyfps/releases/tag/v0.0.5
