// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_compiler::{diagnostics::BuildDiagnostics, *};
use std::error::Error;
use std::io::Write;
use std::ops::Deref;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let include_paths = test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source.clone(), Some(&testcase.absolute_path), &mut diag);
    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Cpp);
    compiler_config.include_paths = include_paths;
    let (root_component, diag) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_error() {
        let vec = diag.to_string_vec();
        return Err(vec.join("\n").into());
    }

    let mut generated_cpp: Vec<u8> = Vec::new();

    generator::generate(generator::OutputFormat::Cpp, &mut generated_cpp, &root_component)?;

    if diag.has_error() {
        let vec = diag.to_string_vec();
        return Err(vec.join("\n").into());
    }

    generated_cpp.write_all(
        br"
#ifdef NDEBUG
#undef NDEBUG
#endif
#include <assert.h>
#include <cmath>
#include <iostream>
#include <slint_testing.h>
namespace slint_testing = slint::testing;
",
    )?;
    generated_cpp.write_all(b"int main() {\n    slint::testing::init();\n")?;
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

    let cpp_file = cpp_file.into_temp_path();

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

    let binary_path = cpp_file.with_extension(std::env::consts::EXE_EXTENSION);

    let keep_temp_files = std::env::var("KEEP_TEMP_FILES").is_ok();

    let binary_path = scopeguard::guard(binary_path, |path| {
        if !keep_temp_files {
            std::fs::remove_file(path).unwrap_or(());
        }
    });

    compiler_command.arg(&*cpp_file);

    if keep_temp_files {
        println!(
            "Leaving temporary files behind for {} : source {} binary {}",
            testcase.absolute_path.display(),
            cpp_file.display(),
            binary_path.display()
        );
        cpp_file.keep()?;
    }

    if compiler.is_like_clang() || compiler.is_like_gnu() {
        compiler_command.arg("-std=c++20");
        compiler_command.arg("-g");
        compiler_command.arg("-Werror").arg("-Wall").arg("-Wextra");
        compiler_command.arg(concat!("-L", env!("CPP_LIB_PATH")));
        compiler_command.arg("-lslint_cpp");
        compiler_command.arg("-o").arg(&*binary_path);
    } else if compiler.is_like_msvc() {
        compiler_command.arg("/std:c++20");
        compiler_command.arg("/link").arg(concat!(env!("CPP_LIB_PATH"), "\\slint_cpp.dll.lib"));
        let mut out_arg = std::ffi::OsString::from("/OUT:");
        out_arg.push(&*binary_path);
        compiler_command.arg(out_arg);
    }

    let output = compiler_command.output()?;
    print!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
    if !output.status.success() {
        print!("{}", String::from_utf8_lossy(output.stdout.as_ref()));
        return Err("C++ Compilation error (see stdout)".to_owned().into());
    }

    let mut cmd;
    if std::env::var("USE_VALGRIND").is_ok() {
        cmd = std::process::Command::new("valgrind");
        cmd.arg(binary_path.deref());
    } else {
        cmd = std::process::Command::new(binary_path.deref());
    }

    let output = cmd
        .envs(library_search_path_env_with(env!("CPP_LIB_PATH")))
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

    Ok(())
}

fn library_search_path_env_with(
    value_to_prepend: &str,
) -> impl IntoIterator<Item = (&'static str, String)> {
    let (var, separator) = if cfg!(target_os = "windows") {
        ("PATH", ';')
    } else if cfg!(target_os = "macos") {
        ("DYLD_FALLBACK_LIBRARY_PATH", ':')
    } else {
        ("LD_LIBRARY_PATH", ':')
    };

    std::iter::once((
        var,
        format!("{}{}{}", value_to_prepend, separator, std::env::var(var).unwrap_or_default()),
    ))
}
