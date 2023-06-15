// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#[cfg(test)]
mod interpreter;

include!(env!("TEST_FUNCTIONS"));

macro_rules! test_example {
    ($id:ident, $path:literal) => {
        #[test]
        fn $id() {
            let relative_path = std::path::PathBuf::from(concat!("../../../examples/", $path));
            let mut absolute_path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&relative_path);
            if !absolute_path.exists() {
                // Try with .60 instead (for the updater_test)
                let legacy = absolute_path.to_string_lossy().replace(".slint", ".60");
                if std::path::Path::new(&legacy).exists() {
                    absolute_path = legacy.into();
                }
            }
            interpreter::test(&test_driver_lib::TestCase { absolute_path, relative_path }).unwrap();
        }
    };
}

test_example!(example_printerdemo, "printerdemo/ui/printerdemo.slint");
test_example!(example_printerdemo_old, "printerdemo_old/ui/printerdemo.slint");
test_example!(example_memory, "memory/memory.slint");
test_example!(example_slide_puzzle, "slide_puzzle/slide_puzzle.slint");
test_example!(example_todo, "todo/ui/todo.slint");
test_example!(example_gallery, "gallery/gallery.slint");
test_example!(example_fancy_demo, "fancy_demo/main.slint");
test_example!(example_bash_sysinfo, "bash/sysinfo.slint");
test_example!(example_carousel, "carousel/ui/carousel_demo.slint");
test_example!(example_iot_dashboard, "iot-dashboard/main.slint");

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
