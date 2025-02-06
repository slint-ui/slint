// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Test that all styles have the same API.

use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::{Function, Type};
use i_slint_compiler::object_tree::PropertyVisibility;
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;
use smol_str::{SmolStr, ToSmolStr};
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::rc::Rc;

#[derive(PartialEq, Debug)]
struct PropertyInfo {
    ty: Type,
    vis: PropertyVisibility,
    pure: bool,
}

impl Display for PropertyInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}{:?}", self.ty, if self.pure { "pure-" } else { "" }, self.vis)?;
        if let Type::Callback(cb) = &self.ty {
            if !cb.arg_names.is_empty() {
                write!(f, "{:?}", cb.arg_names)?
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Component {
    properties: BTreeMap<String, PropertyInfo>,
    accessible_role: Option<String>,
}

#[derive(Default)]
struct Style {
    components: BTreeMap<SmolStr, Component>,
    structs: BTreeMap<SmolStr, Type>,
}

fn load_component(component: &Rc<i_slint_compiler::object_tree::Component>) -> Component {
    let mut result = Component::default();
    let mut elem = component.root_element.clone();
    loop {
        result.properties.extend(
            elem.borrow()
                .property_declarations
                .iter()
                .filter(|(_, v)| v.visibility != PropertyVisibility::Private)
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        PropertyInfo {
                            ty: v.property_type.clone(),
                            vis: v.visibility,
                            pure: v.pure.unwrap_or(false),
                        },
                    )
                }),
        );

        if result.accessible_role.is_none() {
            if let Some(role) = elem.borrow().bindings.get("accessible-role") {
                match &role.borrow().expression {
                    Expression::Invalid => (),
                    Expression::EnumerationValue(e) => {
                        result.accessible_role = Some(e.enumeration.values[e.value].to_string())
                    }
                    e => panic!(
                        "accessible-role not an EnumerationValue : {e:?}    (for {:?})",
                        role.borrow().span
                    ),
                };
            }
        }

        let e = match &elem.borrow().base_type {
            i_slint_compiler::langtype::ElementType::Component(r) => r.root_element.clone(),
            i_slint_compiler::langtype::ElementType::Builtin(b) => {
                let builtins = i_slint_compiler::typeregister::reserved_properties()
                    .map(|x| x.0.to_smolstr())
                    .collect::<HashSet<_>>();
                result.properties.extend(
                    b.properties.iter().filter(|(k, _)| !builtins.contains(*k)).map(|(k, v)| {
                        (
                            k.to_string(),
                            PropertyInfo {
                                ty: v.ty.clone(),
                                vis: v.property_visibility,
                                pure: false,
                            },
                        )
                    }),
                );
                // Synthesize focus() and `clear-focus()` as styles written in .slint will have it but the qt style exposes NativeXX directly.
                if b.accepts_focus {
                    result.properties.insert(
                        "focus".into(),
                        PropertyInfo {
                            ty: Type::Function(Rc::new(Function {
                                return_type: Type::Void,
                                args: vec![],
                                arg_names: vec![],
                            })),
                            vis: PropertyVisibility::Public,
                            pure: false,
                        },
                    );
                    result.properties.insert(
                        "clear-focus".into(),
                        PropertyInfo {
                            ty: Type::Function(Rc::new(Function {
                                return_type: Type::Void,
                                args: vec![],
                                arg_names: vec![],
                            })),
                            vis: PropertyVisibility::Public,
                            pure: false,
                        },
                    );
                }
                break;
            }
            i_slint_compiler::langtype::ElementType::Native(_) => unreachable!(),
            i_slint_compiler::langtype::ElementType::Error => unreachable!(),
            i_slint_compiler::langtype::ElementType::Global => break,
        };
        elem = e;
    }
    result
}

fn load_style(style_name: String) -> Style {
    let mut config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Llr,
    );
    config.style = Some(style_name);
    let mut diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    let mut loader = TypeLoader::new(TypeRegister::builtin(), config, &mut diag);
    // ensure that the style is loaded
    spin_on::spin_on(loader.import_component("std-widgets.slint", "Button", &mut diag));

    if diag.has_errors() {
        #[cfg(feature = "display-diagnostics")]
        diag.print();
        panic!("error parsing style {}", loader.compiler_config.style.as_ref().unwrap());
    }

    let doc = loader
        .get_document(&loader.resolve_import_path(None, "std-widgets.slint").unwrap().0)
        .unwrap();

    let mut style = Style::default();

    for (name, what) in doc.exports.iter() {
        let name = &**name;
        match what {
            itertools::Either::Left(component) => {
                let component = load_component(component);
                let old = style.components.insert(name.clone(), component);
                assert!(
                    old.is_none(),
                    "Duplicated component '{name}' in style {}",
                    loader.compiler_config.style.as_ref().unwrap()
                );
            }
            itertools::Either::Right(ty) => {
                let old = style.structs.insert(name.clone(), ty.clone());
                assert!(
                    old.is_none(),
                    "Duplicated struct '{name}' in style {}",
                    loader.compiler_config.style.as_ref().unwrap()
                );
            }
        }
    }
    style
}

fn compare_styles(base: &Style, mut other: Style, style_name: &str) -> bool {
    let mut ok = true;
    for (compo_name, c1) in base.components.iter() {
        // These more or less internals component can have different properties
        let ignore_extra =
            matches!(compo_name.as_str(), "TabImpl" | "TabWidgetImpl" | "StyleMetrics");
        if let Some(mut c2) = other.components.remove(compo_name) {
            if c1.accessible_role != c2.accessible_role {
                eprintln!(
                    "Mismatch accessible-role for {compo_name} in {style_name} : {:?} != {:?}",
                    c2.accessible_role, c1.accessible_role
                );
                ok = false;
            }

            for (prop_name, p1) in c1.properties.iter() {
                if let Some(p2) = c2.properties.remove(prop_name) {
                    if p1 != &p2 {
                        eprintln!("Mismatch property info '{compo_name}::{prop_name}' in {style_name} : {p1} != {p2}",);
                        ok = false;
                    }
                } else if !ignore_extra {
                    eprintln!("Property '{compo_name}::{prop_name}' not found in {style_name}");
                    ok = false;
                }
            }
            // Extra property on StyleMetrics are allowed
            if !c2.properties.is_empty() && !ignore_extra {
                for prop_name in c2.properties.keys() {
                    eprintln!("Extra property '{compo_name}::{prop_name}' found in {style_name}");
                }
                ok = false;
            }
        } else {
            eprintln!("Component '{compo_name}' not found in {style_name}");
            ok = false;
        }
    }
    if !other.components.is_empty() {
        for compo_name in other.components.keys() {
            eprintln!("Extra component '{compo_name}' found in {style_name}");
        }
        ok = false;
    }
    if base.structs != other.structs {
        eprintln!(
            "Mismatch struct export in '{style_name}': {:?} != {:?}",
            base.structs, other.structs
        );
        ok = false;
    }
    ok
}

#[test]
fn check_styles() {
    let base = load_style("fluent".into());

    let mut ok = true;
    for s in i_slint_compiler::fileaccess::styles() {
        let other = load_style(s.into());
        ok &= compare_styles(&base, other, s);
    }

    assert!(ok);
}
