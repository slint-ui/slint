// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore libtest nextest

use rayon::prelude::*;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn make_generator_file(path: &Path) -> std::io::Result<BufWriter<File>> {
    Ok(BufWriter::new(File::create(path)?))
}

fn validate_test_file(base: &OsStr, path: &Path) -> std::io::Result<()> {
    let template = include_str!("template.rs");
    let expected = template.replace("{FILENAME}", &base.to_string_lossy());

    println!("cargo::rerun-if-env-changed=SLINT_UPDATE_TESTS");
    // If requested, update the file to match the template instead of failing.
    if std::env::var("SLINT_UPDATE_TESTS").is_ok() {
        std::fs::write(path, expected)?;
        println!("cargo:warning=Updated test file: {}", path.display());
        return Ok(());
    }

    assert!(std::fs::exists(path).unwrap_or_default(), "Missing test binary: {}", path.display());
    let file_contents = std::fs::read_to_string(path)?;
    let normalize = |s: &str| s.replace("\r\n", "\n").trim_end().to_string();

    assert_eq!(
        normalize(&file_contents),
        normalize(&expected),
        "Test file '{}' does not match template.\nRun with SLINT_UPDATE_TESTS=1 to update from template.",
        path.display(),
    );
    Ok(())
}

fn make_generator_files() -> std::io::Result<HashMap<OsString, BufWriter<File>>> {
    // Always re-generate all files, to ensure SLINT_TEST_FILTER can actually filter out test cases.
    let mut generated_files = HashMap::new();
    let tests_folder: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests"].iter().collect();

    for file in std::fs::read_dir(tests_folder)? {
        let file = file?.path();
        let base = file.file_stem().expect("Missing file name!");
        validate_test_file(base, &file)?;

        let generated_path =
            PathBuf::from(&std::env::var_os("OUT_DIR").unwrap()).join(file.file_name().unwrap());
        generated_files.insert(base.into(), make_generator_file(&generated_path)?);
    }
    Ok(generated_files)
}

fn generated_file_for_test<'a>(
    testcase: &test_driver_lib::TestCase,
    generated_files: &'a mut HashMap<OsString, BufWriter<File>>,
    fallback: &'a mut BufWriter<File>,
) -> Result<&'a mut BufWriter<File>, std::io::Error> {
    let base: Option<&Path> = testcase.relative_path.iter().next().map(|folder| folder.as_ref());
    let case_root_dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "cases"].iter().collect();

    // For each folder in cases/ generate a separate file.
    // This allows splitting the test cases into multiple test binaries, which allows
    // parallelizing the compilation.
    if base.map(|path| case_root_dir.join(path).is_dir()).unwrap_or_default() {
        let mut base = base.unwrap().to_owned();
        if base.starts_with("widgets") {
            if let Some(style) = testcase.requested_style {
                base = PathBuf::from(format!("{}-{}", base.display(), style));
            }
        }

        let base = base.into_os_string();
        if generated_files.get(&base).is_none() {
            // The generated file hashmap is filled from the list of available test binaries.
            // Panic if we cannot find a generator file to write into, as that means there is no
            // corresponding test binary.
            panic!("Missing test binary for subfolder: {}", base.display());
        }
        Ok(generated_files.get_mut(&base).unwrap())
    } else {
        Ok(fallback)
    }
}

