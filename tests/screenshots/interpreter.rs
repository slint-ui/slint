// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::InternalToken;

/// The case at `path`, compiled like the generated embed-assets cases.
pub fn load(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> slint_interpreter::ComponentInstance {
    let (source, compiler) = compiler(path, scale_factor, include_paths);
    let result = spin_on::spin_on(compiler.build_from_source(source, path.into()));
    result.print_diagnostics();
    assert!(!result.has_errors(), "compile error in {path}");
    create(result, scale_factor)
}

/// Like [`load()`], after sending the compiled unit through serialization,
/// as a microcontroller receives it.
pub fn load_serialized(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> slint_interpreter::ComponentInstance {
    use i_slint_compiler::llr::serialize;
    let (source, mut compiler) = compiler(path, scale_factor, include_paths);
    let mut config = compiler.compiler_configuration(InternalToken).clone();
    // The compiler keeps these as overrides, outside of its configuration
    config.include_paths = compiler.include_paths().clone();
    config.style = compiler.style().cloned();
    let (diagnostics, compiled) = spin_on::spin_on(serialize::compile(
        &i_slint_compiler::source_path::SourcePath::new(path),
        source,
        config,
    ));
    let (unit, resources) = compiled
        .unwrap_or_else(|| panic!("compile error in {path}: {:?}", diagnostics.to_string_vec()));
    let unit = serialize::from_bytes(&serialize::to_bytes(&unit).unwrap()).unwrap();
    let resources = resources.iter().map(|resource| {
        let bytes = serialize::resource_to_bytes(resource).unwrap();
        slint_interpreter::LoadedResource::new(
            serialize::resource_from_bytes(&bytes).unwrap(),
            InternalToken,
        )
    });
    let received =
        slint_interpreter::CompilationResult::from_compilation_unit(unit, resources, InternalToken);
    create(received, scale_factor)
}

/// The source of the case at `path`, and a compiler set up for it.
fn compiler(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> (String, slint_interpreter::Compiler) {
    // The build script sets these for the compiler it runs; this one runs now.
    // Safety: the test threads only read these, and all set the same values.
    unsafe {
        std::env::set_var("SLINT_DEFAULT_FONT", env!("SLINT_DEFAULT_FONT"));
        std::env::set_var("SLINT_FONT_PATH", env!("SLINT_FONT_PATH"));
    }
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    compiler.set_include_paths(include_paths.iter().map(Into::into).collect());
    compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::EmbedTextures);
    let config = compiler.compiler_configuration(InternalToken);
    config.experimental_features = i_slint_compiler::ExperimentalFeatures::all();
    config.const_scale_factor = Some(scale_factor);
    (std::fs::read_to_string(path).unwrap(), compiler)
}

fn create(
    result: slint_interpreter::CompilationResult,
    scale_factor: f32,
) -> slint_interpreter::ComponentInstance {
    let instance = result.component("TestCase").expect("no TestCase component").create().unwrap();
    // The window must have the scale factor that the compiler pre-rendered for.
    slint::ComponentHandle::window(&instance)
        .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
    instance
}
