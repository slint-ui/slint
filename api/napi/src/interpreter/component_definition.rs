// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use slint_interpreter::ComponentDefinition;

use super::{JsComponentInstance, JsProperty};

#[napi(js_name = "ComponentDefinition")]
pub struct JsComponentDefinition {
    internal: ComponentDefinition,
}

impl From<ComponentDefinition> for JsComponentDefinition {
    fn from(definition: ComponentDefinition) -> Self {
        Self { internal: definition }
    }
}

#[napi]
impl JsComponentDefinition {
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Err(napi::Error::from_reason(
            "ComponentDefinition can only be created by using ComponentCompiler.".to_string(),
        ))
    }

    #[napi(getter)]
    pub fn properties(&self) -> Vec<JsProperty> {
        self.internal
            .properties()
            .map(|(name, value_type)| JsProperty { name, value_type: value_type.into() })
            .collect()
    }

    #[napi(getter)]
    pub fn callbacks(&self) -> Vec<String> {
        self.internal.callbacks().collect()
    }

    #[napi(getter)]
    pub fn global_properties(&self, global_name: String) -> Option<Vec<JsProperty>> {
        self.internal.global_properties(global_name.as_str()).map(|iter| {
            iter.map(|(name, value_type)| JsProperty { name, value_type: value_type.into() })
                .collect()
        })
    }

    #[napi(getter)]
    pub fn global_callbacks(&self, global_name: String) -> Option<Vec<String>> {
        self.internal.global_callbacks(global_name.as_str()).map(|iter| iter.collect())
    }

    #[napi]
    pub fn create(&self) -> Option<JsComponentInstance> {
        if let Ok(instance) = self.internal.create() {
            return Some(instance.into());
        }

        None
    }

    #[napi(getter)]
    pub fn name(&self) -> String {
        self.internal.name().into()
    }
}
