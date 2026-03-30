// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(test)]
mod interpreter;

include!(env!("TEST_FUNCTIONS"));

macro_rules! test_example {
    ($id:ident, $path:literal) => {
        #[test]
        fn $id() {
            let relative_path = std::path::PathBuf::from(concat!("../../../", $path));
            let mut absolute_path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&relative_path);
            if !absolute_path.exists() {
                // Try with .60 instead (for the updater_test)
                let legacy = absolute_path.to_string_lossy().replace(".slint", ".60");
                if std::path::Path::new(&legacy).exists() {
                    absolute_path = legacy.into();
                }
            }
            interpreter::test(&test_driver_lib::TestCase {
                absolute_path,
                relative_path,
                requested_style: None,
            })
            .unwrap();
        }
    };
}

test_example!(example_printerdemo, "demos/printerdemo/ui/printerdemo.slint");
test_example!(example_usecases, "demos/usecases/ui/app.slint");
test_example!(example_memory, "examples/memory/memory.slint");
test_example!(example_slide_puzzle, "examples/slide_puzzle/slide_puzzle.slint");
test_example!(example_todo, "examples/todo/ui/todo.slint");
test_example!(example_gallery, "examples/gallery/gallery.slint");
test_example!(example_fancy_demo, "examples/fancy_demo/main.slint");
test_example!(example_bash_sysinfo, "examples/bash/sysinfo.slint");
test_example!(example_carousel, "examples/carousel/ui/carousel_demo.slint");
test_example!(example_iot_dashboard, "examples/iot-dashboard/main.slint");
test_example!(example_dial, "examples/dial/dial.slint");
test_example!(example_sprite_sheet, "examples/sprite-sheet/demo.slint");
test_example!(example_fancy_switches, "examples/fancy-switches/demo.slint");
test_example!(example_home_automation, "demos/home-automation/ui/demo.slint");
test_example!(example_energy_monitor, "demos/energy-monitor/ui/desktop_window.slint");
test_example!(example_weather, "demos/weather-demo/ui/main.slint");
test_example!(example_grid_model_rows, "examples/layouts/grid-with-model-in-rows.slint");
test_example!(example_grid_with_repeated_rows, "examples/layouts/grid-with-repeated-rows.slint");
test_example!(example_vector_as_grid, "examples/layouts/vector-as-grid.slint");
test_example!(example_vlayout, "examples/layouts/vertical-layout-with-model.slint");
test_example!(example_flexbox_interactive, "examples/layouts/flexbox-interactive.slint");

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