fn main() -> std::io::Result<()> {
    let live_preview = std::env::var("SLINT_LIVE_PREVIEW").is_ok();

    let mut generated_file = make_generator_file(
        &Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"),
    )?;

    let mut generated_files = make_generator_files()?;

    let testcases = test_driver_lib::collect_test_cases("cases")?;

    // Generate the per-case modules on all cores: with the build-time feature,
    // each case runs the Slint compiler (twice with deterministic-output),
    // which dominates the build script's runtime.
    let case_modules = rayon::ThreadPoolBuilder::new()
        .stack_size(512 * 1024)
        .build()
        .expect("failed to create thread pool")
        .install(|| {
            testcases
                .par_iter()
                .map(|testcase| process_case(testcase, live_preview))
                .collect::<std::io::Result<Vec<CaseModule>>>()
        })?;

    // Write the module declarations serially, in collection order, so the
    // including files don't depend on thread scheduling.
    let mut main_thread_tests = Vec::new();
    for (testcase, case_module) in testcases.iter().zip(case_modules) {
        let generated_file =
            generated_file_for_test(testcase, &mut generated_files, &mut generated_file)?;
        writeln!(generated_file, "{}", case_module.module_line)?;
        main_thread_tests.extend(case_module.main_thread_tests);
    }

    if let Some(file) = generated_files.get_mut(OsStr::new("widgets-qt")) {
        write_main_thread_harness(file, &main_thread_tests)?;
    }

    generated_file.flush()?;
    for file in generated_files.values_mut() {
        file.flush()?;
    }

    // By default resources are embedded. The WASM example builds provide test coverage for that. This switch
    // provides test coverage for the non-embedding case, compiling tests without embedding the images.
    if !live_preview {
        println!("cargo:rustc-env=SLINT_EMBED_RESOURCES=false");
    }

    //Make sure to use a consistent style
    println!("cargo:rustc-env=SLINT_STYLE=fluent");
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
    println!("cargo:rustc-env=SLINT_EMIT_DEBUG_INFO=1");
    Ok(())
}

/// One test function of a case that must run on the process main thread,
/// registered as a `libtest_mimic::Trial` by [`write_main_thread_harness`].
struct MainThreadTest {
    /// Module holding the function, without the `r#` prefix.
    module: String,
    function: String,
    ignored: bool,
}

/// One processed case: the module declaration to write into the including file,
/// and the case's main-thread tests.
/// The list is empty for cases run under the regular libtest harness.
struct CaseModule {
    module_line: String,
    main_thread_tests: Vec<MainThreadTest>,
}

