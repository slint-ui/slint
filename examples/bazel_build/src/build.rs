use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let mut library_paths = HashMap::new();

    slint_build::compile_with_config(
        std::env::var_os("SLINT_INDEX_FILE_PATH").unwrap(),
        slint_build::CompilerConfiguration::new().with_library_paths(library_paths),
    )
    .unwrap();
}
