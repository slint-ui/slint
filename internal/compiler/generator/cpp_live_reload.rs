// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::cpp::{concatenate_ident, cpp_ast::*, ident, Config};
use crate::langtype::Type;
use crate::llr;
use crate::object_tree::Document;
use crate::CompilerConfiguration;
use itertools::Itertools as _;
use smol_str::format_smolstr;
use std::io::BufWriter;

pub fn generate(
    doc: &Document,
    config: Config,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<File> {
    let mut file = super::cpp::generate_types(&doc.used_types.borrow().structs_and_enums, &config);

    file.includes.push("<slint_live_reload.h>".into());

    generate_value_conversions(&mut file, &doc.used_types.borrow().structs_and_enums);

    let llr = crate::llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config)?;

    let main_file = doc
        .node
        .as_ref()
        .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?
        .source_file
        .path()
        .to_string_lossy();

    for p in &llr.public_components {
        generate_public_component(&mut file, p, &llr, compiler_config, &main_file);
    }

    for glob in &llr.globals {
        if glob.must_generate() {
            generate_global(&mut file, glob);
            file.definitions.extend(glob.aliases.iter().map(|name| {
                Declaration::TypeAlias(TypeAlias {
                    old_name: ident(&glob.name),
                    new_name: ident(name),
                })
            }));
        };
    }

    super::cpp::generate_type_aliases(&mut file, doc);

    let cpp_files = file.split_off_cpp_files(config.header_include, config.cpp_files.len());
    for (cpp_file_name, cpp_file) in config.cpp_files.iter().zip(cpp_files) {
        use std::io::Write;
        write!(&mut BufWriter::new(std::fs::File::create(&cpp_file_name)?), "{cpp_file}")?;
    }

    Ok(file)
}

fn generate_public_component(
    file: &mut File,
    component: &llr::PublicComponent,
    unit: &llr::CompilationUnit,
    compiler_config: &CompilerConfiguration,
    main_file: &str,
) {
    let component_id = ident(&component.name);

    let mut component_struct = Struct { name: component_id.clone(), ..Default::default() };

    component_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: "slint::private_api::live_reload::LiveReloadingComponent".into(),
            name: "live_reload".into(),
            ..Default::default()
        }),
    ));

    let mut global_accessor_function_body = Vec::new();
    for glob in unit.globals.iter().filter(|glob| glob.exported && glob.must_generate()) {
        let accessor_statement = format!(
            "{0}if constexpr(std::is_same_v<T, {1}>) {{ return T(live_reload); }}",
            if global_accessor_function_body.is_empty() { "" } else { "else " },
            concatenate_ident(&glob.name),
        );
        global_accessor_function_body.push(accessor_statement);
    }
    if !global_accessor_function_body.is_empty() {
        global_accessor_function_body.push(
            "else { static_assert(!sizeof(T*), \"The type is not global/or exported\"); }".into(),
        );

        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "global".into(),
                signature: "() const -> T".into(),
                statements: Some(global_accessor_function_body),
                template_parameters: Some("typename T".into()),
                ..Default::default()
            }),
        ));
    }

    generate_public_api_for_properties(
        "",
        &mut component_struct.members,
        &component.public_properties,
        &component.private_properties,
    );

    component_struct.members.push((
        Access::Public,
        Declaration::Var(Var {
            ty: "static const slint::private_api::ItemTreeVTable".into(),
            name: "static_vtable".into(),
            ..Default::default()
        }),
    ));

    file.definitions.push(Declaration::Var(Var {
        ty: "const slint::private_api::ItemTreeVTable".into(),
        name: format_smolstr!("{component_id}::static_vtable"),
        init: Some(format!(
            "{{ nullptr, nullptr, nullptr, nullptr, \
                nullptr, nullptr, nullptr, nullptr, nullptr, \
                nullptr, nullptr, nullptr, nullptr, \
                nullptr, nullptr, nullptr, \
                slint::private_api::drop_in_place<{component_id}>, slint::private_api::dealloc }}"
        )),
        ..Default::default()
    }));

    let create_code = vec![
        format!("slint::SharedVector<slint::SharedString> include_paths{{ {} }};", compiler_config.include_paths.iter().map(|p| format!("\"{}\"", escape_string(&p.to_string_lossy()))).join(", ")),
        format!("slint::SharedVector<slint::SharedString> library_paths{{ {} }};", compiler_config.library_paths.iter().map(|(l, p)| format!("\"{l}={}\"", p.to_string_lossy())).join(", ")),
        format!("auto live_reload = slint::private_api::live_reload::LiveReloadingComponent({main_file:?}, {:?}, include_paths, library_paths, \"{}\");", component.name, compiler_config.style.as_ref().unwrap_or(&String::new())),
        format!("auto self_rc = vtable::VRc<slint::private_api::ItemTreeVTable, {component_id}>::make(std::move(live_reload));"),
        format!("return slint::ComponentHandle<{component_id}>(self_rc);"),
    ];

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "create".into(),
            signature: format!("() -> slint::ComponentHandle<{component_id}>"),
            statements: Some(create_code),
            is_static: true,
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            is_constructor_or_destructor: true,
            name: ident(&component_struct.name),
            signature: "(slint::private_api::live_reload::LiveReloadingComponent live_reload)"
                .into(),
            constructor_member_initializers: vec!["live_reload(std::move(live_reload))".into()],
            statements: Some(vec![]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "show".into(),
            signature: "() -> void".into(),
            statements: Some(vec!["window().show();".into()]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "hide".into(),
            signature: "() -> void".into(),
            statements: Some(vec!["window().hide();".into()]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "window".into(),
            signature: "() const -> slint::Window&".into(),
            statements: Some(vec!["return live_reload.window();".into()]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "run".into(),
            signature: "() -> void".into(),
            statements: Some(vec![
                "show();".into(),
                "slint::run_event_loop();".into(),
                "hide();".into(),
            ]),
            ..Default::default()
        }),
    ));

    file.definitions.extend(component_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(component_struct));
}