/// Writes the `main` of the `widgets-qt` test target.
///
/// The qt style calls QStyle from the thread that runs the test.
/// On macOS the style creates AppKit controls,
/// and AppKit deadlocks on any thread but the process main thread.
/// libtest runs every test on a worker thread,
/// so the target sets `harness = false` and forks one subprocess per test through libtest-mimic.
/// Each subprocess runs its single test on its own main thread,
/// with its own thread-local testing platform.
/// The `test-backends` crate uses the same pattern.
fn write_main_thread_harness(
    output: &mut dyn Write,
    tests: &[MainThreadTest],
) -> std::io::Result<()> {
    writeln!(
        output,
        "
type TestFn = fn() -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>>;

const TESTS: &[(&str, bool, TestFn)] = &["
    )?;
    for test in tests {
        writeln!(
            output,
            "    (\"{module}::{function}\", {ignored}, r#{module}::{function} as TestFn),",
            module = test.module,
            function = test.function,
            ignored = test.ignored,
        )?;
    }
    writeln!(
        output,
        r#"];

fn main() {{
    let args = libtest_mimic::Arguments::from_args();
    // `--exact <name>` marks the subprocess; cargo nextest uses it the same
    // way. Run that one test right here, on this process's main thread.
    // cargo forwards a filter to every test binary, so a name that matches
    // no test here falls through and reports zero tests instead.
    if args.exact && !args.list {{
        if let Some(name) = args.filter.as_deref() {{
            if let Some(test) = TESTS.iter().find(|(n, _, _)| *n == name) {{
                return (test.2)().unwrap();
            }}
        }}
    }}
    let tests = TESTS
        .iter()
        .map(|&(name, ignored, _)| {{
            libtest_mimic::Trial::test(name, move || {{
                let status = ::std::process::Command::new(::std::env::current_exe()?)
                    .args(["--exact", name])
                    .status()?;
                if status.success() {{
                    Ok(())
                }} else {{
                    Err(format!("test failed in subprocess: {{status}}").into())
                }}
            }})
            .with_ignored_flag(ignored)
        }})
        .collect();
    libtest_mimic::run(&args, tests).exit();
}}"#
    )?;
    Ok(())
}

/// Write the `{module_name}.rs` file for the test case into OUT_DIR and return
/// the module declaration that includes it.
fn process_case(
    testcase: &test_driver_lib::TestCase,
    live_preview: bool,
) -> std::io::Result<CaseModule> {
    println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
    let mut module_name = testcase.identifier();
    if module_name.starts_with(|c: char| !c.is_ascii_alphabetic()) {
        module_name.insert(0, '_');
    }
    let module_line = format!("#[path=\"{module_name}.rs\"] pub mod r#{module_name};");
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let ignored = if testcase.is_ignored("rust") {
        "#[ignore = \"testcase ignored for rust\"]"
    } else if (cfg!(not(feature = "build-time")) || live_preview)
        && source.contains("//bundle-translations")
    {
        "#[ignore = \"translation bundle not working with the macro\"]"
    } else if live_preview && testcase.is_ignored("js") {
        "#[ignore = \"Ignored JS testcases ignored in live-preview mode\"]"
    } else if live_preview && testcase.is_ignored("live-preview") {
        "#[ignore = \"testcase ignored in live-preview mode\"]"
    } else if live_preview && source.contains("#3464") {
        "#[ignore = \"issue #3464 not fixed with the interpreter\"]"
    } else if live_preview && module_name.contains("write_to_model") {
        "#[ignore = \"Interpreted model don't forward to underlying models for anonymous structs\"]"
    } else {
        ""
    };

    // The widgets-qt harness calls plain functions instead of `#[test]`
    // functions; see write_main_thread_harness.
    let main_thread = testcase.requested_style == Some("qt");

    let mut output = BufWriter::new(File::create(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join(format!("{module_name}.rs")),
    )?);

    output.write_all(b"#![deny(warnings)]\n#![deny(rust_2018_idioms)]\n#![deny(unsafe_code)]\n")?;

    #[cfg(not(feature = "build-time"))]
    if let Some(placeholder) = generate_macro(&source, &mut output, testcase, main_thread)? {
        output.flush()?;
        let main_thread_tests = if main_thread {
            vec![MainThreadTest {
                module: module_name,
                function: placeholder.to_string(),
                ignored: true,
            }]
        } else {
            Vec::new()
        };
        return Ok(CaseModule { module_line, main_thread_tests });
    }
    #[cfg(feature = "build-time")]
    generate_source(&source, &mut output, testcase)?;

    let mut main_thread_tests = Vec::new();
    for (i, x) in test_driver_lib::extract_test_functions(&source)
        .filter(|x| x.language_id == "rust")
        .enumerate()
    {
        let attributes = if main_thread {
            main_thread_tests.push(MainThreadTest {
                module: module_name.clone(),
                function: format!("t_{i}"),
                ignored: !ignored.is_empty(),
            });
            "pub".to_string()
        } else {
            format!("#[test] {ignored}")
        };
        write!(
            output,
            r"
#[rust_analyzer::skip]
{} fn t_{}() -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {{
    use i_slint_backend_testing as slint_testing;
    slint_testing::init_no_event_loop();
    slint_testing::configure_test_fonts();
    {}
    Ok(())
}}",
            attributes,
            i,
            x.source.replace('\n', "\n    ")
        )?;
    }

    output.flush()?;
    Ok(CaseModule { module_line, main_thread_tests })
}

/// Generates the `slint!` macro invocation for the case,
/// or a placeholder function when the case cannot run as a macro.
/// Returns the placeholder's name, and `None` when it generates the macro.
#[cfg(not(feature = "build-time"))]
fn generate_macro(
    source: &str,
    output: &mut dyn Write,
    testcase: &test_driver_lib::TestCase,
    main_thread: bool,
) -> Result<Option<&'static str>, std::io::Error> {
    let write_placeholder = |output: &mut dyn Write,
                             ignore_reason: &str,
                             name: &str|
     -> std::io::Result<()> {
        if main_thread {
            writeln!(
                    output,
                    "pub fn {name}() -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {{ Ok(()) }}"
                )
        } else {
            writeln!(output, "#[test] #[ignore = \"{ignore_reason}\"] fn {name}() {{}}")
        }
    };
    if source.contains("\\{") {
        // Unfortunately, \{ is not valid in a rust string so it cannot be used in a slint! macro
        let name = "ignored_because_string_template";
        write_placeholder(output, "string template don't work in macros", name)?;
        return Ok(Some(name));
    }
    if testcase.is_ignored("rust-macro") {
        let name = "ignored_for_macro";
        write_placeholder(output, "testcase ignored for the slint! macro", name)?;
        return Ok(Some(name));
    }
    // to silence all the warnings in .slint files that would be turned into errors
    output.write_all(b"#![allow(deprecated)]")?;
    let include_paths = test_driver_lib::extract_include_paths(source);
    let library_paths = test_driver_lib::extract_library_paths(source);
    output.write_all(b"slint::slint!{")?;
    for path in include_paths {
        let mut abs_path = testcase.absolute_path.clone();
        abs_path.pop();
        abs_path.push(path);

        output.write_all(b"#[include_path=r#\"")?;
        output.write_all(abs_path.to_string_lossy().as_bytes())?;
        output.write_all(b"\"#]\n")?;

        println!("cargo:rerun-if-changed={}", abs_path.to_string_lossy());
    }
    for (lib, path) in library_paths {
        let mut abs_path = testcase.absolute_path.clone();
        abs_path.pop();
        abs_path.push(path);

        output.write_all(b"#[library_path(")?;
        output.write_all(lib.as_bytes())?;
        output.write_all(b")=r#\"")?;
        output.write_all(abs_path.to_string_lossy().as_bytes())?;
        output.write_all(b"\"#]\n")?;

        println!("cargo:rerun-if-changed={}", abs_path.to_string_lossy());
    }

    if let Some(style) = testcase.requested_style {
        output.write_all(b"#[style=\"")?;
        output.write_all(style.as_bytes())?;
        output.write_all(b"\"#]\n")?;
    }

    let mut abs_path = testcase.absolute_path.clone();
    abs_path.pop();
    output.write_all(b"#[include_path=r#\"")?;
    output.write_all(abs_path.to_string_lossy().as_bytes())?;
    output.write_all(b"\"#]\n")?;
    output.write_all(source.as_bytes())?;
    output.write_all(b"}\n")?;
    Ok(None)
}

