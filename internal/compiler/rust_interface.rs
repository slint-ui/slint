// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Describes the Rust API generated from a Slint document.

use crate::langtype::{StructName, Type};
use crate::llr::{CompilationUnit, PublicProperties, TopLevelComponentType};
use crate::object_tree::{Document, PropertyVisibility};
use itertools::Either;
use std::collections::BTreeMap;
use std::fmt::{Display, Write as _};

const DESCRIPTOR_VERSION: &[u8] = b"slint-rust-interface-v1\0";

/// One canonical entry in a generated Rust interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustInterfaceEntry {
    path: String,
    signature: String,
}

impl RustInterfaceEntry {
    /// Create an interface entry.
    pub fn new(path: impl Into<String>, signature: impl Into<String>) -> Self {
        Self { path: path.into(), signature: signature.into() }
    }

    /// Return the stable path that identifies this part of the interface.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Return the canonical signature for this part of the interface.
    pub fn signature(&self) -> &str {
        &self.signature
    }
}

/// The generated Rust interface of a compiled Slint document.
///
/// Entries are sorted by path and contain only Rust-visible declarations.
/// Layouts, bindings, and other implementation details don't affect this descriptor.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustInterfaceDescriptor {
    entries: Vec<RustInterfaceEntry>,
}

impl RustInterfaceDescriptor {
    /// Describe the Rust API generated for `document` and its lowered compilation unit.
    pub fn from_document(document: &Document, unit: &CompilationUnit) -> Self {
        let mut entries = Vec::new();

        for component in &unit.public_components {
            let component_path = format!("component {}", component.name);
            let component_kind = match component.top_level_type {
                TopLevelComponentType::Window => "window",
                TopLevelComponentType::SystemTrayIcon => "system-tray-icon",
            };
            entries.push(RustInterfaceEntry::new(&component_path, component_kind));
            describe_members(&component_path, &component.public_properties, &mut entries);
        }

        for global in unit.globals.iter().filter(|global| global.exported) {
            let global_path = format!("global {}", global.name);
            entries.push(RustInterfaceEntry::new(&global_path, "global"));
            describe_members(&global_path, &global.public_properties, &mut entries);
        }

        for ty in &document.used_types.borrow().structs_and_enums {
            match ty {
                Type::Struct(the_struct) => {
                    let StructName::User { name, .. } = &the_struct.name else { continue };
                    let struct_path = format!("struct {name}");
                    entries.push(RustInterfaceEntry::new(&struct_path, "struct"));
                    for (field_name, field_type) in &the_struct.fields {
                        entries.push(RustInterfaceEntry::new(
                            format!("{struct_path}/field {field_name}"),
                            type_signature(field_type),
                        ));
                    }
                }
                Type::Enumeration(enumeration) => {
                    let enum_path = format!("enum {}", enumeration.name);
                    entries.push(RustInterfaceEntry::new(&enum_path, "enum"));
                    let mut variants = enumeration
                        .values
                        .iter()
                        .enumerate()
                        .map(|(index, name)| (name, index, index == enumeration.default_value))
                        .collect::<Vec<_>>();
                    variants.sort_by(|left, right| left.0.cmp(right.0));
                    for (name, index, is_default) in variants {
                        entries.push(RustInterfaceEntry::new(
                            format!("{enum_path}/variant {name}"),
                            format!("index {index}{}", if is_default { ", default" } else { "" }),
                        ));
                    }
                }
                _ => {}
            }
        }

        let mut named_exports = BTreeMap::<String, String>::new();
        for global in unit.globals.iter().filter(|global| global.exported) {
            for alias in &global.aliases {
                if alias != &global.name {
                    named_exports.insert(alias.to_string(), format!("global {}", global.name));
                }
            }
        }
        for (exported_name, target) in document.exports.iter() {
            let target = match target {
                Either::Left(component) => {
                    let kind = if component.is_global() { "global" } else { "component" };
                    Some((kind, component.id.as_str()))
                }
                Either::Right(Type::Struct(the_struct)) => match &the_struct.name {
                    StructName::User { name, .. } => Some(("struct", name.as_str())),
                    _ => None,
                },
                Either::Right(Type::Enumeration(enumeration)) => {
                    Some(("enum", enumeration.name.as_str()))
                }
                Either::Right(_) => None,
            };
            let Some((kind, target_name)) = target else { continue };
            if exported_name.name.as_str() != target_name {
                named_exports
                    .insert(exported_name.name.to_string(), format!("{kind} {target_name}"));
            }
        }
        entries.extend(
            named_exports.into_iter().map(|(name, target)| {
                RustInterfaceEntry::new(format!("named export {name}"), target)
            }),
        );

        Self::from_entries(entries)
    }

