// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::PlatformError;
use slint::platform::{Platform, SetPlatformError, WindowAdapter};
use std::rc::Rc;

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Err(PlatformError::NoPlatform)
    }
}

#[test]
fn select_reports_already_set_before_resolving_the_backend() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let result = slint::BackendSelector::new().backend_name("unavailable".into()).select();
    assert!(matches!(result, Err(PlatformError::SetPlatformError(SetPlatformError::AlreadySet))));
}
