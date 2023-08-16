<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
# Slide Puzzle

Example based on the flutter slide_puzzle example:
https://flutter.github.io/samples/slide_puzzle

This will allow to compare Slint and Flutter.

Remaining feature to implement to have parity:

* "Spring" animation instead of a bezier curve.
* Hover/Pressed effect on the auto-play checkbox.
* When clicking on the auto-play checkbox, the gray hover
  circle bounces in the direction of the mouse cursor
* The different styles are well separated in different files.
* Shadow on the tiles
* Some layout adjustment
* startup animation

## Comparison

Comparison with the flutter demo (as of commit ecd7f7d
 of this repository, and commit a23d035 of the flutter repository)

| . | Slint | Flutter |
| --- | ---| --- |
| UI files | slide_puzzle.slint | src/puzzle_controls.dart src/puzzle_flow_delegate.dart src/puzzle_home_state.dart src/shared_theme.dart src/theme_plaster.dart src/themes.dart src/theme_seattle.dart src/theme_simple.dart src/widgets/decoration_image_plus.dart src/widgets/material_interior_alt.dart |
| Line of codes for the UI | 444 | 1140 |
| Lines of code for the UI without empty lines and comments | 386 | 831 |
| Logic files | main.rs | main.dart src/flutter.dart src/app_state.dart src/core/body.dart src/core/point_int.dart src/core/puzzle_animator.dart src/core/puzzle.dart src/core/puzzle_proxy.dart src/core/puzzle_simple.dart src/core/puzzle_smart.dart src/core/util.dart |
| Lines of code of logic | 238 | 962 |
| Lines of code of logic without empty lines and comments | 197 | 702 |
| RAM use | TBD | TBD |
| binary size | TBD | TBD |