fn generate_global(file: &mut File, global: &llr::GlobalComponent) {
    let mut global_struct = Struct { name: ident(&global.name), ..Default::default() };

    global_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: "const slint::private_api::live_reload::LiveReloadingComponent&".into(),
            name: "live_reload".into(),
            ..Default::default()
        }),
    ));

    global_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            is_constructor_or_destructor: true,
            name: ident(&global.name),
            signature:
                "(const slint::private_api::live_reload::LiveReloadingComponent &live_reload)"
                    .into(),
            constructor_member_initializers: vec!["live_reload(live_reload)".into()],
            statements: Some(vec![]),
            ..Default::default()
        }),
    ));

    generate_public_api_for_properties(
        &format!("{}.", global.name),
        &mut global_struct.members,
        &global.public_properties,
        &global.private_properties,
    );

    file.definitions.extend(global_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(global_struct));
}

fn generate_public_api_for_properties(
    prefix: &str,
    declarations: &mut Vec<(Access, Declaration)>,
    public_properties: &llr::PublicProperties,
    private_properties: &llr::PrivateProperties,
) {
    for p in public_properties {
        let prop_name = &p.name;
        let prop_ident = concatenate_ident(prop_name);

        if let Type::Callback(callback) = &p.ty {
            let ret = callback.return_type.cpp_type().unwrap();
            let param_types =
                callback.args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let callback_emitter = vec![format!(
                "return {}(live_reload.invoke(\"{prefix}{prop_name}\" {}));",
                convert_from_value_fn(&callback.return_type),
                (0..callback.args.len()).map(|i| format!(", arg_{i}")).join(""),
            )];
            declarations.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format_smolstr!("invoke_{prop_ident}"),
                    signature: format!(
                        "({}) const -> {ret}",
                        param_types
                            .iter()
                            .enumerate()
                            .map(|(i, ty)| format!("{ty} arg_{i}"))
                            .join(", "),
                    ),
                    statements: Some(callback_emitter),
                    ..Default::default()
                }),
            ));
            let args = callback
                .args
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}(args[{i}])", convert_from_value_fn(t)))
                .join(", ");
            let return_statement = if callback.return_type == Type::Void {
                format!("callback_handler({args}); return slint::interpreter::Value();",)
            } else {
                format!(
                    "return {}(callback_handler({args}));",
                    convert_to_value_fn(&callback.return_type),
                )
            };
            declarations.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format_smolstr!("on_{}", concatenate_ident(&p.name)),
                    template_parameters: Some(format!(
                        "std::invocable<{}> Functor",
                        param_types.join(", "),
                    )),
                    signature: "(Functor && callback_handler) const".into(),
                    statements: Some(vec![
                        "using slint::private_api::live_reload::into_slint_value;".into(),
                        format!(
                            "live_reload.set_callback(\"{prefix}{prop_name}\", [callback_handler]([[maybe_unused]] auto args) {{ {return_statement} }});",
                        ),
                    ]),
                    ..Default::default()
                }),
            ));
        } else if let Type::Function(function) = &p.ty {
            let param_types =
                function.args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let ret = function.return_type.cpp_type().unwrap();
            let call_code = vec![format!(
                "return {}(live_reload.invoke(\"{prefix}{prop_name}\"{}));",
                convert_from_value_fn(&function.return_type),
                (0..function.args.len()).map(|i| format!(", arg_{i}")).join("")
            )];
            declarations.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format_smolstr!("invoke_{}", concatenate_ident(&p.name)),
                    signature: format!(
                        "({}) const -> {ret}",
                        param_types
                            .iter()
                            .enumerate()
                            .map(|(i, ty)| format!("{ty} arg_{i}"))
                            .join(", "),
                    ),
                    statements: Some(call_code),
                    ..Default::default()
                }),
            ));
        } else {
            let cpp_property_type = p.ty.cpp_type().expect("Invalid type in public properties");
            let prop_getter: Vec<String> = vec![format!(
                "return {}(live_reload.get_property(\"{prefix}{prop_name}\"));",
                convert_from_value_fn(&p.ty)
            )];
            declarations.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format_smolstr!("get_{}", &prop_ident),
                    signature: format!("() const -> {cpp_property_type}"),
                    statements: Some(prop_getter),
                    ..Default::default()
                }),
            ));

            if !p.read_only {
                let prop_setter: Vec<String> = vec![
                    "using slint::private_api::live_reload::into_slint_value;".into(),
                    format!(
                        "live_reload.set_property(\"{prefix}{prop_name}\", {}(value));",
                        convert_to_value_fn(&p.ty)
                    ),
                ];
                declarations.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format_smolstr!("set_{}", &prop_ident),
                        signature: format!("(const {} &value) const -> void", cpp_property_type),
                        statements: Some(prop_setter),
                        ..Default::default()
                    }),
                ));
            } else {
                declarations.push((
                    Access::Private,
                    Declaration::Function(Function {
                        name: format_smolstr!("set_{}", &prop_ident),
                        signature: format!(
                            "(const {cpp_property_type} &) const = delete /* property '{}' is declared as 'out' (read-only). Declare it as 'in' or 'in-out' to enable the setter */", p.name
                        ),
                        ..Default::default()
                    }),
                ));
            }
        }
    }

    for (name, ty) in private_properties {
        let prop_ident = concatenate_ident(name);

        if let Type::Function(function) = &ty {
            let param_types = function.args.iter().map(|t| t.cpp_type().unwrap()).join(", ");
            declarations.push((
                Access::Private,
                Declaration::Function(Function {
                    name: format_smolstr!("invoke_{prop_ident}"),
                    signature: format!(
                        "({param_types}) const = delete /* the function '{name}' is declared as private. Declare it as 'public' */",
                    ),
                    ..Default::default()
                }),
            ));
        } else {
            declarations.push((
                Access::Private,
                Declaration::Function(Function {
                    name: format_smolstr!("get_{prop_ident}"),
                    signature: format!(
                        "() const = delete /* the property '{name}' is declared as private. Declare it as 'in', 'out', or 'in-out' to make it public */",
                    ),
                    ..Default::default()
                }),
            ));
            declarations.push((
                Access::Private,
                Declaration::Function(Function {
                    name: format_smolstr!("set_{}", &prop_ident),
                    signature: format!(
                        "(const auto &) const = delete /* property '{name}' is declared as private. Declare it as 'in' or 'in-out' to make it public */",
                    ),
                    ..Default::default()
                }),
            ));
        }
    }
}

