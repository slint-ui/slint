// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::{BTreeMap, BTreeSet};

use smol_str::SmolStr;

use super::{PyComponent, PyEnum, PyModule, PyProperty, PyStruct, PyStructField, PyStructOrEnum};

#[cfg(test)]
use super::PyEnumVariant;

impl PyModule {
    pub fn changed_globals(&self, other: &Self) -> Option<PyComponentsDifference> {
        PyComponentsDifference::compare(&self.globals, &other.globals)
    }

    pub fn changed_components(&self, other: &Self) -> Option<PyComponentsDifference> {
        PyComponentsDifference::compare(&self.components, &other.components)
    }

    pub fn changed_structs_or_enums(&self, other: &Self) -> Option<PyStructsOrEnumsDifference> {
        PyStructsOrEnumsDifference::compare(&self.structs_and_enums, &other.structs_and_enums)
    }
}

pub struct PyComponentsDifference {
    pub added_components: Vec<SmolStr>,
    pub removed_components: Vec<SmolStr>,
    pub changed_components: Vec<(SmolStr, ComponentDifference)>,
}

impl PyComponentsDifference {
    fn compare(orig: &[PyComponent], new: &[PyComponent]) -> Option<Self> {
        let orig_components = orig
            .iter()
            .map(|compo| (compo.name.as_str(), compo))
            .collect::<BTreeMap<&str, &PyComponent>>();

        let new_components = new
            .iter()
            .map(|compo| (compo.name.as_str(), compo))
            .collect::<BTreeMap<&str, &PyComponent>>();

        let added_components = new_components
            .iter()
            .filter_map(|(name, _)| {
                if orig_components.contains_key(name) { None } else { Some((*name).into()) }
            })
            .collect::<Vec<_>>();

        let removed_components =
            orig_components
                .iter()
                .filter_map(|(name, _)| {
                    if new_components.contains_key(name) { None } else { Some((*name).into()) }
                })
                .collect::<Vec<_>>();

        let changed_components = orig_components
            .iter()
            .filter_map(|(name, orig_global)| {
                let new_glob = new_components.get(name)?;

                let diff = ComponentDifference::compare(&orig_global, &new_glob);

                diff.map(|diff| ((*name).into(), diff))
            })
            .collect::<Vec<_>>();

        if !added_components.is_empty()
            || !removed_components.is_empty()
            || !changed_components.is_empty()
        {
            Some(PyComponentsDifference {
                added_components,
                removed_components,
                changed_components,
            })
        } else {
            None
        }
    }

    pub fn incompatible_changes(&self) -> bool {
        !self.removed_components.is_empty()
            || self.changed_components.iter().any(|(_, change)| change.incompatible_changes())
    }
}

#[derive(PartialEq, Debug)]
pub struct TypeChange {
    pub name: SmolStr,
    pub old_type: SmolStr,
    pub new_type: SmolStr,
}

#[derive(PartialEq, Debug)]
pub struct ComponentDifference {
    // TODO: represent callbacks and functions differently?
    pub added_properties: Vec<PyProperty>,
    pub removed_properties: Vec<PyProperty>,
    pub type_changed_properties: Vec<TypeChange>,
    pub added_aliases: Vec<SmolStr>,
    pub removed_aliases: Vec<SmolStr>,
}

