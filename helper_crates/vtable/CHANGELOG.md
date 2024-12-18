<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Changelog
All notable changes to this crate will be documented in this file.

## [0.2.1] - 2024-12-18

 - Fixed Warnings

## [0.2.0] - 2024-03-14

 - Make `Dyn` not Send or Sync, thereby fixing a soundness hole

## [0.1.12] - 2024-02-26

 - Fix error reported by miri
 - Fix compiler and clippy warnings

## [0.1.11] - 2023-09-04

 - Use portable_atomic instead of deprecated atomic_polyfill.

## [0.1.10] - 2023-04-03

 - updated syn to syn 2.0

## [0.1.9] - 2022-09-14

 - Added `VRc::map_dyn`, the equivalent of `VRc::map` to create a `VRcMapped`
   when the VRc is already type erased
 - Fixed warnings
 - Update `atomic-polyfill` dependency

## [0.1.8] - 2022-07-05

 - Changed the representation of the different types to use NonNull
 - Added `VRef::as_ptr`

## [0.1.7] - 2022-05-04

 - Implement `Debug` for `VRc`
 - Quieten warning about unused unsafe in the `#[vtable]` generated code

## [0.1.6] - 2022-03-09

 - Add `VWeak::ptr_eq`

## [0.1.5] - 2022-01-21

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
