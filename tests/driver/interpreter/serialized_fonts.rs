// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::source_path::SourcePath;
use i_slint_core::InternalToken;
use slint_interpreter::{CompilationResult, Value};

/// Compiles a component that imports a copy of the Plaster font, deletes the copy,
/// and measures "Hello" in the component created from the unit, with or without the font's data.
fn width_of_hello_from_unit(with_fonts: bool) -> f64 {
    let dir =
        std::env::temp_dir().join(format!("slint-unit-fonts-{}-{with_fonts}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let plaster = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/slide_puzzle/plaster-font/Plaster-Regular.ttf");
    std::fs::copy(plaster, dir.join("Plaster-Regular.ttf")).unwrap();
    let source = r#"
import "Plaster-Regular.ttf";
export component App inherits Window {
    label := Text { text: "Hello"; font-family: "Plaster"; font-size: 20px; }
    out property <length> label-width: label.preferred-width;
}"#;
    let path = SourcePath::File(dir.join("app.slint"));
    let config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    let (diagnostics, loader, _) = spin_on::spin_on(i_slint_compiler::load_root_file(
        &path,
        source.into(),
        Default::default(),
        config,
        false,
    ));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    let document = loader.get_document(&path).unwrap();
    let unit = i_slint_compiler::llr::lower_to_item_tree::lower_to_item_tree(
        document,
        &loader.compiler_config,
    );
    // The font files by the path that the unit registers each with, as a viewer on another device receives them.
    let fonts = loader
        .all_documents()
        .flat_map(|doc| doc.custom_fonts.iter())
        .map(|(font, _)| (font.to_string(), std::fs::read(font.as_native_path().unwrap()).unwrap()))
        .collect::<Vec<_>>();
    std::fs::remove_dir_all(&dir).unwrap();

    let unit = i_slint_compiler::llr::serialize::from_bytes(
        &i_slint_compiler::llr::serialize::to_bytes(&unit).unwrap(),
    )
    .unwrap();
    let result = CompilationResult::from_compilation_unit_with_fonts(
        unit,
        [],
        if with_fonts { fonts } else { Vec::new() },
        InternalToken,
    );
    let instance = result.component("App").unwrap().create().unwrap();
    let Value::Number(width) = instance.get_property("label-width").unwrap() else {
        panic!("label-width isn't a number")
    };
    width
}

#[test]
fn a_unit_registers_the_fonts_it_carries() {
    i_slint_backend_testing::init_no_event_loop();
    i_slint_backend_testing::configure_test_fonts();
    // "Hello" is 62px wide in Plaster, and 49px in the fallback font.
    // A registered font stays registered, so the unit without the font's data comes first.
    assert_eq!(width_of_hello_from_unit(false), 49.);
    assert_eq!(width_of_hello_from_unit(true), 62.);
}
