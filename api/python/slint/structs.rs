// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString, PyTuple};

use crate::value::TypeCollection;

#[pyclass(unsendable, module = "slint.core")]
pub struct StructFactory {
    name: &'static str,
    exported_fields: Vec<&'static str>,
    default_data: slint_interpreter::Struct,
    type_collection: TypeCollection,
}

impl StructFactory {
    fn new(
        name: &'static str,
        exported_fields: Vec<&'static str>,
        default_data: slint_interpreter::Struct,
        type_collection: TypeCollection,
    ) -> Self {
        Self { name, exported_fields, default_data, type_collection }
    }
}

#[pymethods]
impl StructFactory {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<crate::value::PyStruct>> {
        if args.len() != 0 {
            return Err(PyTypeError::new_err(format!(
                "{}() accepts keyword arguments only",
                self.name
            )));
        }

        let pystruct = Py::new(
            py,
            crate::value::PyStruct {
                data: self.default_data.clone(),
                type_collection: self.type_collection.clone(),
            },
        )?;

        if let Some(kwargs) = kwargs {
            let instance = pystruct.bind(py);
            for (key_obj, value) in kwargs.iter() {
                let key = key_obj.downcast::<PyString>().map_err(|_| {
                    PyTypeError::new_err(format!(
                        "{}() keyword arguments must be strings",
                        self.name
                    ))
                })?;
                let key_str = key.to_str()?;
                if !self.exported_fields.iter().any(|field| *field == key_str) {
                    return Err(PyTypeError::new_err(format!(
                        "{}() got an unexpected keyword argument '{}'",
                        self.name, key_str
                    )));
                }
                instance.setattr(key_str, value)?;
            }
        }

        Ok(pystruct)
    }

    #[getter]
    fn __name__(&self) -> &str {
        self.name
    }
}

macro_rules! generate_struct_support {
    ($(
        $(#[$struct_attr:meta])*
        struct $Name:ident {
            @name = $inner_name:literal
            export {
                $( $(#[$pub_attr:meta])* $pub_field:ident : $pub_type:ty, )*
            }
            private {
                $( $(#[$pri_attr:meta])* $pri_field:ident : $pri_type:ty, )*
            }
        }
    )*) => {
        #[cfg(feature = "stubgen")]
        pub(super) mod stub_structs {
            use pyo3_stub_gen::{
                inventory,
                type_info::{
                    MemberInfo, MethodInfo, MethodType, ParameterDefault, ParameterInfo,
                    ParameterKind, PyClassInfo, PyMethodsInfo, DeprecatedInfo,
                },
                TypeInfo,
            };

            const EMPTY_DOC: &str = "";
            const NO_DEFAULT: Option<fn() -> String> = None;
            const NO_DEPRECATED: Option<DeprecatedInfo> = None;

            macro_rules! field_type_info {
                (bool) => { || <bool as PyStubType>::type_output() };
                (f32) => { || <f32 as PyStubType>::type_output() };
                (f64) => { || <f64 as PyStubType>::type_output() };
                (i16) => { || <i16 as PyStubType>::type_output() };
                (i32) => { || <i32 as PyStubType>::type_output() };
                (u32) => { || <u32 as PyStubType>::type_output() };
                (SharedString) => { || <SharedString as PyStubType>::type_output() };
                (String) => { || <String as PyStubType>::type_output() };
                (Coord) => { || TypeInfo::builtin("float") };
                (PointerEventButton) => { || TypeInfo::unqualified("PointerEventButton") };
                (PointerEventKind) => { || TypeInfo::unqualified("PointerEventKind") };
                (KeyboardModifiers) => { || TypeInfo::unqualified("KeyboardModifiers") };
                (SortOrder) => { || TypeInfo::unqualified("SortOrder") };
                (MenuEntry) => { || TypeInfo::unqualified("MenuEntry") };
                (LogicalPosition) => {
                    || TypeInfo::with_module("typing.Tuple[float, float]", "typing".into())
                };
                (Image) => {
                    || TypeInfo::with_module("slint.Image", "slint".into())
                };
                ($other:ty) => { || TypeInfo::with_module("typing.Any", "typing".into()) };
            }
            fn ellipsis_default() -> String {
                "...".to_string()
            }

            mod markers {
                $(
                    pub struct $Name;
                )*
            }

            $(
                inventory::submit! {
                    PyClassInfo {
                        pyclass_name: stringify!($Name),
                        struct_id: || ::std::any::TypeId::of::<markers::$Name>(),
                        module: Some("slint.core"),
                        doc: EMPTY_DOC,
                        getters: &[
                            $(
                                MemberInfo {
                                    name: stringify!($pub_field),
                                    r#type: field_type_info!($pub_type),
                                    doc: EMPTY_DOC,
                                    default: NO_DEFAULT,
                                    deprecated: NO_DEPRECATED,
                                    item: false,
                                    is_abstract: false,
                                },
                            )*
                        ],
                        setters: &[
                            $(
                                MemberInfo {
                                    name: stringify!($pub_field),
                                    r#type: field_type_info!($pub_type),
                                    doc: EMPTY_DOC,
                                    default: NO_DEFAULT,
                                    deprecated: NO_DEPRECATED,
                                    item: false,
                                    is_abstract: false,
                                },
                            )*
                        ],
                        bases: &[],
                        has_eq: false,
                        has_ord: false,
                        has_hash: false,
                        has_str: false,
                        subclass: false,
                        is_abstract: false,
                    }
                }

                inventory::submit! {
                    PyMethodsInfo {
                        struct_id: || ::std::any::TypeId::of::<markers::$Name>(),
                        attrs: &[],
                        getters: &[],
                        setters: &[],
                        methods: &[MethodInfo {
                            name: "__init__",
                            parameters: &[ $(
                                ParameterInfo {
                                    name: stringify!($pub_field),
                                    kind: ParameterKind::KeywordOnly,
                                    type_info: field_type_info!($pub_type),
                                    default: ParameterDefault::Expr(ellipsis_default),
                                },
                            )* ],
                            r#return: ::pyo3_stub_gen::type_info::no_return_type_output,
                            doc: EMPTY_DOC,
                            r#type: MethodType::Instance,
                            is_async: false,
                            deprecated: NO_DEPRECATED,
                            type_ignored: None,
                            is_abstract: false,
                        }],
                    }
                }
            )*
        }

        fn register_built_in_structs(
            py: Python<'_>,
            module: &Bound<'_, PyModule>,
            type_collection: &TypeCollection,
        ) -> PyResult<()> {
            $(
                {
                    let name = stringify!($Name);
                    let default_value: slint_interpreter::Value =
                        i_slint_core::items::$Name::default().into();
                    let data = match default_value {
                        slint_interpreter::Value::Struct(s) => s,
                        _ => unreachable!(),
                    };

                    let factory = StructFactory::new(
                        name,
                        vec![ $( stringify!($pub_field), )* ],
                        data,
                        type_collection.clone(),
                    );

                    let factory_py = Py::new(py, factory)?;
                    module.add(name, factory_py.clone_ref(py))?;
                }
            )*
            Ok(())
        }
    };
}

i_slint_common::for_each_builtin_structs!(generate_struct_support);

pub fn register_structs(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    let type_collection = TypeCollection::with_builtin(py);
    register_built_in_structs(py, module, &type_collection)
}
