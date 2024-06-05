// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use napi::Result;
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
    pub fn functions(&self) -> Vec<String> {
        self.internal.functions().collect()
    }

    #[napi(getter)]
    pub fn globals(&self) -> Vec<String> {
        self.internal.globals().collect()
    }

    #[napi]
    pub fn global_properties(&self, global_name: String) -> Option<Vec<JsProperty>> {
        self.internal.global_properties(global_name.as_str()).map(|iter| {
            iter.map(|(name, value_type)| JsProperty { name, value_type: value_type.into() })
                .collect()
        })
    }

    #[napi]
    pub fn global_callbacks(&self, global_name: String) -> Option<Vec<String>> {
        self.internal.global_callbacks(global_name.as_str()).map(|iter| iter.collect())
    }

    #[napi]
    pub fn global_functions(&self, global_name: String) -> Option<Vec<String>> {
        self.internal.global_functions(global_name.as_str()).map(|iter| iter.collect())
    }

    #[napi]
    pub fn create(&self) -> Result<JsComponentInstance> {
        Ok(self.internal.create().map_err(|e| napi::Error::from_reason(e.to_string()))?.into())
    }

    #[napi(getter)]
    pub fn name(&self) -> String {
        self.internal.name().into()
    }
}
