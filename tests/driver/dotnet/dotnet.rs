// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Generates C# for every test case, builds all of it into one runner, and runs each case in its own process.
//! A `dotnet build` per case would take seconds each.
//! Each case runs from the unit that the runner embeds, like an app built with `SlintPrecompile`.
//! The generated code would compile the case at run time without it, from a project directory that doesn't exist.
//! The `csharp` blocks of a case run after the check of its `test` properties.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::{self, csharp};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::*;

const STYLE: &str = "fluent";

struct Batch {
    /// The cases that failed before running, with the reason.
    failures: HashMap<String, String>,
    runner: PathBuf,
}

static BATCH: LazyLock<Batch> = LazyLock::new(build_batch);

pub fn test(identifier: &str) -> Result<(), Box<dyn Error>> {
    if let Some(failure) = BATCH.failures.get(identifier) {
        return Err(failure.clone().into());
    }
    let output = std::process::Command::new("dotnet")
        .arg(&BATCH.runner)
        .arg(identifier)
        .env("SLINT_BACKEND", "testing")
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{identifier} failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

/// The components of a generated case, and whether each has a `bool` `test` property.
struct Case {
    components: Vec<(String, bool)>,
    has_blocks: bool,
    /// The name of the resource that holds the unit.
    unit_resource: String,
}

/// Not canonicalized: on Windows that gives a `\\?\` path, which MSBuild can't import from.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf()
}

/// Returns the generated code, the `csharp` blocks as a class, if there are any, and the unit.
fn generate(
    identifier: &str,
    path: &Path,
    project: &Path,
) -> Result<(String, Option<String>, Vec<u8>, Case), String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let format = generator::OutputFormat::CSharp(csharp::Config {
        namespace: Some(format!("Case_{identifier}")),
        runtime_path: Some(project.to_string_lossy().into_owned()),
        live_preview: false,
        unit: true,
    });
    let mut config = CompilerConfiguration::new(format.clone());
    config.style = Some(STYLE.into());
    let edition = test_driver_lib::edition_or_current(&source);
    config.edition = Some(edition.parse().map_err(|e| format!("invalid edition: {e:?}"))?);
    // Relative to the directory of this crate, which cargo test runs in, like the other drivers.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    config.include_paths =
        test_driver_lib::extract_include_paths(&source).map(|p| crate_dir.join(p)).collect();
    config.library_paths = test_driver_lib::extract_library_paths(&source)
        .map(|(name, path)| (name.to_string(), crate_dir.join(path)))
        .collect();
    config.input_project_directory = Some(project.to_path_buf());
    if source.contains("//bundle-translations") {
        config.bundled_translations_path = path.parent().map(Path::to_path_buf);
        config.translation_domain =
            path.file_stem().map(|stem| stem.to_string_lossy().into_owned());
    }
    if source.contains("//no-default-translation-context") {
        config.default_translation_context = i_slint_compiler::DefaultTranslationContext::None;
    }
    let unit_config = config.clone();
    let (doc, diag, loader) = spin_on::spin_on(compile_source(
        &source_path::SourcePath::new(path),
        source.clone(),
        BuildDiagnostics::default(),
        config,
    ));
    if diag.has_errors() {
        return Err(diag.to_string_vec().join("\n"));
    }
    let (unit_diag, unit) = spin_on::spin_on(csharp::unit::compile(
        &source_path::SourcePath::new(path),
        source.clone(),
        unit_config,
    ));
    let unit = unit.ok_or_else(|| {
        format!("compiling the unit failed:\n{}", unit_diag.to_string_vec().join("\n"))
    })?;

    let mut code = Vec::new();
    generator::generate(format, &mut code, None, &doc, &loader.compiler_config)
        .map_err(|e| e.to_string())?;

    // The same name that `lower_to_item_tree` gives each public component.
    let components = doc
        .exported_roots()
        .map(|c| {
            let has_test = c
                .root_element
                .borrow()
                .property_declarations
                .get("test")
                .is_some_and(|p| p.property_type == Type::Bool);
            (csharp::ident(&doc.export_names(&c)[0]), has_test)
        })
        .collect();

    let blocks = test_driver_lib::extract_test_functions(&source)
        .filter(|f| f.language_id == "csharp")
        .map(|f| {
            format!(
                "        {{\n            {}\n        }}\n",
                f.source.replace('\n', "\n            ")
            )
        })
        .collect::<String>();
    let blocks = (!blocks.is_empty()).then(|| {
        format!(
            "namespace Case_{identifier};\n\ninternal static class TestBlocks\n{{\n    public static void Run()\n    {{\n{blocks}    }}\n}}\n"
        )
    });
    let has_blocks = blocks.is_some();
    let unit_resource = format!("slint-unit:{}", project.to_string_lossy());
    Ok((
        String::from_utf8(code).unwrap(),
        blocks,
        unit,
        Case { components, has_blocks, unit_resource },
    ))
}

