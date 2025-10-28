// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    #[cfg(not(target_os = "android"))]
    servo_example_lib::main();

    #[cfg(target_os = "android")]
    servo_example_lib::android_main();
}