#[cfg(feature = "build-time")]
fn generate_source(
    source: &str,
    output: &mut impl Write,
    testcase: &test_driver_lib::TestCase,
) -> Result<(), std::io::Error> {
    println!("cargo::rerun-if-env-changed=SLINT_LIVE_PREVIEW");

    let generated = compile_and_generate(source, testcase)?;

    #[cfg(feature = "deterministic-output")]
    {
        let second = compile_and_generate(source, testcase)?;
        let expect_utf8 =
            |bytes| std::str::from_utf8(bytes).expect("generated Rust is valid UTF-8");
        assert_eq!(
            expect_utf8(&generated),
            expect_utf8(&second),
            "Non-deterministic compiler output for {:?}",
            testcase.absolute_path
        );
    }

    output.write_all(&generated)?;
    Ok(())
}

#[cfg(feature = "build-time")]
fn compile_and_generate(
    source: &str,
    testcase: &test_driver_lib::TestCase,
) -> Result<Vec<u8>, std::io::Error> {
    use i_slint_compiler::{diagnostics::BuildDiagnostics, *};

    let include_paths = test_driver_lib::extract_include_paths(source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let library_paths = test_driver_lib::extract_library_paths(source)
        .map(|(k, v)| (k.to_string(), std::path::PathBuf::from(v)))
        .collect::<std::collections::HashMap<_, _>>();

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source.to_owned(), Some(&testcase.absolute_path), &mut diag);
    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Rust);
    compiler_config.enable_experimental = true;
    compiler_config.include_paths = include_paths;
    compiler_config.library_paths = library_paths;
    compiler_config.style = Some(testcase.requested_style.unwrap_or("fluent").to_string());
    compiler_config.debug_info = true;
    if source.contains("//bundle-translations") {
        compiler_config.translation_path_bundle =
            Some(testcase.absolute_path.parent().unwrap().to_path_buf());
        compiler_config.translation_domain =
            Some(testcase.absolute_path.file_stem().unwrap().to_str().unwrap().to_string());
    }
    if source.contains("//no-default-translation-context") {
        compiler_config.default_translation_context =
            i_slint_compiler::DefaultTranslationContext::None;
    }
    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_errors() {
        diag.print_warnings_and_exit_on_error();
        return Err(std::io::Error::other(format!("build error in {:?}", testcase.absolute_path)));
    } else {
        diag.print();
    }

    let mut output = Vec::new();
    generator::generate(
        generator::OutputFormat::Rust,
        &mut output,
        None,
        &root_component,
        &loader.compiler_config,
    )?;
    Ok(output)
}
