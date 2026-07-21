// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint!(export { App } from "ui/app.slint";);

// The external plugin is an independently-delivered part. Here it is loaded from
// source at runtime via the interpreter, but it could equally be a separately
// compiled binary. The shell mounts it through `mount extern via FeatureNav`.
fn plugin_factory() -> slint::ComponentFactory {
    slint::ComponentFactory::new(|ctx| {
        let compiler = slint_interpreter::Compiler::new();
        let def = spin_on::spin_on(
            compiler.build_from_source(
                r#"
export component Plugin inherits Rectangle {
    background: #2b2b40;
    VerticalLayout {
        alignment: center;
        Text {
            text: "External plugin\n(loaded via ComponentFactory)";
            color: white;
            horizontal-alignment: center;
        }
    }
}
"#
                .into(),
                std::path::PathBuf::from("plugin.slint"),
            ),
        )
        .component("Plugin")
        .unwrap();
        def.create_embedded(ctx).ok()
    })
}

pub fn main() {
    let app = App::new().unwrap();
    app.set_plugin_factory(plugin_factory());
    app.run().unwrap();
}
