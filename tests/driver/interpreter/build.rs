// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");

    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    // Example .slint files that are also run through the interpreter. Unlike the
    // test cases collected below, these live outside tests/cases, so we list them
    // here and honor SLINT_TEST_FILTER as a substring of their repo-root-relative
    // path (e.g. SLINT_TEST_FILTER=examples runs the examples/ entries).
    const EXAMPLES: &[(&str, &str)] = &[
        ("example_printerdemo", "demos/printerdemo/ui/printerdemo.slint"),
        ("example_usecases", "demos/usecases/ui/app.slint"),
        ("example_memory", "examples/memory/memory.slint"),
        ("example_slide_puzzle", "examples/slide_puzzle/slide_puzzle.slint"),
        ("example_todo", "examples/todo/ui/todo.slint"),
        ("example_gallery", "examples/gallery/gallery.slint"),
        ("example_fancy_demo", "examples/fancy_demo/main.slint"),
        ("example_bash_sysinfo", "examples/bash/sysinfo.slint"),
        ("example_carousel", "examples/carousel/ui/carousel_demo.slint"),
        ("example_iot_dashboard", "examples/iot-dashboard/main.slint"),
        ("example_dial", "examples/dial/dial.slint"),
        ("example_sprite_sheet", "examples/sprite-sheet/demo.slint"),
        ("example_fancy_switches", "examples/fancy-switches/demo.slint"),
        ("example_home_automation", "demos/home-automation/ui/demo.slint"),
        ("example_energy_monitor", "demos/energy-monitor/ui/desktop_window.slint"),
        ("example_weather", "demos/weather-demo/ui/main.slint"),
        ("example_grid_model_rows", "examples/layouts/grid-with-model-in-rows.slint"),
        ("example_grid_with_repeated_rows", "examples/layouts/grid-with-repeated-rows.slint"),
        ("example_vector_as_grid", "examples/layouts/vector-as-grid.slint"),
        ("example_vlayout", "examples/layouts/vertical-layout-with-model.slint"),
        ("example_flexbox_interactive", "examples/layouts/flexbox-interactive.slint"),
    ];

    let filter = std::env::var("SLINT_TEST_FILTER").ok();
    for (test_function_name, path) in EXAMPLES {
        if let Some(filter) = &filter
            && !path.contains(filter.as_str())
        {
            continue;
        }
        write!(
            tests_file,
            r##"
            #[test]
            fn {function_name}() {{
                run_example(r#"{path}"#);
            }}
        "##,
            function_name = test_function_name,
            path = path,
        )?;
    }

    for testcase in test_driver_lib::collect_test_cases("cases")?.into_iter() {
        let test_function_name = testcase.identifier();
        let ignored = testcase.is_ignored("interpreter");

        write!(
            tests_file,
            r##"
            #[test]
            {ignore}
            fn test_interpreter_{function_name}() {{
                interpreter::test(&test_driver_lib::TestCase{{
                    absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
                    relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
                    requested_style: {requested_style},
                }}).unwrap();
            }}
        "##,
            ignore = if ignored { "#[ignore]" } else { "" },
            function_name = test_function_name,
            absolute_path = testcase.absolute_path.to_string_lossy(),
            relative_path = testcase.relative_path.to_string_lossy(),
            requested_style =
                testcase.requested_style.map_or("None".into(), |style| format!("Some({style:?})")),
        )?;
    }

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    Ok(())
}
