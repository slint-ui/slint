// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::context::SlintContext;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

#[test]
fn mocked_time_fires_the_timers_of_a_sibling_context() {
    i_slint_backend_testing::init_no_event_loop();
    let sibling = SlintContext::current().unwrap().new_sharing_platform();

    let fired = Rc::new(Cell::new(false));
    let fired_ = fired.clone();
    sibling.single_shot(Duration::from_millis(10), move || fired_.set(true));
    #[cfg(not(feature = "internal"))]
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    #[cfg(feature = "internal")]
    i_slint_backend_testing::mock_elapsed_time(20);
    assert!(fired.get());
}
