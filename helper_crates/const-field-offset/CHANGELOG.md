
# Changelog

## [0.2.0]

 - Breaking change: `FIELD_OFFSETS` is now a zero-sized type with a `const fn`
   per field, instead of a struct with one `FieldOffset` field per struct field.
   Use `Foo::FIELD_OFFSETS.bar()` instead of `Foo::FIELD_OFFSETS.bar`. This
   avoids quadratic behavior in the MIR SROA optimization pass on generated
   code with many fields.
 - The derive macro now uses `core::mem::offset_of!` instead of computing the
   `repr(C)` layout manually.
 - The minimum supported Rust version is now 1.85.
 - Removed the unused `field-offset-trait` feature.
 - Upgraded to edition 2024.

## [0.1.5] - 2024-03-14

 - Warning fixes

## [0.1.4] - 2024-02-20

 - Warning fixes

## [0.1.3] - 2023-04-03

 - Upgraded syn to syn 2

## [0.1.2] - 2021-11-24

### Changed
 - Fixed `FieldOffsets` derive macro on non-pub structs when one of its pub field expose a private type
 - Added intra docs link in the generated documentation


## [0.1.1] - 2021-08-16

### Changed
 - Fixed a bunch of clippy warnings


## [0.1.0] - 2020-08-26 (1138c9dbedd13ba110e0953b0f501beb57a18309)
 - Initial release.