    /// Create a descriptor from canonical entries.
    ///
    /// This supports embedding a compiled descriptor in generated Rust code.
    pub fn from_entries(entries: impl IntoIterator<Item = RustInterfaceEntry>) -> Self {
        let mut entries_by_path = BTreeMap::new();
        for entry in entries {
            if let Some(previous) = entries_by_path.insert(entry.path, entry.signature.clone()) {
                assert_eq!(previous, entry.signature, "conflicting Rust interface entries");
            }
        }
        Self {
            entries: entries_by_path
                .into_iter()
                .map(|(path, signature)| RustInterfaceEntry { path, signature })
                .collect(),
        }
    }

    /// Return the sorted canonical entries.
    pub fn entries(&self) -> &[RustInterfaceEntry] {
        &self.entries
    }

    /// Calculate a stable BLAKE3 fingerprint of this descriptor.
    pub fn fingerprint(&self) -> RustInterfaceFingerprint {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DESCRIPTOR_VERSION);
        for entry in &self.entries {
            hash_field(&mut hasher, entry.path.as_bytes());
            hash_field(&mut hasher, entry.signature.as_bytes());
        }
        RustInterfaceFingerprint(*hasher.finalize().as_bytes())
    }

    /// Compare this compiled descriptor with a candidate descriptor.
    pub fn diff(&self, candidate: &Self) -> RustInterfaceDiff {
        let current = self
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.signature.as_str()))
            .collect::<BTreeMap<_, _>>();
        let candidate = candidate
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.signature.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut changes = Vec::new();

        for (path, current_signature) in &current {
            match candidate.get(path) {
                None => changes.push(RustInterfaceChange::Removed {
                    path: (*path).to_string(),
                    signature: (*current_signature).to_string(),
                }),
                Some(candidate_signature) if candidate_signature != current_signature => {
                    changes.push(RustInterfaceChange::Changed {
                        path: (*path).to_string(),
                        current: (*current_signature).to_string(),
                        candidate: (*candidate_signature).to_string(),
                    });
                }
                Some(_) => {}
            }
        }
        for (path, candidate_signature) in &candidate {
            if !current.contains_key(path) {
                changes.push(RustInterfaceChange::Added {
                    path: (*path).to_string(),
                    signature: (*candidate_signature).to_string(),
                });
            }
        }
        changes.sort_by(|left, right| left.path().cmp(right.path()));
        RustInterfaceDiff { changes }
    }
}

fn describe_members(
    container_path: &str,
    properties: &PublicProperties,
    entries: &mut Vec<RustInterfaceEntry>,
) {
    for (name, property) in properties {
        let member_path = match &property.ty {
            Type::Callback(_) => format!("{container_path}/callback {name}"),
            Type::Function(_) => format!("{container_path}/function {name}"),
            _ => format!("{container_path}/property {name}"),
        };
        let signature = match &property.ty {
            Type::Callback(function) | Type::Function(function) => function_signature(function),
            ty => format!("{} {}", direction_signature(property.visibility), type_signature(ty)),
        };
        entries.push(RustInterfaceEntry::new(member_path, signature));
    }
}

fn direction_signature(visibility: PropertyVisibility) -> &'static str {
    match visibility {
        PropertyVisibility::Private => "private",
        PropertyVisibility::Input => "in",
        PropertyVisibility::Output => "out",
        PropertyVisibility::InOut => "in-out",
        PropertyVisibility::Constexpr => "constexpr",
        PropertyVisibility::Fake => "fake",
        PropertyVisibility::Public => "public",
        PropertyVisibility::Protected => "protected",
    }
}