fn convert_to_value_fn(ty: &Type) -> String {
    match ty {
        Type::Struct(s) if s.name.is_none() => {
            let mut init = s.fields.iter().enumerate().map(|(i, (name, ty))| {
                format!(
                    "s.set_field(\"{name}\", {}(std::get<{i}>(tuple))); ",
                    convert_to_value_fn(ty)
                )
            });
            format!("([](const auto &tuple) {{ slint::interpreter::Struct s; {}return slint::interpreter::Value(s); }})", init.join(""))
        }
        // Array of anonymous struct
        Type::Array(a) if matches!(a.as_ref(), Type::Struct(s) if s.name.is_none()) => {
            let conf_fn = convert_to_value_fn(&a);
            let aty = a.cpp_type().unwrap();
            format!("([](const auto &model) {{ return slint::interpreter::Value(std::make_shared<slint::MapModel<{aty}, slint::interpreter::Value>>(model, {conf_fn})); }})")
        }
        _ => "into_slint_value".into(),
    }
}

fn convert_from_value_fn(ty: &Type) -> String {
    match ty {
        Type::Struct(s) if s.name.is_none() => {
            let mut init = s.fields.iter().map(|(name, ty)| {
                format!("slint::private_api::live_reload::from_slint_value<{}>(s.get_field(\"{name}\").value())", ty.cpp_type().unwrap())
            });
            format!(
                "([](const slint::interpreter::Value &v) {{ auto s = v.to_struct().value(); return std::make_tuple({}); }})",
                init.join(", ")
            )
        }
        _ => format!(
            "slint::private_api::live_reload::from_slint_value<{}>",
            ty.cpp_type().unwrap_or_default()
        ),
    }
}

