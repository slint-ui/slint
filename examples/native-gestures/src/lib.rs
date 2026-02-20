// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

pub fn app_main() -> Result<(), slint::PlatformError> {
    MainWindow::new()?.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), slint::PlatformError> {
    slint::android::init(app).unwrap();
    app_main()
}
