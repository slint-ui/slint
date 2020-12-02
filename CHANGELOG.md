# Changelog
All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed
 - The Component is now handled in a `ComponentHandle` struct in both rust and C++
 - ARGBColor renamed RgbaColor
 - width and hight of some builtin elements now defaults to 100%

### Added
 - Allow dashes in identifiers (#52)
 - VerticalLayout / HorizontalLayout
 - Placeholder text in line edit
 - global components (#96)
 - `Crop` element
 - `ComboBox` element
 - `PopupWindow` element
 - `Image` element: New source-clip-{x, y, width, height} properties
 - `Timer` in rust API
 - Transitions are now implemented
 - round/ceil/floor/mod/max/min/cubic-bezier functions
 - Signal can have return value
 - `has_hover` property in `TouchArea`
 - `font-weight` property on Text



## [0.0.2] - 2020-12-22

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

