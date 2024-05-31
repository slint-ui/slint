// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
# Migration from previous versions

## Migration from v0.1.x

* `Value::Array` was removed and [`Value::Model`] needs to be used instead.
* `CallCallbackError` was renamed to [`InvokeCallbackError`].
* `WeakComponentInstance` was removed. Use `slint::Weak<slint::interpreter::ComponentInstance>` instead.
  You might need to `use slint::ComponentHandle;` in your code to bring the trait into scope.

### Crate features

Some crate features have been renamed:

| Old Feature Name                    | New Feature Name                   | Note                                                                          |
| ------------------------------------| ---------------------------------- | ----------------------------------------------------------------------------- |
| `backend-gl` | `backend-gl-all`     | Enable this feature if you want to use the OpenGL ES 2.0 rendering backend with support for all windowing systems. |
| `x11`        | `backend-gl-x11`     | Enable this feature and switch off `backend-gl-all` if you want a smaller build with just X11 support.             |
| `wayland`    | `backend-gl-wayland` | Enable this feature and switch off `backend-gl-all` if you want a smaller build with just wayland support.         |

*/

use crate::*;
