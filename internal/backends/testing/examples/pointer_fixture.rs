// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::ComponentHandle;

slint::slint! {
    export { PointerFixture } from "../../../tools/slint-test/tests/pointer_fixture.slint";
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            threading: true,
            renderer_name: Some("skia".into()),
            ..Default::default()
        },
    )))?;
    i_slint_backend_testing::systest::init()?;
    PointerFixture::new()?.run()?;
    Ok(())
}