fn write_program(dir: &Path, cases: &BTreeMap<String, Case>) {
    let mut program = String::from(
        "internal static class Program\n{\n    private static int Main(string[] args)\n    {\n        global::Slint.Testing.Init();\n        switch (args[0])\n        {\n",
    );
    for (identifier, case) in cases {
        let _ = writeln!(program, "            case \"{identifier}\":");
        for (class, has_test) in &case.components {
            let test = if class == "Test" { "Test_" } else { "Test" };
            let check = if *has_test {
                format!(
                    " if (!c.{test}) {{ System.Console.Error.WriteLine(\"{class}: test is false\"); return 1; }}"
                )
            } else {
                String::new()
            };
            let _ = writeln!(
                program,
                "                using (var c = global::Case_{identifier}.{class}.Create()) {{{check} }}"
            );
        }
        if case.has_blocks {
            let _ =
                writeln!(program, "                global::Case_{identifier}.TestBlocks.Run();");
        }
        let _ = writeln!(program, "                return 0;");
    }
    program.push_str("            default:\n                return 2;\n        }\n    }\n}\n");
    std::fs::write(dir.join("Program.cs"), program).unwrap();
}

fn check_output(output: std::process::Output, what: &str) {
    if !output.status.success() {
        panic!(
            "{what} failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn build_batch() -> Batch {
    let repository = repository();
    check_output(
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "slint-dotnet", "--features", "testing", "--manifest-path"])
            .arg(repository.join("Cargo.toml"))
            .output()
            .expect("cargo must be installed"),
        "building slint-dotnet",
    );

    let dir = PathBuf::from(env!("RUNNER_DIR"));
    let cases_dir = dir.join("cases");
    let _ = std::fs::remove_dir_all(&cases_dir);
    std::fs::create_dir_all(&cases_dir).unwrap();
    let projects_dir = dir.join("projects");
    let _ = std::fs::remove_dir_all(&projects_dir);

    let mut failures = HashMap::new();
    let mut cases = BTreeMap::new();
    let mut units = String::from("<Project>\n  <ItemGroup>\n");
    let list = std::fs::read_to_string(env!("TEST_CASES")).unwrap();
    let lines: Vec<(&str, &str)> =
        list.lines().map(|line| line.split_once('\t').unwrap()).collect();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let generated: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = lines
            .chunks(lines.len().div_ceil(threads).max(1))
            .map(|chunk| {
                let projects_dir = &projects_dir;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&(identifier, path)| {
                            // Windows drops a trailing space from a directory name, as in `popupwindow_size_fixed_layout .slint`.
                            let project = projects_dir.join(identifier.trim_end());
                            (identifier, generate(identifier, Path::new(path), &project))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers.into_iter().flat_map(|worker| worker.join().unwrap()).collect()
    });
    for (identifier, result) in generated {
        match result {
            Ok((code, blocks, unit, case)) => {
                std::fs::write(cases_dir.join(format!("{identifier}.g.cs")), code).unwrap();
                let unit_file = cases_dir.join(format!("{identifier}.unit"));
                std::fs::write(&unit_file, unit).unwrap();
                let _ = writeln!(
                    units,
                    "    <EmbeddedResource Include=\"{}\" LogicalName=\"{}\" WithCulture=\"false\" />",
                    escape_item_attribute(&unit_file.to_string_lossy()),
                    escape_item_attribute(&case.unit_resource)
                );
                if let Some(blocks) = blocks {
                    std::fs::write(cases_dir.join(format!("{identifier}.test.cs")), blocks)
                        .unwrap();
                }
                cases.insert(identifier.to_string(), case);
            }
            Err(e) => {
                failures.insert(
                    identifier.to_string(),
                    format!("compiling the .slint file failed:\n{e}"),
                );
            }
        }
    }

    units.push_str("  </ItemGroup>\n</Project>\n");
    std::fs::write(dir.join("units.props"), units).unwrap();

    std::fs::write(
        dir.join("runner.csproj"),
        format!(
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net10.0</TargetFramework>
    <!-- Slint.csproj makes Slint.Testing visible to this assembly. -->
    <AssemblyName>Slint.TestDriver</AssemblyName>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <GenerateFullPaths>true</GenerateFullPaths>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
    <PackageReference Include="MSTest.TestFramework" Version="4.0.2" />
    <Using Include="Microsoft.VisualStudio.TestTools.UnitTesting" />
    <Using Include="Slint" />
  </ItemGroup>
  <Import Project="units.props" />
</Project>
"#,
            repository.join("api/dotnet/Slint/Slint.csproj").display()
        ),
    )
    .unwrap();

    // Each failed build drops the cases its errors point at, and tries again.
    let error = regex_lite(&cases_dir);
    loop {
        write_program(&dir, &cases);
        let output = std::process::Command::new("dotnet")
            .args(["build", "-nologo", "-v", "q", "-clp:NoSummary"])
            .arg(dir.join("runner.csproj"))
            .output()
            .expect("the .NET SDK must be installed");
        if output.status.success() {
            break;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut failed: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for line in stdout.lines() {
            if let Some(identifier) = error(line) {
                failed.entry(identifier).or_default().push(line);
            }
        }
        if failed.is_empty() {
            panic!(
                "building the runner failed:\n{stdout}{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        for (identifier, lines) in failed {
            let _ = std::fs::remove_file(cases_dir.join(format!("{identifier}.g.cs")));
            let _ = std::fs::remove_file(cases_dir.join(format!("{identifier}.test.cs")));
            cases.remove(&identifier);
            failures.insert(
                identifier,
                format!("the generated C# doesn't compile:\n{}", lines.join("\n")),
            );
        }
    }

    Batch { failures, runner: dir.join("bin/Debug/net10.0/Slint.TestDriver.dll") }
}

/// Escapes `text` for an attribute of an MSBuild item, which expands `%`, `$`, `@`, and `;`.
fn escape_item_attribute(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        match c {
            '%' | '$' | '@' | ';' | '\'' => {
                let _ = write!(escaped, "%{:02X}", c as u32);
            }
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// Returns a function that extracts the case of a C# compiler error line, such as
/// `/…/cases/basic_x.g.cs(3,5): error CS0103: …`, or of its `csharp` blocks in `basic_x.test.cs`.
fn regex_lite(cases_dir: &Path) -> impl Fn(&str) -> Option<String> {
    let prefix = format!("{}{}", cases_dir.display(), std::path::MAIN_SEPARATOR);
    move |line: &str| {
        let rest = line.trim().strip_prefix(&prefix)?;
        let (file, rest) = rest.split_once(".g.cs(").or_else(|| rest.split_once(".test.cs("))?;
        rest.contains("): error ").then(|| file.to_string())
    }
}
