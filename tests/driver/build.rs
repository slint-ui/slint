use std::path::PathBuf;

fn main() {
    // Variables that cc.rs needs.
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rustc-env=HOST={}", std::env::var("HOST").unwrap());
    println!("cargo:rustc-env=OPT_LEVEL={}", std::env::var("OPT_LEVEL").unwrap());

    // Variables that we need.
    println!("cargo:rustc-env=CARGO={}", std::env::var("CARGO").unwrap());

    let mut generated_include_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    generated_include_dir.pop();
    generated_include_dir.pop();
    generated_include_dir.pop();

    let lib_dir = generated_include_dir.clone();
    println!("cargo:rustc-env=CPP_LIB_PATH={}", lib_dir.display());

    generated_include_dir.push("include");
    println!("cargo:rustc-env=GENERATED_CPP_HEADERS_PATH={}", generated_include_dir.display());

    let mut api_includes = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    api_includes.pop();
    api_includes.pop();
    api_includes = api_includes.join("api/sixtyfps-cpp/include");

    println!("cargo:rustc-env=CPP_API_HEADERS_PATH={}", api_includes.display());
}