impl ComponentDifference {
    fn compare(old_compo: &PyComponent, new_compo: &PyComponent) -> Option<Self> {
        let orig_props = old_compo
            .properties
            .iter()
            .map(|p| (p.name.as_str(), p))
            .collect::<BTreeMap<&str, &PyProperty>>();
        let new_props = new_compo
            .properties
            .iter()
            .map(|p| (p.name.as_str(), p))
            .collect::<BTreeMap<&str, &PyProperty>>();

        let added_properties = new_props
            .iter()
            .filter_map(|(name, new_prop)| {
                if orig_props.contains_key(name) { None } else { Some((*new_prop).clone()) }
            })
            .collect::<Vec<_>>();

        let removed_properties =
            orig_props
                .iter()
                .filter_map(|(name, old_prop)| {
                    if new_props.contains_key(name) { None } else { Some((*old_prop).clone()) }
                })
                .collect::<Vec<_>>();

        let type_changed_properties = orig_props
            .iter()
            .filter_map(|(name, orig_prop)| {
                let new_prop = new_props.get(name)?;

                if orig_prop.ty != new_prop.ty {
                    Some(TypeChange {
                        name: (*name).into(),
                        old_type: orig_prop.ty.clone(),
                        new_type: new_prop.ty.clone(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let old_aliases = old_compo.aliases.iter().collect::<BTreeSet<_>>();
        let new_aliases = new_compo.aliases.iter().collect::<BTreeSet<_>>();

        let added_aliases =
            new_aliases.difference(&old_aliases).map(|s| (*s).clone()).collect::<Vec<_>>();
        let removed_aliases =
            old_aliases.difference(&new_aliases).map(|s| (*s).clone()).collect::<Vec<_>>();

        let diff = Self {
            added_properties,
            removed_properties,
            type_changed_properties,
            added_aliases,
            removed_aliases,
        };
        if diff.has_difference() { Some(diff) } else { None }
    }

    fn has_difference(&self) -> bool {
        !self.added_properties.is_empty()
            || !self.removed_properties.is_empty()
            || !self.type_changed_properties.is_empty()
            || !self.added_aliases.is_empty()
            || !self.removed_aliases.is_empty()
    }

    fn incompatible_changes(&self) -> bool {
        !self.removed_properties.is_empty()
            || !self.type_changed_properties.is_empty()
            || !self.removed_aliases.is_empty()
    }
}

pub struct PyStructsOrEnumsDifference {
    pub added_structs: Vec<SmolStr>,
    pub removed_structs: Vec<SmolStr>,
    pub changed_structs: Vec<(SmolStr, StructDifference)>,
    pub added_enums: Vec<SmolStr>,
    pub removed_enums: Vec<SmolStr>,
    pub changed_enums: Vec<(SmolStr, EnumDifference)>,
}

impl PyStructsOrEnumsDifference {
    fn compare(orig: &[PyStructOrEnum], new: &[PyStructOrEnum]) -> Option<Self> {
        let mut orig_structs = BTreeMap::new();
        let mut orig_enums = BTreeMap::new();
        for struct_or_enum in orig {
            match struct_or_enum {
                PyStructOrEnum::Struct(py_struct) => {
                    orig_structs.insert(py_struct.name.as_str(), py_struct);
                }
                PyStructOrEnum::Enum(py_enum) => {
                    orig_enums.insert(py_enum.name.as_str(), py_enum);
                }
            }
        }

        let mut new_structs = BTreeMap::new();
        let mut new_enums = BTreeMap::new();
        for struct_or_enum in new {
            match struct_or_enum {
                PyStructOrEnum::Struct(py_struct) => {
                    new_structs.insert(py_struct.name.as_str(), py_struct);
                }
                PyStructOrEnum::Enum(py_enum) => {
                    new_enums.insert(py_enum.name.as_str(), py_enum);
                }
            }
        }

        let added_structs =
            new_structs
                .iter()
                .filter_map(|(name, _)| {
                    if orig_structs.contains_key(name) { None } else { Some((*name).into()) }
                })
                .collect::<Vec<_>>();

        let added_enums = new_enums
            .iter()
            .filter_map(
                |(name, _)| {
                    if orig_enums.contains_key(name) { None } else { Some((*name).into()) }
                },
            )
            .collect::<Vec<_>>();

        let removed_structs =
            orig_structs
                .iter()
                .filter_map(|(name, _)| {
                    if new_structs.contains_key(name) { None } else { Some((*name).into()) }
                })
                .collect::<Vec<_>>();

        let removed_enums = orig_enums
            .iter()
            .filter_map(
                |(name, _)| {
                    if new_enums.contains_key(name) { None } else { Some((*name).into()) }
                },
            )
            .collect::<Vec<_>>();

        let changed_structs = orig_structs
            .iter()
            .filter_map(|(name, orig_struct)| {
                let new_struct = new_structs.get(name)?;

                let diff = StructDifference::compare(&orig_struct, &new_struct);

                diff.map(|diff| ((*name).into(), diff))
            })
            .collect::<Vec<_>>();

        let changed_enums = orig_enums
            .iter()
            .filter_map(|(name, orig_enum)| {
                let new_enum = new_enums.get(name)?;

                let diff = EnumDifference::compare(&orig_enum, &new_enum);

                diff.map(|diff| ((*name).into(), diff))
            })
            .collect::<Vec<_>>();

        if !added_structs.is_empty()
            || !removed_structs.is_empty()
            || !changed_structs.is_empty()
            || !added_enums.is_empty()
            || !removed_enums.is_empty()
            || !changed_enums.is_empty()
        {
            Some(Self {
                added_structs,
                removed_structs,
                changed_structs,
                added_enums,
                removed_enums,
                changed_enums,
            })
        } else {
            None
        }
    }

    pub fn incompatible_changes(&self) -> bool {
        !self.removed_structs.is_empty()
            || !self.removed_enums.is_empty()
            || self.changed_structs.iter().any(|(_, c)| c.incompatible_changes())
            || self.changed_enums.iter().any(|(_, c)| c.incompatible_changes())
    }
}

#[derive(PartialEq, Debug)]
pub struct StructDifference {
    pub added_fields: Vec<PyStructField>,
    pub removed_fields: Vec<PyStructField>,
    pub type_changed_fields: Vec<TypeChange>,
    pub added_aliases: Vec<SmolStr>,
    pub removed_aliases: Vec<SmolStr>,
}

impl StructDifference {
    fn compare(old_struct: &PyStruct, new_struct: &PyStruct) -> Option<Self> {
        let orig_fields = old_struct
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f))
            .collect::<BTreeMap<&str, &PyStructField>>();
        let new_fields = new_struct
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f))
            .collect::<BTreeMap<&str, &PyStructField>>();

        let added_fields = new_fields
            .iter()
            .filter_map(|(name, new_field)| {
                if orig_fields.contains_key(name) { None } else { Some((*new_field).clone()) }
            })
            .collect::<Vec<_>>();

        let removed_fields = orig_fields
            .iter()
            .filter_map(|(name, old_field)| {
                if new_fields.contains_key(name) { None } else { Some((*old_field).clone()) }
            })
            .collect::<Vec<_>>();

        let type_changed_fields = orig_fields
            .iter()
            .filter_map(|(name, orig_field)| {
                let new_field = new_fields.get(name)?;

                if orig_field.ty != new_field.ty {
                    Some(TypeChange {
                        name: (*name).into(),
                        old_type: orig_field.ty.clone(),
                        new_type: new_field.ty.clone(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let old_aliases = old_struct.aliases.iter().collect::<BTreeSet<_>>();
        let new_aliases = new_struct.aliases.iter().collect::<BTreeSet<_>>();

        let added_aliases =
            new_aliases.difference(&old_aliases).map(|s| (*s).clone()).collect::<Vec<_>>();
        let removed_aliases =
            old_aliases.difference(&new_aliases).map(|s| (*s).clone()).collect::<Vec<_>>();

        let diff = Self {
            added_fields,
            removed_fields,
            type_changed_fields,
            added_aliases,
            removed_aliases,
        };
        if diff.has_difference() { Some(diff) } else { None }
    }

    fn has_difference(&self) -> bool {
        !self.added_fields.is_empty()
            || !self.removed_fields.is_empty()
            || !self.type_changed_fields.is_empty()
            || !self.added_aliases.is_empty()
            || !self.removed_aliases.is_empty()
    }

    fn incompatible_changes(&self) -> bool {
        !self.removed_fields.is_empty()
            || !self.removed_aliases.is_empty()
            || !self.type_changed_fields.is_empty()
    }
}

#[derive(Debug, PartialEq)]
pub struct EnumDifference {
    pub added_variants: Vec<SmolStr>,
    pub removed_variants: Vec<SmolStr>,
    pub added_aliases: Vec<SmolStr>,
    pub removed_aliases: Vec<SmolStr>,
}

impl EnumDifference {
    fn compare(old_enum: &PyEnum, new_enum: &PyEnum) -> Option<Self> {
        let old_variants = old_enum.variants.iter().map(|v| &v.name).collect::<BTreeSet<_>>();
        let new_variants = new_enum.variants.iter().map(|v| &v.name).collect::<BTreeSet<_>>();

        let added_variants =
            new_variants.difference(&old_variants).map(|s| (*s).clone()).collect::<Vec<_>>();
        let removed_variants =
            old_variants.difference(&new_variants).map(|s| (*s).clone()).collect::<Vec<_>>();

        let old_aliases = old_enum.aliases.iter().collect::<BTreeSet<_>>();
        let new_aliases = new_enum.aliases.iter().collect::<BTreeSet<_>>();

        let added_aliases =
            new_aliases.difference(&old_aliases).map(|s| (*s).clone()).collect::<Vec<_>>();
        let removed_aliases =
            old_aliases.difference(&new_aliases).map(|s| (*s).clone()).collect::<Vec<_>>();

        let diff = Self { added_variants, removed_variants, added_aliases, removed_aliases };
        if diff.has_difference() { Some(diff) } else { None }
    }

    fn has_difference(&self) -> bool {
        !self.added_variants.is_empty()
            || !self.removed_variants.is_empty()
            || !self.added_aliases.is_empty()
            || !self.removed_aliases.is_empty()
    }

    fn incompatible_changes(&self) -> bool {
        !self.removed_variants.is_empty() || !self.removed_aliases.is_empty()
    }
}

#[test]
fn globals() {
    let old = super::PyModule {
        globals: vec![
            PyComponent {
                name: SmolStr::new_static("SameGlobal"),
                properties: vec![PyProperty {
                    name: SmolStr::new_static("str_prop"),
                    ty: SmolStr::new_static("str"),
                }],
                aliases: vec![SmolStr::new_static("SameGlobalAlias")],
            },
            PyComponent {
                name: SmolStr::new_static("ChangedGlobal"),
                properties: vec![
                    PyProperty {
                        name: SmolStr::new_static("same_str_prop"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyProperty {
                        name: SmolStr::new_static("change_to_int_prop"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyProperty {
                        name: SmolStr::new_static("removed_prop"),
                        ty: SmolStr::new_static("int"),
                    },
                ],
                aliases: vec![SmolStr::new_static("ChangedGlobalAlias")],
            },
            PyComponent {
                name: SmolStr::new_static("ToBeRemoved"),
                properties: Vec::new(),
                aliases: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let new = super::PyModule {
        globals: vec![
            PyComponent {
                name: SmolStr::new_static("SameGlobal"),
                properties: vec![PyProperty {
                    name: SmolStr::new_static("str_prop"),
                    ty: SmolStr::new_static("str"),
                }],
                aliases: vec![SmolStr::new_static("SameGlobalAlias")],
            },
            PyComponent {
                name: SmolStr::new_static("ChangedGlobal"),
                properties: vec![
                    PyProperty {
                        name: SmolStr::new_static("same_str_prop"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyProperty {
                        name: SmolStr::new_static("change_to_int_prop"),
                        ty: SmolStr::new_static("int"),
                    },
                    PyProperty {
                        name: SmolStr::new_static("new_prop"),
                        ty: SmolStr::new_static("float"),
                    },
                ],
                aliases: vec![SmolStr::new_static("NewGlobalAlias")],
            },
            PyComponent {
                name: SmolStr::new_static("NewGlobal"),
                properties: vec![PyProperty {
                    name: SmolStr::new_static("str_prop"),
                    ty: SmolStr::new_static("str"),
                }],
                aliases: Vec::new(),
            },
        ],
        ..Default::default()
    };

    assert!(old.changed_globals(&old).is_none());

    let changed = old.changed_globals(&new);
    assert!(changed.is_some());
    let changed = changed.unwrap();

    assert_eq!(changed.added_components, vec![SmolStr::new_static("NewGlobal")]);
    assert_eq!(changed.removed_components, vec![SmolStr::new_static("ToBeRemoved")]);

    let expected_glob_change = ComponentDifference {
        added_properties: vec![PyProperty {
            name: SmolStr::new_static("new_prop"),
            ty: SmolStr::new_static("float"),
        }],
        removed_properties: vec![PyProperty {
            name: SmolStr::new_static("removed_prop"),
            ty: SmolStr::new_static("int"),
        }],
        type_changed_properties: vec![TypeChange {
            name: SmolStr::new_static("change_to_int_prop"),
            old_type: SmolStr::new_static("str"),
            new_type: SmolStr::new_static("int"),
        }],
        added_aliases: vec![SmolStr::new_static("NewGlobalAlias")],
        removed_aliases: vec![SmolStr::new_static("ChangedGlobalAlias")],
    };

    assert_eq!(
        changed.changed_components,
        vec![(SmolStr::new_static("ChangedGlobal"), expected_glob_change)]
    );
}

#[test]
fn structs_and_enums() {
    let old = super::PyModule {
        structs_and_enums: vec![
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("SameStruct"),
                fields: vec![PyStructField {
                    name: SmolStr::new_static("intfield"),
                    ty: SmolStr::new_static("int"),
                }],
                aliases: vec![SmolStr::new_static("SameStructalias")],
            }),
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("StructWithChangedFields"),
                fields: vec![
                    PyStructField {
                        name: SmolStr::new_static("removed_field"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyStructField {
                        name: SmolStr::new_static("unchanged_field"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyStructField {
                        name: SmolStr::new_static("to_int_field"),
                        ty: SmolStr::new_static("float"),
                    },
                ],
                aliases: vec![SmolStr::new_static("RemovedAlias")],
            }),
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("RemovedStruct"),
                fields: Vec::new(),
                aliases: Vec::new(),
            }),
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("StructBecomesEnum"),
                fields: Vec::new(),
                aliases: Vec::new(),
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("SameEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant1"),
                        strvalue: SmolStr::new_static("Variant1"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant2"),
                        strvalue: SmolStr::new_static("Variant2"),
                    },
                ],
                aliases: vec![SmolStr::new_static("SameEnumAlias")],
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("ChangedEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant1"),
                        strvalue: SmolStr::new_static("Variant1"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant2"),
                        strvalue: SmolStr::new_static("Variant2"),
                    },
                ],
                aliases: vec![SmolStr::new_static("ChangedEnumRemovedAlias")],
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("RemovedEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant1"),
                        strvalue: SmolStr::new_static("Variant1"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant2"),
                        strvalue: SmolStr::new_static("Variant2"),
                    },
                ],
                aliases: Vec::new(),
            }),
        ],
        ..Default::default()
    };

    let new = super::PyModule {
        structs_and_enums: vec![
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("SameStruct"),
                fields: vec![PyStructField {
                    name: SmolStr::new_static("intfield"),
                    ty: SmolStr::new_static("int"),
                }],
                aliases: vec![SmolStr::new_static("SameStructalias")],
            }),
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("StructWithChangedFields"),
                fields: vec![
                    PyStructField {
                        name: SmolStr::new_static("added_field"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyStructField {
                        name: SmolStr::new_static("unchanged_field"),
                        ty: SmolStr::new_static("str"),
                    },
                    PyStructField {
                        name: SmolStr::new_static("to_int_field"),
                        ty: SmolStr::new_static("int"),
                    },
                ],
                aliases: vec![SmolStr::new_static("NewAlias")],
            }),
            PyStructOrEnum::Struct(PyStruct {
                name: SmolStr::new_static("AddedStruct"),
                fields: Vec::new(),
                aliases: Vec::new(),
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("StructBecomesEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant1"),
                        strvalue: SmolStr::new_static("Variant1"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant2"),
                        strvalue: SmolStr::new_static("Variant2"),
                    },
                ],
                aliases: Vec::new(),
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("SameEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant1"),
                        strvalue: SmolStr::new_static("Variant1"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant2"),
                        strvalue: SmolStr::new_static("Variant2"),
                    },
                ],
                aliases: vec![SmolStr::new_static("SameEnumAlias")],
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("ChangedEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant3"),
                        strvalue: SmolStr::new_static("Variant3"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant4"),
                        strvalue: SmolStr::new_static("Variant4"),
                    },
                ],
                aliases: vec![SmolStr::new_static("ChangedEnumAddedAlias")],
            }),
            PyStructOrEnum::Enum(PyEnum {
                name: SmolStr::new_static("AddedEnum"),
                variants: vec![
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant1"),
                        strvalue: SmolStr::new_static("Variant1"),
                    },
                    PyEnumVariant {
                        name: SmolStr::new_static("Variant2"),
                        strvalue: SmolStr::new_static("Variant2"),
                    },
                ],
                aliases: Vec::new(),
            }),
        ],
        ..Default::default()
    };

    assert!(old.changed_structs_or_enums(&old).is_none());

    let changed = old.changed_structs_or_enums(&new);
    assert!(changed.is_some());
    let changed = changed.unwrap();

    assert_eq!(changed.added_structs, vec![SmolStr::new_static("AddedStruct")]);
    assert_eq!(
        changed.removed_structs,
        vec![SmolStr::new_static("RemovedStruct"), SmolStr::new_static("StructBecomesEnum")]
    );

    assert_eq!(
        changed.added_enums,
        vec![SmolStr::new_static("AddedEnum"), SmolStr::new_static("StructBecomesEnum")]
    );
    assert_eq!(changed.removed_enums, vec![SmolStr::new_static("RemovedEnum")]);

    let expected_struct_change = StructDifference {
        added_fields: vec![PyStructField {
            name: SmolStr::new_static("added_field"),
            ty: SmolStr::new_static("str"),
        }],
        removed_fields: vec![PyStructField {
            name: SmolStr::new_static("removed_field"),
            ty: SmolStr::new_static("str"),
        }],
        type_changed_fields: vec![TypeChange {
            name: SmolStr::new_static("to_int_field"),
            old_type: SmolStr::new_static("float"),
            new_type: SmolStr::new_static("int"),
        }],
        added_aliases: vec![SmolStr::new_static("NewAlias")],
        removed_aliases: vec![SmolStr::new_static("RemovedAlias")],
    };

    assert_eq!(
        changed.changed_structs,
        vec![(SmolStr::new_static("StructWithChangedFields"), expected_struct_change)]
    );

    let expected_enum_change = EnumDifference {
        added_variants: vec![SmolStr::new_static("Variant3"), SmolStr::new_static("Variant4")],
        removed_variants: vec![SmolStr::new_static("Variant1"), SmolStr::new_static("Variant2")],
        added_aliases: vec![SmolStr::new_static("ChangedEnumAddedAlias")],
        removed_aliases: vec![SmolStr::new_static("ChangedEnumRemovedAlias")],
    };

    assert_eq!(
        changed.changed_enums,
        vec![(SmolStr::new_static("ChangedEnum"), expected_enum_change)]
    );
}