fn generate_value_conversions(file: &mut File, structs_and_enums: &[Type]) {
    for ty in structs_and_enums {
        match ty {
            Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                let name = ident(&s.name.as_ref().unwrap());
                let mut to_statements = vec![
                    "using slint::private_api::live_reload::into_slint_value;".into(),
                    "slint::interpreter::Struct s;".into(),
                ];
                let mut from_statements = vec![
                    "using slint::private_api::live_reload::from_slint_value;".into(),
                    "slint::interpreter::Struct s = val.to_struct().value();".into(),
                    format!("{name} self;"),
                ];
                for (f, t) in &s.fields {
                    to_statements.push(format!(
                        "s.set_field(\"{f}\", into_slint_value(self.{}));",
                        ident(f)
                    ));
                    from_statements.push(format!(
                        "self.{} = slint::private_api::live_reload::from_slint_value<{}>(s.get_field(\"{f}\").value());",
                        ident(f),
                        t.cpp_type().unwrap()
                    ));
                }
                to_statements.push("return s;".into());
                from_statements.push("return self;".into());
                file.declarations.push(Declaration::Function(Function {
                    name: "into_slint_value".into(),
                    signature: format!(
                        "([[maybe_unused]] const {name} &self) -> slint::interpreter::Value"
                    ),
                    statements: Some(to_statements),
                    is_inline: true,
                    ..Function::default()
                }));
                file.declarations.push(Declaration::Function(Function {
                    name: "from_slint_value".into(),
                    signature: format!(
                        "(const slint::interpreter::Value &val, const {name} *) -> {name}"
                    ),
                    statements: Some(from_statements),
                    is_inline: true,
                    ..Function::default()
                }));
            }
            Type::Enumeration(_) => {
                // todo;
            }
            _ => (),
        }
    }
}
