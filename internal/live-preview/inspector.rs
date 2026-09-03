// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

use i_slint_core::InternalToken;
use i_slint_core::item_tree::ItemTreeRc;
use i_slint_core::model::{ModelRc, VecModel};
use i_slint_core::window::{WindowAdapterRc, WindowInner};
use slint_interpreter::{ComponentHandle as _, Struct, Value};

pub struct InspectorOverlay {
    component: slint_interpreter::ComponentInstance,
    item_tree: ItemTreeRc,
    window_adapter: WindowAdapterRc,
}

impl InspectorOverlay {
    pub async fn new(window: &i_slint_core::api::Window) -> anyhow::Result<Self> {
        let compiler = slint_interpreter::Compiler::default();
        let compilation_result = compiler
            .build_from_source(
                include_str!("inspector.slint").into(),
                PathBuf::from("inspector.slint"),
            )
            .await;
        if compilation_result.has_errors() {
            compilation_result.print_diagnostics();
            anyhow::bail!("Cannot compile inspector overlay");
        }
        let definition = compilation_result
            .component("InspectorOverlay")
            .ok_or_else(|| anyhow::anyhow!("Inspector overlay component is missing"))?;
        let component = definition
            .create_detached_with_existing_window(window, InternalToken)
            .map_err(|error| anyhow::anyhow!("Cannot create inspector overlay: {error}"))?;
        let item_tree = component.as_item_tree(InternalToken);
        let window_adapter = WindowInner::from_pub(window).window_adapter();
        Ok(Self { component, item_tree, window_adapter })
    }

    pub fn attach(&self) -> anyhow::Result<()> {
        WindowInner::from_pub(self.window_adapter.window())
            .add_overlay(&self.item_tree)
            .map_err(|error| anyhow::anyhow!("Cannot show inspector overlay: {error}"))
    }

    pub fn detach(&self) {
        WindowInner::from_pub(self.window_adapter.window()).clear_overlays();
    }

    pub fn update(
        &self,
        user_instance: Option<&slint_interpreter::ComponentInstance>,
        highlight: Option<&(lsp_types::Url, u32)>,
    ) {
        let highlight = user_instance.zip(highlight).and_then(|(instance, (url, offset))| {
            url.to_file_path().ok().map(|path| (instance.as_weak(), path, *offset))
        });
        self.component
            .set_callback("highlight-positions", move |_| {
                let highlights: Vec<Value> = highlight
                    .as_ref()
                    .and_then(|(instance, path, offset)| {
                        instance.upgrade().map(|instance| (instance, path, *offset))
                    })
                    .into_iter()
                    .flat_map(|(instance, path, offset)| instance.component_positions(path, offset))
                    .map(|geometry| {
                        Value::Struct(Struct::from_iter([
                            ("x".into(), Value::Number(geometry.rect.origin.x.into())),
                            ("y".into(), Value::Number(geometry.rect.origin.y.into())),
                            ("width".into(), Value::Number(geometry.rect.size.width.into())),
                            ("height".into(), Value::Number(geometry.rect.size.height.into())),
                            ("angle".into(), Value::Number(geometry.angle.into())),
                        ]))
                    })
                    .collect();
                Value::Model(ModelRc::new(VecModel::from(highlights)))
            })
            .unwrap();
    }
}
