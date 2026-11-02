// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Serialization of a [`CompilationUnit`], for handing a compiled unit to another process
//! or another device.
//!
//! Write a unit with [`to_bytes()`] and read it with [`from_bytes()`]; this works without std.
//! The unit refers to its pre-rendered textures and fonts by index:
//! write each with [`resource_to_bytes()`], so that a receiver can keep those it already has.
//! The unit serializes on its own, without any state shared across fields.
//! The `Arc`s that the builtins provide are written as a reference to them,
//! and every other `Arc` is written by value, through serde's `rc` feature.
//! The native classes are the exception: the unit carries each one it uses, once.
//! Both ends must be built from the same Slint version: the format isn't stable.

use super::CompilationUnit;
use crate::langtype::{Enumeration, Struct, StructName};
use crate::native_class::{BuiltinPropertyInfo, NativeClass};
use crate::typeregister::{BUILTIN, builtin_structs};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use i_slint_common::pre_rendered::PreRenderedResource;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

/// Serializes `unit` for [`from_bytes()`].
pub fn to_bytes(unit: &CompilationUnit) -> postcard::Result<Vec<u8>> {
    postcard::to_allocvec(&(NativeClasses::of(unit), unit))
}

/// Compiles the root file at `path` like the interpreter does, into a unit for [`to_bytes()`],
/// and the resources it refers to by index for [`resource_to_bytes()`],
/// unless the compilation fails.
#[cfg(feature = "std")]
pub async fn compile(
    path: &crate::source_path::SourcePath,
    source_code: String,
    mut config: crate::CompilerConfiguration,
) -> (
    crate::diagnostics::BuildDiagnostics,
    Option<(CompilationUnit, Vec<Option<PreRenderedResource>>)>,
) {
    // Like the interpreter's `Compiler`
    if std::env::var_os("SLINT_INLINING").is_none() {
        config.inline_all_elements = false;
    }
    let (diagnostics, loader) =
        crate::load_root_file(path, source_code, Default::default(), config).await;
    let compiled =
        loader.get_document(path).filter(|_| !diagnostics.has_errors()).map(|document| {
            (
                super::lower_to_item_tree::lower_to_item_tree(document, &loader.compiler_config),
                crate::embedded_resources::pre_rendered_resources(document),
            )
        });
    (diagnostics, compiled)
}

/// Deserializes a unit written by [`to_bytes()`].
pub fn from_bytes(bytes: &[u8]) -> postcard::Result<CompilationUnit> {
    let (classes, mut unit): (NativeClasses, CompilationUnit) = postcard::from_bytes(bytes)?;
    for item in unit.sub_components.iter_mut().flat_map(|sc| sc.items.iter_mut()) {
        item.ty =
            classes.0.get(&item.ty.class_name).cloned().ok_or(postcard::Error::SerdeDeCustom)?;
    }
    Ok(unit)
}

/// Serializes a resource of a unit for [`resource_from_bytes()`].
/// `None` stands for a resource that the compiler didn't pre-render.
pub fn resource_to_bytes(resource: &Option<PreRenderedResource>) -> postcard::Result<Vec<u8>> {
    postcard::to_allocvec(resource)
}

/// Deserializes a resource written by [`resource_to_bytes()`].
pub fn resource_from_bytes(bytes: &[u8]) -> postcard::Result<Option<PreRenderedResource>> {
    postcard::from_bytes(bytes)
}

/// The native classes of the unit's items, by name.
///
/// A deserialized unit can't take them from the builtin elements, since building those
/// needs the whole compiler. So the unit carries each class it uses, once, with only the
/// type of each property. An item writes only the name of its class.
struct NativeClasses(BTreeMap<SmolStr, Arc<NativeClass>>);

impl NativeClasses {
    fn of(unit: &CompilationUnit) -> Self {
        let mut classes = BTreeMap::new();
        for item in unit.sub_components.iter().flat_map(|sc| sc.items.iter()) {
            let mut class = Some(&item.ty);
            while let Some(c) = class
                && classes.insert(c.class_name.clone(), c.clone()).is_none()
            {
                class = c.parent.as_ref();
            }
        }
        Self(classes)
    }
}

#[derive(Serialize, Deserialize)]
struct NativeClassRepr<'a> {
    class_name: &'a str,
    parent: Option<&'a str>,
    properties: Vec<(&'a str, crate::langtype::Type)>,
}

impl Serialize for NativeClasses {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // A class must come after its parent
        let mut ordered = self.0.values().collect::<Vec<_>>();
        ordered.sort_by_cached_key(|class| {
            core::iter::successors(class.parent.as_ref(), |parent| parent.parent.as_ref()).count()
        });
        serializer.collect_seq(ordered.into_iter().map(|class| NativeClassRepr {
            class_name: &class.class_name,
            parent: class.parent.as_ref().map(|p| p.class_name.as_str()),
            properties: class.properties.iter().map(|(k, v)| (k.as_str(), v.ty.clone())).collect(),
        }))
    }
}

impl<'de> Deserialize<'de> for NativeClasses {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut classes = BTreeMap::<SmolStr, Arc<NativeClass>>::new();
        for repr in Vec::<NativeClassRepr<'de>>::deserialize(deserializer)? {
            let parent = repr
                .parent
                .map(|parent| {
                    classes
                        .get(parent)
                        .cloned()
                        .ok_or_else(|| D::Error::custom(format!("no native class {parent}")))
                })
                .transpose()?;
            let class = NativeClass {
                parent,
                ..NativeClass::new_with_properties(
                    repr.class_name,
                    repr.properties
                        .into_iter()
                        .map(|(name, ty)| (name.into(), BuiltinPropertyInfo::new(ty))),
                )
            };
            classes.insert(class.class_name.clone(), Arc::new(class));
        }
        Ok(Self(classes))
    }
}