fn function_signature(function: &crate::langtype::Function) -> String {
    let arguments = function.args.iter().map(type_signature).collect::<Vec<_>>().join(", ");
    format!("({arguments}) -> {}", type_signature(&function.return_type))
}

fn type_signature(ty: &Type) -> String {
    match ty {
        Type::Invalid => "invalid".into(),
        Type::Void => "void".into(),
        Type::InferredProperty => "inferred-property".into(),
        Type::InferredCallback => "inferred-callback".into(),
        Type::Callback(function) => format!("callback {}", function_signature(function)),
        Type::Function(function) => format!("function {}", function_signature(function)),
        Type::ComponentFactory => "component-factory".into(),
        Type::Float32 => "float".into(),
        Type::Int32 => "int".into(),
        Type::String => "string".into(),
        Type::Color => "color".into(),
        Type::Duration => "duration".into(),
        Type::PhysicalLength => "physical-length".into(),
        Type::LogicalLength => "length".into(),
        Type::Rem => "relative-font-size".into(),
        Type::Angle => "angle".into(),
        Type::Percent => "percent".into(),
        Type::Image => "image".into(),
        Type::Bool => "bool".into(),
        Type::Model => "model".into(),
        Type::PathData => "path-data".into(),
        Type::Easing => "easing".into(),
        Type::Brush => "brush".into(),
        Type::Array(element) => format!("[{}]", type_signature(element)),
        Type::Struct(the_struct) => match &the_struct.name {
            StructName::None => {
                let fields = the_struct
                    .fields
                    .iter()
                    .map(|(name, ty)| format!("{name}: {}", type_signature(ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {fields} }}")
            }
            StructName::User { name, .. } => format!("struct {name}"),
            StructName::Builtin(builtin) => format!(
                "builtin {}",
                builtin
                    .slint_name()
                    .map_or_else(|| format!("{builtin:?}"), |name| name.to_string())
            ),
        },
        Type::Enumeration(enumeration) => format!("enum {}", enumeration.name),
        Type::Keys => "keys".into(),
        Type::DataTransfer => "data-transfer".into(),
        Type::UnitProduct(units) => {
            let units = units
                .iter()
                .map(|(unit, power)| format!("{unit}^{power}"))
                .collect::<Vec<_>>()
                .join(" * ");
            format!("unit-product({units})")
        }
        Type::ElementReference => "element-reference".into(),
        Type::LayoutCache => "layout-cache".into(),
        Type::ArrayOfU16 => "[u16]".into(),
        Type::StyledText => "styled-text".into(),
        Type::MouseCursor => "mouse-cursor".into(),
        Type::Closure => "closure".into(),
    }
}

fn hash_field(hasher: &mut blake3::Hasher, field: &[u8]) {
    hasher.update(&(field.len() as u64).to_le_bytes());
    hasher.update(field);
}

/// A stable BLAKE3 fingerprint of a generated Rust interface.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct RustInterfaceFingerprint([u8; 32]);

impl RustInterfaceFingerprint {
    /// Return the raw BLAKE3 digest.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Display for RustInterfaceFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for RustInterfaceFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, formatter)
    }
}

/// A human-readable difference between compiled and candidate Rust interfaces.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustInterfaceDiff {
    changes: Vec<RustInterfaceChange>,
}

impl RustInterfaceDiff {
    /// Return whether the interfaces match exactly.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

impl Display for RustInterfaceDiff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, change) in self.changes.iter().enumerate() {
            if index > 0 {
                formatter.write_char('\n')?;
            }
            match change {
                RustInterfaceChange::Added { path, signature } => {
                    write!(formatter, "+ {path}: {signature}")
                }
                RustInterfaceChange::Removed { path, signature } => {
                    write!(formatter, "- {path}: {signature}")
                }
                RustInterfaceChange::Changed { path, current, candidate } => {
                    write!(formatter, "~ {path}: {current} -> {candidate}")
                }
            }?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RustInterfaceChange {
    Added { path: String, signature: String },
    Removed { path: String, signature: String },
    Changed { path: String, current: String, candidate: String },
}

impl RustInterfaceChange {
    fn path(&self) -> &str {
        match self {
            Self::Added { path, .. } | Self::Removed { path, .. } | Self::Changed { path, .. } => {
                path
            }
        }
    }
}
