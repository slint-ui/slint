// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashMap, fmt::Display};

use slint_interpreter::ComponentInstance;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeComponent {
    Main,
    Global(String),
}

impl Display for RuntimeComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeComponent::Main => write!(f, "<MAIN>"),
            RuntimeComponent::Global(g) => write!(f, "{g}"),
        }
    }
}

fn has_getter(visibility: &i_slint_compiler::object_tree::PropertyVisibility) -> bool {
    matches!(
        visibility,
        i_slint_compiler::object_tree::PropertyVisibility::Output
            | i_slint_compiler::object_tree::PropertyVisibility::InOut
    )
}

fn has_setter(visibility: &i_slint_compiler::object_tree::PropertyVisibility) -> bool {
    matches!(
        visibility,
        i_slint_compiler::object_tree::PropertyVisibility::Input
            | i_slint_compiler::object_tree::PropertyVisibility::InOut
    )
}

fn is_property(ty: &i_slint_compiler::langtype::Type) -> bool {
    !matches!(
        *ty,
        i_slint_compiler::langtype::Type::Function(_)
            | i_slint_compiler::langtype::Type::Callback(_)
            | i_slint_compiler::langtype::Type::InferredCallback
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProperty {
    pub name: String,
    pub ty: i_slint_compiler::langtype::Type,
    pub visibility: i_slint_compiler::object_tree::PropertyVisibility,
    pub value: Option<slint_interpreter::Value>,
}

impl RuntimeProperty {
    pub fn is_function(&self) -> bool {
        matches!(self.ty, i_slint_compiler::langtype::Type::Function(_))
    }

    pub fn is_callback(&self) -> bool {
        matches!(
            self.ty,
            i_slint_compiler::langtype::Type::Callback(_)
                | i_slint_compiler::langtype::Type::InferredCallback
        )
    }

    pub fn is_property(&self) -> bool {
        is_property(&self.ty)
    }

    pub fn has_getter(&self) -> bool {
        has_getter(&self.visibility)
    }

    pub fn has_setter(&self) -> bool {
        has_setter(&self.visibility)
    }
}

pub fn query_runtime_properties_and_callbacks(
    component_instance: &ComponentInstance,
) -> HashMap<RuntimeComponent, Vec<RuntimeProperty>> {
    let definition = &component_instance.definition();

    let mut result = HashMap::new();

    fn collect_runtime_properties(
        it: &mut dyn Iterator<
            Item = (
                String,
                (
                    i_slint_compiler::langtype::Type,
                    i_slint_compiler::object_tree::PropertyVisibility,
                ),
            ),
        >,
        value_query: &dyn Fn(&str) -> Option<slint_interpreter::Value>,
    ) -> Vec<RuntimeProperty> {
        let mut v = it
            .map(|(name, (ty, visibility))| {
                let value = value_query(&name);

                RuntimeProperty { name, ty, visibility, value }
            })
            .collect::<Vec<_>>();

        v.sort_by_key(|p| p.name.clone());
        v
    }

    result.insert(
        RuntimeComponent::Main,
        collect_runtime_properties(&mut definition.properties_and_callbacks(), &|name| {
            component_instance.get_property(name).ok()
        }),
    );

    for global in definition.globals() {
        result.insert(
            RuntimeComponent::Global(global.clone()),
            collect_runtime_properties(
                &mut definition
                    .global_properties_and_callbacks(&global)
                    .expect("Global was just valid"),
                &|name| component_instance.get_global_property(&global, &name).ok(),
            ),
        );
    }

    result
}

pub fn set_runtime_json_properties(
    component_instance: &ComponentInstance,
    component: RuntimeComponent,
    property_name: Option<String>,
    json: serde_json::Value,
) -> Result<(), Vec<String>> {
    eprintln!("APPLYING JSON VALUE FROM UI: CI + {component}.{property_name:?}: {json:?}");

    let definition = &component_instance.definition();

    let mut properties_set = 0_usize;
    let mut failed_properties = vec![];

    let it: Box<dyn Iterator<Item = _>> = match &component {
        RuntimeComponent::Main => Box::new(definition.properties_and_callbacks()),
        RuntimeComponent::Global(g) => Box::new(
            definition
                .global_properties_and_callbacks(g)
                .ok_or(vec![format!("Global {g} does not exist")])?,
        ),
    };

    let it: Box<dyn Iterator<Item = _>> = match &property_name {
        None => Box::new(it.filter(|(_, (it, iv))| is_property(it) && has_setter(iv))),
        Some(p) => {
            let p = p.clone();
            Box::new(it.filter(move |(ip, (it, iv))| &p == ip && is_property(it) && has_setter(iv)))
        }
    };

    for (name, (ty, _)) in it {
        let (name, json_value) = if let Some(pn) = &property_name {
            (pn.clone(), Some(&json))
        } else {
            let json_value = match &json {
                serde_json::Value::Object(obj) => {
                    if let Some(j) = obj.get(&name) {
                        j
                    } else {
                        failed_properties
                            .push(format!("Value for property {name} not found in JSON object"));
                        continue;
                    }
                }
                _ => {
                    failed_properties.push(
                    "JSON value must be an object when setting all properties of a Slint Element"
                        .to_string(),
                    );
                    continue;
                }
            };
            (name.clone(), Some(json_value))
        };

        let Some(json_value) = json_value else {
            // already logged an error for this!
            continue;
        };
        eprintln!("   *** properties to look at: {name} of type {ty:?} <= JS: {json_value:?}");

        let Ok(value) = slint_interpreter::json::value_from_json(&ty, json_value) else {
            failed_properties.push(format!("Could not convert JSON value for property {name}"));
            continue;
        };

        eprintln!("   *** properties to look at: {name} of type {ty:?} <= V: {value:?}");

        let result = match &component {
            RuntimeComponent::Main => component_instance.set_property(&name, value),
            RuntimeComponent::Global(g) => component_instance.set_global_property(g, &name, value),
        };

        if let Err(msg) = result {
            failed_properties.push(format!("Could not set property {name}: {msg}"));
            continue;
        } else {
            properties_set += 1;
        }
    }

    if !failed_properties.is_empty() {
        return Err(failed_properties);
    }

    if properties_set == 0 {
        Err(vec![format!("No property set")])
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        common::test::{main_test_file_name, test_file_name},
        preview::test::interpret_test_with_sources,
    };

    use std::collections::HashMap;

    #[test]
    fn test_query_runtime_properties_and_callbacks() {
        let component_instance = interpret_test_with_sources(
            "fluent",
            HashMap::from([
                (
                    main_test_file_name(),
                    String::from(
                        r#"
                    import { User2 } from "user2.slint";
                    export * from "user1.slint";

                    export global MainGlobal {
                        in property <int> main-global-in: 1;
                        out property <int> main-global-out: 2;
                        in-out property <int> main-global-in-out: 4;
                    }

                    export { User2 }

                    export component MainComponent {
                        in property <int> main-component-in: MainGlobal.main-global-in + User2.user2-in;
                        out property <int> main-component-out: MainGlobal.main-global-out + User2.user2-out;
                        in-out property <int> main-component-in-out: MainGlobal.main-global-in-out + User2.user2-in-out;
                    }
                "#,
                    ),
                ),
                (
                    test_file_name("user1.slint"),
                    String::from(
                        r#"
                    export global User1 {
                        in property <int> user1-in: 8;
                        out property <int> user1-out: 16;
                        in-out property <int> user1-in-out: 32;
                    }
                "#,
                    ),
                ),
                (
                    test_file_name("user2.slint"),
                    String::from(
                        r#"
                    export global User2 {
                        in property <int> user2-in: 64;
                        out property <int> user2-out: 128;
                        in-out property <int> user2-in-out: 256;
                    }
                "#,
                    ),
                ),
            ]),
        );

        let properties = query_runtime_properties_and_callbacks(&component_instance);

        assert_eq!(properties.len(), 4);

        let main = properties.get(&RuntimeComponent::Main).unwrap();

        assert_eq!(main.len(), 3);
        assert_eq!(
            main[0],
            RuntimeProperty {
                name: "main-component-in".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Input,
                value: Some(slint_interpreter::Value::Number(65.0)),
            }
        );
        assert_eq!(
            main[1],
            RuntimeProperty {
                name: "main-component-in-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::InOut,
                value: Some(slint_interpreter::Value::Number(260.0))
            }
        );
        assert_eq!(
            main[2],
            RuntimeProperty {
                name: "main-component-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Output,
                value: Some(slint_interpreter::Value::Number(130.0))
            }
        );

        let global = properties.get(&RuntimeComponent::Global("MainGlobal".into())).unwrap();

        assert_eq!(global.len(), 3);
        assert_eq!(
            global[0],
            RuntimeProperty {
                name: "main-global-in".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Input,
                value: Some(slint_interpreter::Value::Number(1.0))
            }
        );
        assert_eq!(
            global[1],
            RuntimeProperty {
                name: "main-global-in-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::InOut,
                value: Some(slint_interpreter::Value::Number(4.0))
            }
        );
        assert_eq!(
            global[2],
            RuntimeProperty {
                name: "main-global-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Output,
                value: Some(slint_interpreter::Value::Number(2.0))
            }
        );

        let user1 = properties.get(&RuntimeComponent::Global("User1".into())).unwrap();

        assert_eq!(user1.len(), 3);

        assert_eq!(
            user1[0],
            RuntimeProperty {
                name: "user1-in".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Input,
                value: Some(slint_interpreter::Value::Number(8.0))
            }
        );
        assert_eq!(
            user1[1],
            RuntimeProperty {
                name: "user1-in-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::InOut,
                value: Some(slint_interpreter::Value::Number(32.0))
            }
        );
        assert_eq!(
            user1[2],
            RuntimeProperty {
                name: "user1-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Output,
                value: Some(slint_interpreter::Value::Number(16.0))
            }
        );

        let user2 = properties.get(&RuntimeComponent::Global("User2".into())).unwrap();

        assert_eq!(user2.len(), 3);
        assert_eq!(
            user2[0],
            RuntimeProperty {
                name: "user2-in".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Input,
                value: Some(slint_interpreter::Value::Number(64.0))
            }
        );
        assert_eq!(
            user2[1],
            RuntimeProperty {
                name: "user2-in-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::InOut,
                value: Some(slint_interpreter::Value::Number(256.0))
            }
        );
        assert_eq!(
            user2[2],
            RuntimeProperty {
                name: "user2-out".into(),
                ty: i_slint_compiler::langtype::Type::Int32,
                visibility: i_slint_compiler::object_tree::PropertyVisibility::Output,
                value: Some(slint_interpreter::Value::Number(128.0))
            }
        );
    }
}
