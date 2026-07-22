// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let fonts_dir: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "fonts"].iter().collect();
    let primary_font = fonts_dir.join("NotoSans-Regular.ttf");

    // Safety: there are no other threads at this point
    unsafe {
        std::env::set_var("SLINT_DEFAULT_FONT", &primary_font);
        std::env::set_var("SLINT_FONT_PATH", &fonts_dir);
    }
    println!("cargo:rustc-env=SLINT_DEFAULT_FONT={}", primary_font.display());
    println!("cargo:rustc-env=SLINT_FONT_PATH={}", fonts_dir.display());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    let mut generated_file = BufWriter::new(std::fs::File::create(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"),
    )?);

    #[cfg(feature = "software")]
    gen_software(&mut generated_file)?;

    #[cfg(feature = "skia")]
    gen_skia(&mut generated_file)?;

    #[cfg(feature = "software-embed-assets")]
    gen_software_embed_assets(&mut generated_file)?;

    generated_file.flush()?;

    Ok(())
}

// Renders every case with everything pre-rendered at compile time (`EmbedTextures`): bitmap fonts
// and pre-decoded textures, the MCU-style path. Compares against (and creates) a single
// `references/software_embed_assets` reference per case.
#[cfg(feature = "software-embed-assets")]
fn gen_software_embed_assets(generated_file: &mut impl Write) -> std::io::Result<()> {
    let references_root_dir: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "references", "software_embed_assets"].iter().collect();

    for testcase in test_driver_lib::collect_test_cases("screenshots/cases")? {
        let mut reference_path = references_root_dir
            .join(testcase.relative_path.clone())
            .with_extension("png")
            .to_str()
            .unwrap()
            .escape_default()
            .to_string();

        reference_path = format!("\"{reference_path}\"");

        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let module_name = testcase.identifier();

        writeln!(
            generated_file,
            "#[path=\"{module_name}.embed.rs\"] mod r#software_embed_assets_{module_name};"
        )?;
        let source = std::fs::read_to_string(&testcase.absolute_path)?;

        let markers = parse_markers(&source, &testcase);
        let ignored =
            if testcase.is_ignored("software") || testcase.is_ignored("software-embed-assets") {
                "#[ignore]"
            } else {
                ""
            };

        let mut output = BufWriter::new(std::fs::File::create(
            Path::new(&std::env::var_os("OUT_DIR").unwrap())
                .join(format!("{module_name}.embed.rs")),
        )?);

        generate_source(
            source.as_str(),
            &mut output,
            testcase,
            markers.scale_factor.unwrap_or(1.),
            i_slint_compiler::EmbedResourcesKind::EmbedTextures,
        )
        .unwrap();

        write_software_test(
            &mut output,
            &markers,
            ignored,
            SoftwareDriver::EmbedAssets { reference: reference_path },
        )?;

        output.flush()?;
    }
    Ok(())
}

#[cfg(feature = "software")]
fn generate_source(
    source: &str,
    output: &mut impl Write,
    testcase: test_driver_lib::TestCase,
    scale_factor: f32,
    embed_resources: i_slint_compiler::EmbedResourcesKind,
) -> Result<(), std::io::Error> {
    use i_slint_compiler::{diagnostics::BuildDiagnostics, *};

    let include_paths = test_driver_lib::extract_include_paths(source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();

    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse(source.to_owned(), Some(&testcase.absolute_path), &mut diag);
    let mut compiler_config = CompilerConfiguration::new(generator::OutputFormat::Rust);
    compiler_config.include_paths = include_paths;
    compiler_config.embed_resources = embed_resources;
    compiler_config.enable_experimental = true;
    compiler_config.style = Some("fluent".to_string());
    compiler_config.const_scale_factor = scale_factor.into();
    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));

    if diag.has_errors() {
        diag.print_warnings_and_exit_on_error();
        return Err(std::io::Error::other(format!("build error in {:?}", testcase.absolute_path)));
    } else {
        diag.print();
    }

    generator::generate(
        generator::OutputFormat::Rust,
        output,
        None,
        &root_component,
        &loader.compiler_config,
    )?;
    Ok(())
}

