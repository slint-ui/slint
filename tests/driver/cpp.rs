use lazy_static::lazy_static;
use sixtyfps_compilerlib::*;
use std::error::Error;
use std::io::Write;

lazy_static! {
    static ref NATIVE_LIBRARY_DEPENDENCIES: Vec<String> =
        test_driver_lib::native_library_dependencies(
            env!("CARGO"),
            &[],
            "sixtyfps_rendering_backend_gl",
        )
        .unwrap()
        .split(" ")
        .map(String::from)
        .collect();
}

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = testcase.absolute_path.clone();
    let mut tr = typeregister::TypeRegister::builtin();
    let doc = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    run_passes(&doc, &mut diag, &mut tr);

    let (mut diag, source) = diag.check_and_exit_on_error(source.clone());

    let mut generated_cpp: Vec<u8> = Vec::new();

    generator::generate(&mut generated_cpp, &doc.root_component, &mut diag)?;
    diag.check_and_exit_on_error(source);

    write!(
        &mut generated_cpp,
        "int main() {{ static {} component; return 0; }}",
        &doc.root_component.id
    )?;

    //println!("CODE: {}", String::from_utf8(generated_cpp.clone())?);

    let mut cpp_file = tempfile::Builder::new().suffix(".cpp").tempfile()?;

    cpp_file
        .write(&generated_cpp)
        .map_err(|err| format!("Error writing generated code: {}", err))?;
    cpp_file
        .as_file()
        .sync_all()
        .map_err(|err| format!("Error flushing generated code to disk: {}", err))?;

    let compiler = cc::Build::new()
        .cargo_metadata(false)
        .cpp(true)
        .opt_level_str(env!("OPT_LEVEL"))
        .target(env!("TARGET"))
        .host(env!("HOST"))
        .include(env!("GENERATED_CPP_HEADERS_PATH"))
        .include(env!("CPP_API_HEADERS_PATH"))
        .try_get_compiler()?;

    let mut compiler_command = compiler.to_command();

    let binary_path = cpp_file.path().with_extension(std::env::consts::EXE_EXTENSION);

    let keep_temp_files = std::env::var("KEEP_TEMP_FILES").is_ok();

    let _binary_deletion_guard = scopeguard::guard(binary_path.clone(), |path| {
        if !keep_temp_files {
            std::fs::remove_file(path).unwrap_or(());
        }
    });

    compiler_command.arg("-o").arg(binary_path.clone()).arg(cpp_file.path());

    if compiler.is_like_clang() || compiler.is_like_gnu() {
        compiler_command.arg("-std=c++17");
        compiler_command.arg(concat!("-L", env!("CPP_LIB_PATH")));
        compiler_command.arg("-lsixtyfps_rendering_backend_gl");
    }

    compiler_command.args(&*NATIVE_LIBRARY_DEPENDENCIES);

    let _output = compiler_command.output()?;

    std::process::Command::new(binary_path.clone())
        .spawn()
        .map_err(|err| format!("Error launching testcase binary: {}", err))?
        .wait()
        .map_err(|err| format!("Test case could not be run: {}", err))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else if let Some(exit_code) = status.code() {
                Err(format!("Test case exited with non-zero code: {}", exit_code))
            } else {
                Err("Test case exited by signal".into())
            }
        })?;

    if keep_temp_files {
        println!(
            "Left temporary files behind for {} : source {} binary {}",
            testcase.absolute_path.display(),
            cpp_file.path().display(),
            binary_path.display()
        );
        cpp_file.keep()?;
    }

    Ok(())
}
