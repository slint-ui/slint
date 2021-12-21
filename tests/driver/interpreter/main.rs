// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#[cfg(test)]
mod interpreter;

include!(env!("TEST_FUNCTIONS"));

macro_rules! test_example {
    ($id:ident, $path:literal) => {
        #[test]
        fn $id() {
            let relative_path = std::path::PathBuf::from(concat!("../../../examples/", $path));
            interpreter::test(&test_driver_lib::TestCase {
                absolute_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(&relative_path),
                relative_path,
            })
            .unwrap();
        }
    };
}

test_example!(example_printerdemo, "printerdemo/ui/printerdemo.60");
test_example!(example_printerdemo_old, "printerdemo_old/ui/printerdemo.60");
test_example!(example_memory, "memory/memory.60");
test_example!(example_slide_puzzle, "slide_puzzle/slide_puzzle.60");
test_example!(example_todo, "todo/ui/todo.60");
test_example!(example_gallery, "gallery/gallery.60");

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