// Test parameters parsed from `KEY=value` / `KEY` markers in a case's source comments.
#[cfg(feature = "software")]
struct ScreenshotMarkers {
    scale_factor: Option<f32>,
    base_threshold: f32,
    rotation_threshold: f32,
    size: (u32, u32),
    skip_clipping: bool,
    skip_line_by_line: bool,
}

#[cfg(feature = "software")]
fn parse_markers(source: &str, testcase: &test_driver_lib::TestCase) -> ScreenshotMarkers {
    // The `f32` value following `needle`, up to the next whitespace; `None` if the marker is
    // absent, panicking if it is present but malformed.
    let parse_f32 = |needle: &str| -> Option<f32> {
        source.find(needle).map(|p| {
            let rest = &source[p + needle.len()..];
            rest.find(char::is_whitespace).and_then(|end| rest[..end].parse().ok()).unwrap_or_else(
                || panic!("Cannot parse {needle} for {}", testcase.relative_path.display()),
            )
        })
    };

    let size = source.find("SIZE=").map_or((64, 64), |p| {
        let rest = &source[p + "SIZE=".len()..];
        rest.find(char::is_whitespace)
            .and_then(|end| rest[..end].split_once('x'))
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .unwrap_or_else(|| {
                panic!("Cannot parse SIZE= for {}", testcase.relative_path.display())
            })
    });

    ScreenshotMarkers {
        scale_factor: parse_f32("SLINT_SCALE_FACTOR="),
        base_threshold: parse_f32("BASE_THRESHOLD=").unwrap_or(0.),
        rotation_threshold: parse_f32("ROTATION_THRESHOLD=").unwrap_or(0.),
        size,
        skip_clipping: source.contains("SKIP_CLIPPING"),
        skip_line_by_line: source.contains("SKIP_LINE_BY_LINE"),
    }
}

// Which software driver a generated test belongs to. `EmbedAssets` (everything pre-rendered at
// compile time) compares against and creates a single reference. `RuntimeAssets` (vector fonts plus
// runtime-decoded images) compares against the `software/` reference if it exists, otherwise the
// `software_embed_assets/` one, creating new references only under `software/`. All values are
// quoted string literals.
#[cfg(feature = "software")]
enum SoftwareDriver {
    EmbedAssets { reference: String },
    RuntimeAssets { primary: String, fallback: String },
}

// Emits the `#[test]` body that renders the (already generated) `TestCase` with the software
// renderer and compares it against the driver's reference(s).
#[cfg(feature = "software")]
fn write_software_test(
    output: &mut impl Write,
    markers: &ScreenshotMarkers,
    ignored: &str,
    driver: SoftwareDriver,
) -> std::io::Result<()> {
    let (size_w, size_h) = markers.size;
    let base_threshold = markers.base_threshold;
    let rotation_threshold = markers.rotation_threshold;
    let skip_clipping = markers.skip_clipping;
    let skip_line_by_line = if markers.skip_line_by_line { "#[cfg(false)]" } else { "" };

    // The embed-assets path checks the upright render, rotations, and line-by-line/partial
    // rendering. The runtime-assets path only checks the upright render: it registers the bundled
    // test fonts (so cases resolve fonts deterministically without a system font dependency) and
    // decodes resources at runtime, a path that isn't rotation/partial-render stable (e.g. image
    // scaling) -- and those renderer dimensions are already covered for every case by the
    // embed-assets path.
    let (configure_fonts, reference_setup, asserts) = match driver {
        SoftwareDriver::EmbedAssets { reference } => (
            "",
            format!("let screenshot = {reference};"),
            format!(
                "    crate::software::assert_with_render(screenshot, window.clone(), &options);\n\n    {skip_line_by_line}\n    crate::software::assert_with_render_by_line(screenshot, window.clone(), &options);"
            ),
        ),
        SoftwareDriver::RuntimeAssets { primary, fallback } => (
            "i_slint_backend_testing::configure_test_fonts();",
            format!(
                "let (screenshot, options) = crate::software::resolve_software_reference({primary}, {fallback}, &options);"
            ),
            "    crate::software::assert_base_render(&screenshot, window.clone(), &options);"
                .to_string(),
        ),
    };

    write!(
        output,
        r"
    #[test] {ignored} fn sw() -> Result<(), Box<dyn std::error::Error>> {{

    let window = crate::software::init_swr();
    {configure_fonts}
    window.set_size(slint::PhysicalSize::new({size_w}, {size_h}));
    let options = crate::testing::TestCaseOptions {{ base_threshold: {base_threshold}f32, rotation_threshold: {rotation_threshold}f32, skip_clipping: {skip_clipping}, create_path: None }};
    {reference_setup}

    let instance = TestCase::new().unwrap();
    instance.show().unwrap();

{asserts}

    Ok(())
    }}",
    )
}

