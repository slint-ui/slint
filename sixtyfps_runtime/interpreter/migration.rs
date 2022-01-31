// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)
/*!
# Migration from previous versions

## Migration from v0.1.x

* `Value::Array` was removed and [`Value::Model`] needs to be used instead.
* `CallCallbackError` was renamed to [`InvokeCallbackError`].
* `WeakComponentInstance` was removed. Use `sixtyfps::Weak<sixtyfps::interpreter::ComponentInstance>` instead.
  You might need to `use sixtyfps::ComponentHandle;` in your code to bring the trait into scope.

*/

use crate::*;