/// `#[serde(with = "native_class")]`: an item's class, written as its name.
///
/// It reads back as a class with only that name, which [`from_bytes()`] replaces.
pub(crate) mod native_class {
    use super::*;

    pub fn serialize<S: Serializer>(
        class: &Arc<NativeClass>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        class.class_name.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<NativeClass>, D::Error> {
        let class_name = SmolStr::deserialize(deserializer)?;
        Ok(Arc::new(NativeClass { class_name, ..Default::default() }))
    }
}

/// A value that the builtins provide, or any other one written by value.
#[derive(Serialize, Deserialize)]
enum Repr<Builtin, Other> {
    Builtin(Builtin),
    Other(Other),
}

/// `#[serde(with = "enumeration")]`: a builtin enum is written as its index in `BUILTIN.enums`,
/// so that a deserialized unit shares it with the compiler's.
pub(crate) mod enumeration {
    use super::*;

    pub fn serialize<S: Serializer>(
        enumeration: &Arc<Enumeration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match BUILTIN.enums.all().position(|builtin| Arc::ptr_eq(builtin, enumeration)) {
            Some(index) => Repr::<u32, &Enumeration>::Builtin(index as u32),
            None => Repr::Other(&**enumeration),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<Enumeration>, D::Error> {
        match Repr::<u32, Enumeration>::deserialize(deserializer)? {
            Repr::Builtin(index) => BUILTIN
                .enums
                .all()
                .nth(index as usize)
                .cloned()
                .ok_or_else(|| D::Error::custom(format!("no builtin enum {index}"))),
            Repr::Other(enumeration) => Ok(Arc::new(enumeration)),
        }
    }
}

/// `#[serde(with = "structure")]`: a builtin struct is written as its name,
/// and read back from [`builtin_structs::find()`].
/// A path element is written by value: its definition comes from the builtin elements.
pub(crate) mod structure {
    use super::*;

    pub fn serialize<S: Serializer>(s: &Arc<Struct>, serializer: S) -> Result<S::Ok, S::Error> {
        match &s.name {
            StructName::Builtin(name)
                if builtin_structs::find(name).is_some_and(|b| Arc::ptr_eq(s, &b)) =>
            {
                Repr::<_, &Struct>::Builtin(name.clone())
            }
            _ => Repr::Other(&**s),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<Struct>, D::Error> {
        match Repr::<crate::langtype::BuiltinStruct, Struct>::deserialize(deserializer)? {
            Repr::Builtin(name) => builtin_structs::find(&name)
                .ok_or_else(|| D::Error::custom(format!("no builtin struct {name:?}"))),
            Repr::Other(s) => Ok(Arc::new(s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llr::CompilationUnit;

    fn compile(source: &str, debug_info: bool) -> CompilationUnit {
        let mut config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        config.debug_info = debug_info;
        crate::llr::lower_test_source_with_config(source, config)
    }

    fn pretty(unit: &CompilationUnit) -> String {
        let mut out = String::new();
        crate::llr::pretty_print::pretty_print(unit, &mut out).unwrap();
        out
    }

    #[test]
    fn round_trip() {
        let source = r#"
            export enum Mode { a, b }
            export struct Point { x: int, y: int }
            export component Main inherits Window {
                in-out property <Mode> mode: Mode.b;
                in-out property <Point> point: { x: 1, y: 2 };
                in-out property <bool> is-b: mode == Mode.b;
                Rectangle { background: red; }
                Rectangle { background: point.x > 0 ? blue : green; }
                TouchArea { clicked => {
                    debug(mode);
                } }
            }
            // only-in-the-source
            "#;
        let unit = compile(source, true);

        let bytes = to_bytes(&unit).unwrap();
        let restored = from_bytes(&bytes).unwrap();
        // Everything but the debug info came back
        assert_eq!(pretty(&restored), pretty(&compile(source, false)));
        assert!(!restored.has_debug_info);
        assert!(restored.sub_components.iter().all(|sc| sc.debug_info.is_none()));
        // The builtins came back as the builtins, or they'd now be written in full
        assert_eq!(to_bytes(&restored).unwrap(), bytes);

        let items = |unit: &CompilationUnit| {
            unit.sub_components[unit.public_components.first().unwrap().item_tree.root]
                .items
                .clone()
        };
        let rectangles = items(&restored)
            .into_iter()
            .filter(|item| item.ty.class_name == "Rectangle")
            .map(|item| item.ty)
            .collect::<Vec<_>>();
        assert_eq!(rectangles.len(), 2);
        // The items share their class, which has the types of its properties
        assert!(Arc::ptr_eq(&rectangles[0], &rectangles[1]));
        let compiled = items(&unit).into_iter().find(|item| item.ty.class_name == "Rectangle");
        let compiled = compiled.unwrap().ty;
        assert_eq!(rectangles[0].properties.len(), compiled.properties.len());
        assert_eq!(
            rectangles[0].lookup_property("background"),
            Some(&crate::langtype::Type::Brush)
        );

        let contains = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(!contains(b"only-in-the-source"));
        assert!(!contains(b"t.slint"));
    }
}
