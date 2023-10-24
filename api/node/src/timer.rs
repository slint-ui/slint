// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use napi::{Env, JsFunction, Result};

use crate::RefCountedReference;

/// Starts the timer with the duration, in order for the callback to called when the timer fires. It is fired only once and then deleted.
#[napi]
pub fn singleshot_timer(env: Env, duration_in_msecs: f64, handler: JsFunction) -> Result<()> {
    if duration_in_msecs < 0. {
        return Err(napi::Error::from_reason("Duration cannot be negative"));
    }
    let duration_in_msecs = duration_in_msecs as u64;

    let handler_ref = RefCountedReference::new(&env, handler)?;

    i_slint_core::timers::Timer::single_shot(
        std::time::Duration::from_millis(duration_in_msecs),
        move || {
            let callback: JsFunction = handler_ref.get().unwrap();
            callback.call_without_args(None).unwrap();
        },
    );

    Ok(())
}
