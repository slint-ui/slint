use sixtyfps_compilerlib::*;
use std::error::Error;
use std::io::Write;

pub struct Driver {
    native_library_dependencies: Vec<String>,
}

impl Driver {
    pub fn new() -> Driver {
        Self {
            native_library_dependencies: "-ldl -pthread -lrt -lm -lfreetype -lfontconfig"
                .split(" ")
                .map(String::from)
                .collect(),
        }
    }

    pub fn test(&self, testcase: &super::TestCase) -> Result<(), Box<dyn Error>> {
        let (syntax_node, mut diag) = parser::parse(&testcase.source);
        diag.current_path = testcase.path.clone();
        let mut tr = typeregister::TypeRegister::builtin();
        let doc = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
        run_passes(&doc, &mut diag, &mut tr);

        let (mut diag, source) = diag.check_and_exit_on_error(testcase.source.clone());

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

        // ### How to ensure that the file was written to disk? If we call cpp_file.close() it will also remove the file :(
        cpp_file.write(&generated_cpp)?;

        let compiler = cc::Build::new()
            .cargo_metadata(false)
            .cpp(true)
            .opt_level_str(env!("OPT_LEVEL"))
            .target(env!("TARGET"))
            .host(env!("HOST"))
            .include(env!("GENERATED_CPP_HEADERS_PATH"))
            .include(env!("CPP_API_HEADERS_PATH"))
            .try_get_compiler()?;

        let mut command = compiler.to_command();

        let binary = tempfile::NamedTempFile::new()?;

        command.arg("-o").arg(binary.path()).arg(cpp_file.path());

        if compiler.is_like_clang() || compiler.is_like_gnu() {
            command.arg("-std=c++17");
            command.arg(concat!("-L", env!("CPP_LIB_PATH")));
            command.arg("-lsixtyfps_rendering_backend_gl");
        }

        command.args(&self.native_library_dependencies);

        let _output = command.output()?;

        std::process::Command::new(binary.path())
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

        if std::env::var("KEEP_TEMP_FILES").is_ok() {
            println!(
                "Left temporary files behind for {} : source {} binary {}",
                testcase.path.display(),
                cpp_file.path().display(),
                binary.path().display()
            );
            cpp_file.keep()?;
            binary.keep()?;
        }

        Ok(())
    }
}
