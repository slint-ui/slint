# Changelog
All notable changes to this crate will be documented in this file.

## [0.1.5]

 - Make it `#[no_std]`
 - Use `atomic-polyfill` to support compiling to architectures where a polyfill
   using critical sections is needed.
 - Implement `Default` for `VWeakMapped`

## [0.1.4] - 2021-11-24

 - Added `VrcMapped` and `VWeakMapped` to allow for references to objects that are reachable via VRc
 - Used intra-doc link in the generated documentation

## [0.1.3] - 2021-08-16

 - Fixed clippy warnings

## [0.1.2] - 2021-06-28

 - `VRc` and `VWeak` now use atomic counters and implement `Sync` and `Send` if the hold type allows it

## [0.1.1] - 2020-12-09

### Changed
 - `VTableMetaDrop` was made unsafe as it should only be implemented by the macro

### Added
 - VRc

## [0.1.0] - 2020-08-26 (58cdaeb8ddd79a7e00108a93028d856deaa0496c)
 - Initial release.