#[cfg(feature = "skia")]
fn gen_skia(generated_file: &mut impl Write) -> Result<(), std::io::Error> {
    let references_root_dir: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "references", "skia"].iter().collect();

    for testcase in test_driver_lib::collect_test_cases("screenshots/cases")? {
        let reference_path = references_root_dir
            .join(testcase.relative_path.clone())
            .with_extension("png")
            .to_string_lossy()
            .into_owned();
        let absolute_path = testcase.absolute_path.to_string_lossy();
        let relative_path = testcase.relative_path.to_string_lossy();

        let identifier = testcase.identifier();
        let ignored = if testcase.is_ignored("skia") { "#[ignore]" } else { "" };

        write!(
            generated_file,
            r##"
#[test] {ignored}
fn skia_{identifier}() -> Result<(), Box<dyn std::error::Error>> {{
    crate::skia::run_test(crate::skia::TestCase {{
        absolute_path: std::path::PathBuf::from(r#"{absolute_path}"#),
        relative_path: std::path::PathBuf::from(r#"{relative_path}"#),
        reference_path: std::path::PathBuf::from(r#"{reference_path}"#),
    }})
}}"##,
        )?;
    }

    Ok(())
}

// The default software driver: compiles every case with `EmbedAllResources`, so nothing is
// pre-rendered at compile time. Fonts stay vector fonts (registered via `register_font_from_memory`
// plus `configure_test_fonts()`, laid out with parley) and images are decoded at runtime. Cases
// excluded from the software renderer (`//ignore: software`) are skipped. Each case compares against
// its `references/software` reference if one exists, otherwise the `references/software_embed_assets`
// one, so a reference is only added where the runtime-decoded output differs from the pre-rendered
// output.
#[cfg(feature = "software")]
fn gen_software(generated_file: &mut impl Write) -> std::io::Result<()> {
    let quoted_reference = |root: &str, testcase: &test_driver_lib::TestCase| {
        let path: std::path::PathBuf =
            [env!("CARGO_MANIFEST_DIR"), "references", root].iter().collect();
        let path = path
            .join(testcase.relative_path.clone())
            .with_extension("png")
            .to_str()
            .unwrap()
            .escape_default()
            .to_string();
        format!("\"{path}\"")
    };

    for testcase in test_driver_lib::collect_test_cases("screenshots/cases")? {
        if testcase.is_ignored("software") {
            continue;
        }
        let source = std::fs::read_to_string(&testcase.absolute_path)?;

        let primary = quoted_reference("software", &testcase);
        let fallback = quoted_reference("software_embed_assets", &testcase);

        println!("cargo:rerun-if-changed={}", testcase.absolute_path.display());
        let module_name = testcase.identifier();

        writeln!(generated_file, "#[path=\"{module_name}.rs\"] mod r#software_{module_name};")?;

        let markers = parse_markers(&source, &testcase);

        let mut output = BufWriter::new(std::fs::File::create(
            Path::new(&std::env::var_os("OUT_DIR").unwrap()).join(format!("{module_name}.rs")),
        )?);

        generate_source(
            source.as_str(),
            &mut output,
            testcase,
            markers.scale_factor.unwrap_or(1.),
            i_slint_compiler::EmbedResourcesKind::EmbedAllResources,
        )
        .unwrap();

        write_software_test(
            &mut output,
            &markers,
            "",
            SoftwareDriver::RuntimeAssets { primary, fallback },
        )?;

        output.flush()?;
    }

    Ok(())
}
