use sixtyfps_compilerlib::*;
use std::error::Error;
use std::io::Write;
use std::ops::Deref;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = testcase.absolute_path.clone();
    let mut tr = typeregister::TypeRegister::builtin();
    let doc = object_tree::Document::from_node(syntax_node.into(), &mut diag, &mut tr);
    let compiler_config = CompilerConfiguration::default();
    run_passes(&doc, &mut diag, &mut tr, &compiler_config);

    if diag.has_error() {
        let vec = diag.inner.iter().map(|d| d.message.clone()).collect::<Vec<String>>();
        return Err(vec.join("\n").into());
    }

    let mut generated_cpp: Vec<u8> = Vec::new();

    generator::generate(&mut generated_cpp, &doc.root_component, &mut diag)?;

    if diag.has_error() {
        let vec = diag.inner.iter().map(|d| d.message.clone()).collect::<Vec<String>>();
        return Err(vec.join("\n").into());
    }

    generated_cpp.write_all(b"#ifdef NDEBUG\n#undef NDEBUG\n#endif\n#include <assert.h>\n")?;
    generated_cpp.write_all(b"int main() {\n")?;
    for x in test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "cpp") {
        write!(generated_cpp, "  {{\n    {}\n  }}\n", x.source.replace("\n", "\n    "))?;
    }
    generated_cpp.write_all(b"}\n")?;

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

    let binary_path = scopeguard::guard(binary_path, |path| {
        if !keep_temp_files {
            std::fs::remove_file(path).unwrap_or(());
        }
    });

    compiler_command.arg("-o").arg(binary_path.clone()).arg(cpp_file.path());

    if compiler.is_like_clang() || compiler.is_like_gnu() {
        compiler_command.arg("-std=c++17");
        compiler_command.arg(concat!("-L", env!("CPP_LIB_PATH")));
        compiler_command.arg("-lsixtyfps_rendering_backend_gl");
        compiler_command.arg("-lsixtyfps_corelib");
    }

    let output = compiler_command.output()?;
    print!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
    if !output.status.success() {
        return Err("C++ Compilation error (see stdout)".to_owned().into());
    }

    let output = std::process::Command::new(binary_path.deref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| format!("Error launching testcase binary: {}", err))?
        .wait_with_output()
        .map_err(|err| format!("Test case could not be run: {}", err))?;

    if !output.status.success() {
        print!("{}", String::from_utf8_lossy(output.stdout.as_ref()));
        print!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
        if let Some(exit_code) = output.status.code() {
            return Err(format!("Test case exited with non-zero code: {}", exit_code).into());
        } else {
            return Err("Test case exited by signal".into());
        }
    }

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
