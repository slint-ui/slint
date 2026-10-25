// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::InternalToken;

/// The case at `path`, compiled like the generated embed-assets cases.
pub fn load(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> slint_interpreter::ComponentInstance {
    // The build script sets these for the compiler it runs; this one runs now.
    // Safety: the test threads only read these, and all set the same values.
    unsafe {
        std::env::set_var("SLINT_DEFAULT_FONT", env!("SLINT_DEFAULT_FONT"));
        std::env::set_var("SLINT_FONT_PATH", env!("SLINT_FONT_PATH"));
    }
    let source = std::fs::read_to_string(path).unwrap();
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    compiler.set_include_paths(include_paths.iter().map(Into::into).collect());
    compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::EmbedTextures);
    let config = compiler.compiler_configuration(InternalToken);
    config.enable_experimental = true;
    config.const_scale_factor = Some(scale_factor);
    let result = spin_on::spin_on(compiler.build_from_source(source, path.into()));
    result.print_diagnostics();
    assert!(!result.has_errors(), "compile error in {path}");

    let instance = result.component("TestCase").expect("no TestCase component").create().unwrap();
    // The window must have the scale factor that the compiler pre-rendered for.
    slint::ComponentHandle::window(&instance)
        .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
    instance
}
