// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Generates C# for every test case, builds all of it into one runner, and runs each case in its own process.
//! A `dotnet build` per case would take seconds each.

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
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn generate(identifier: &str, path: &Path) -> Result<(String, Case), String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source, Some(source_path::SourcePath::new(path)), &mut diag);
    let format = generator::OutputFormat::CSharp(csharp::Config {
        namespace: Some(format!("Case_{identifier}")),
        runtime_path: Some(path.to_string_lossy().into_owned()),
    });
    let mut config = CompilerConfiguration::new(format.clone());
    config.style = Some(STYLE.into());
    let (doc, diag, loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    if diag.has_errors() {
        return Err(diag.to_string_vec().join("\n"));
    }

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
    Ok((String::from_utf8(code).unwrap(), Case { components }))
}

fn write_program(dir: &Path, cases: &BTreeMap<String, Case>) {
    let mut program = String::from(
        "internal static class Program\n{\n    private static int Main(string[] args)\n    {\n        switch (args[0])\n        {\n",
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

    let mut failures = HashMap::new();
    let mut cases = BTreeMap::new();
    for line in std::fs::read_to_string(env!("TEST_CASES")).unwrap().lines() {
        let (identifier, path) = line.split_once('\t').unwrap();
        match generate(identifier, Path::new(path)) {
            Ok((code, case)) => {
                std::fs::write(cases_dir.join(format!("{identifier}.g.cs")), code).unwrap();
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

    std::fs::write(
        dir.join("runner.csproj"),
        format!(
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net10.0</TargetFramework>
    <Nullable>enable</Nullable>
    <GenerateFullPaths>true</GenerateFullPaths>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
  </ItemGroup>
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
            cases.remove(&identifier);
            failures.insert(
                identifier,
                format!("the generated C# doesn't compile:\n{}", lines.join("\n")),
            );
        }
    }

    Batch { failures, runner: dir.join("bin/Debug/net10.0/runner.dll") }
}

/// Returns a function that extracts the case of a C# compiler error line, such as
/// `/…/cases/basic_x.g.cs(3,5): error CS0103: …`.
fn regex_lite(cases_dir: &Path) -> impl Fn(&str) -> Option<String> {
    let prefix = format!("{}{}", cases_dir.display(), std::path::MAIN_SEPARATOR);
    move |line: &str| {
        let rest = line.trim().strip_prefix(&prefix)?;
        let (file, rest) = rest.split_once(".g.cs(")?;
        rest.contains("): error ").then(|| file.to_string())
    }
}
