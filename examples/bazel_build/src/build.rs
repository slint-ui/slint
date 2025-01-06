use std::path::PathBuf;
use std::collections::HashMap;
use widget_library_utils::WIDGET_LIBRARY_NAME;

fn main() {
    let mut library_paths = HashMap::new();
    let ui_lib_path = PathBuf::from(std::env::var_os("SLINT_WIDGET_LIBRARY_INDEX_PATH").unwrap());
    library_paths.insert(WIDGET_LIBRARY_NAME.to_string(), ui_lib_path);

    slint_build::compile_with_config(
        std::env::var_os("SLINT_INDEX_FILE_PATH").unwrap(),
        slint_build::CompilerConfiguration::new().with_library_paths(library_paths),
    )
    .unwrap();
}
