use std::io::Write;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let mut generated_file = std::fs::File::create(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"),
    )?;

    let mut test_dirs = std::collections::HashSet::new();

    for testcase in test_driver_lib::collect_test_cases()? {
        println!("cargo:rerun-if-changed={}", testcase.absolute_path.to_string_lossy());

        test_dirs.insert(testcase.absolute_path.parent().unwrap().to_owned());

        let module_name =
            testcase.relative_path.file_stem().unwrap().to_string_lossy().replace("/", "_");
        write!(generated_file, "#[path=\"{0}.rs\"] mod {0};\n", module_name)?;

        let mut input = std::fs::File::open(&testcase.absolute_path)?;
        let mut output = std::fs::File::create(
            Path::new(&std::env::var_os("OUT_DIR").unwrap()).join(format!("{}.rs", module_name)),
        )?;
        output.write_all(b"sixtyfps::sixtyfps!{\n")?;
        std::io::copy(&mut input, &mut output)?;
        output.write_all(b"}\n#[test] fn test() { /* TODO */ }\n")?
    }

    test_dirs.iter().for_each(|dir| {
        println!("cargo:rerun-if-changed={}", dir.to_string_lossy());
    });

    Ok(())
}
