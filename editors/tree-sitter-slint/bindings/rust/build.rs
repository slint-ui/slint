fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let parser_path = src_dir.join("parser.c");
    c_config.file(&parser_path);
    println!("cargo:rerun-if-changed={}", parser_path.display());

    let scanner_path = src_dir.join("scanner.c");
    c_config.file(&scanner_path);
    println!("cargo:rerun-if-changed={}", scanner_path.display());

    for header in ["tree_sitter/alloc.h", "tree_sitter/array.h", "tree_sitter/parser.h"] {
        println!("cargo:rerun-if-changed={}", src_dir.join(header).display());
    }

    c_config.compile("tree-sitter-slint");
}
