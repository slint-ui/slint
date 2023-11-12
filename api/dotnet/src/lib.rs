// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{path::Path, time::Duration};

use rnet::{net, Delegate0, Net};

use i_slint_core::api::{ComponentHandle, Weak};

use i_slint_core::graphics::Image;

use i_slint_core::timers::{Timer, TimerMode};

use slint_interpreter::{ComponentInstance, Value, ValueType};

use i_slint_compiler::langtype::Type;

rnet::root!();

macro_rules! printdebug {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            println!($($arg)*);
        }
    };
}

enum DotNetType {
    STRING = 0,
    NUMBER = 1,
    BOOL = 2,
    IMAGE = 3,
    STRUCT = 4,
}

#[derive(Net)]
pub struct DotNetValue {
    type_name: String,
    type_type: i32,
    type_value: String,
    is_struct: bool,
    struct_props: Vec<DotNetValue>,
}

#[derive(Net)]
pub struct Tokens {
    props: Vec<DotNetValue>,
    calls: Vec<String>,
}

#[derive(Net)]
pub struct DotNetTimer {
    timer_id: i32,
    interval: u64,
}

thread_local! {
    static TIMER_POOL: std::cell::RefCell<Option<Vec<Timer>>> = Default::default();
}

thread_local! {
    static CURRENT_INSTANCE: std::cell::RefCell<Option<ComponentInstance>> = Default::default();
}

// reject modernity, back to the monke
static mut MAIN_WEAK_INSTANCE: Option<Weak<ComponentInstance>> = None;

#[net]
pub fn interprete(path: &str) -> Tokens {
    let mut compiler = slint_interpreter::ComponentCompiler::default();
    let path = std::path::Path::new(path);
    let ret_handle = async_std::task::block_on(compiler.build_from_path(path)).unwrap();

    let mut m_props: Vec<DotNetValue> = Vec::new();
    let props = ret_handle.properties_and_callbacks();

    for prop in props {
        let p_name = prop.0;
        let p_type = prop.1;
        let val_type;
        let mut val_struct = false;
        let mut val_props = Vec::new();
        let val_val = format!("{:?}", "");

        printdebug!("{:?}", p_type);

        match p_type {
            Type::String => {
                val_type = DotNetType::STRING;
            }
            Type::Int32 | Type::Float32 => {
                val_type = DotNetType::NUMBER;
            }
            Type::Bool => {
                val_type = DotNetType::BOOL;
            }
            Type::Image => {
                val_type = DotNetType::IMAGE;
            }
            Type::Struct { fields, .. } => {
                val_type = DotNetType::STRUCT;
                val_struct = true;

                for (field, s_type) in &fields {
                    let sval_type;

                    match s_type {
                        Type::String => sval_type = DotNetType::STRING,
                        Type::Int32 | Type::Float32 => sval_type = DotNetType::NUMBER,
                        Type::Bool => sval_type = DotNetType::BOOL,
                        Type::Image => sval_type = DotNetType::IMAGE,
                        Type::Struct { .. } => {
                            panic!("struct inside struct not supported");
                        }
                        _ => {
                            panic!("Slint type not supported inside a struct");
                        }
                    }

                    val_props.push(DotNetValue {
                        type_name: field.to_string(),
                        type_type: sval_type as i32,
                        type_value: "".to_string(),
                        is_struct: false,
                        struct_props: Vec::new(),
                    });
                }
            }
            Type::Callback { .. } => {
                // FIX-ME: when we want to implement callback with arguments
                // and return types we have to change this
                continue;
            }
            _ => {
                panic!("Slint type not supported");
            }
        }

        m_props.push(DotNetValue {
            type_name: p_name,
            type_type: val_type as i32,
            type_value: val_val,
            is_struct: val_struct,
            struct_props: val_props,
        });
    }

    let m_calls = ret_handle.callbacks().collect();

    let tokens = Tokens { props: m_props, calls: m_calls };

    tokens
}

#[net]
pub fn create(path: &str) {
    printdebug!("create()");

    let mut compiler = slint_interpreter::ComponentCompiler::default();
    let path = std::path::Path::new(path);
    let ret_handle = async_std::task::block_on(compiler.build_from_path(path)).unwrap();

    slint_interpreter::print_diagnostics(compiler.diagnostics());
    let component = ret_handle.create().unwrap();

    CURRENT_INSTANCE.with(|current| current.replace(Some(component.clone_strong())));

    TIMER_POOL.with(|pool| {
        pool.replace(Some(Vec::new()));
    });
}

#[net]
pub fn get_properties() -> Vec<DotNetValue> {
    printdebug!("get_properties()");

    let mut ret: Vec<DotNetValue> = Vec::new();

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        let binding = strong_ref.definition();
        let props = binding.properties();

        for prop in props {
            let p_name = prop.0;
            let p_type = prop.1;
            let val_type;
            let mut val_struct = false;
            let mut val_props = Vec::new();
            let val_val = format!("{:?}", strong_ref.get_property(&p_name).unwrap());

            printdebug!("property {} value {}", p_name, val_val);

            match p_type {
                ValueType::String => {
                    val_type = DotNetType::STRING;
                }
                ValueType::Number => {
                    val_type = DotNetType::NUMBER;
                }
                ValueType::Bool => {
                    val_type = DotNetType::BOOL;
                }
                ValueType::Image => {
                    val_type = DotNetType::IMAGE;
                }
                ValueType::Struct => {
                    val_type = DotNetType::STRUCT;
                    val_struct = true;

                    // create the struct props
                    let s_val = strong_ref.get_property(&p_name).unwrap();
                    match s_val {
                        Value::Struct(stru) => {
                            for field in stru.iter() {
                                let s_name = field.0.to_string();
                                let s_type = field.1.value_type();
                                let sval_type;
                                let sval_struct = false;
                                let sval_val = format!("{:?}", field.1);

                                printdebug!("struct field {} value {}", s_name, sval_val);

                                // FIX-ME: for now we do not accept
                                // struct inside struct
                                match s_type {
                                    ValueType::String => sval_type = DotNetType::STRING,
                                    ValueType::Number => sval_type = DotNetType::NUMBER,
                                    ValueType::Bool => sval_type = DotNetType::BOOL,
                                    ValueType::Image => sval_type = DotNetType::IMAGE,
                                    ValueType::Struct => {
                                        panic!("struct inside struct not supported");
                                    }
                                    _ => {
                                        panic!("Slint type not supported inside a struct");
                                    }
                                }

                                val_props.push(DotNetValue {
                                    type_name: s_name,
                                    type_type: sval_type as i32,
                                    type_value: sval_val,
                                    is_struct: sval_struct,
                                    struct_props: Vec::new(),
                                });
                            }
                        }
                        _ => {
                            panic!("undefined struct type found ????");
                        }
                    }
                }
                _ => {
                    panic!("Slint type not supported");
                }
            }

            ret.push(DotNetValue {
                type_name: p_name,
                type_type: val_type as i32,
                type_value: val_val,
                is_struct: val_struct,
                struct_props: val_props,
            });
        }
    });

    ret
}

#[net]
pub fn set_struct(value: DotNetValue) {
    printdebug!("set_struct()");

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        let name = &value.type_name;
        let props = value.struct_props;
        let val = strong_ref.get_property(name).unwrap();

        match val {
            Value::Struct(mut stru) => {
                for field in stru.clone().iter() {
                    for from_dot_net in &props {
                        if field.0 == from_dot_net.type_name {
                            printdebug!("Field {} found, updating...", field.0);

                            if (DotNetType::STRING as i32) == from_dot_net.type_type {
                                stru.set_field(
                                    from_dot_net.type_name.clone().into(),
                                    Value::String(from_dot_net.type_value.clone().into())
                                );
                            }
                            else if (DotNetType::NUMBER as i32) == from_dot_net.type_type {
                                stru.set_field(
                                    from_dot_net.type_name.clone().into(),
                                    Value::Number(from_dot_net.type_value.parse::<f64>().unwrap())
                                );
                            }
                            else if (DotNetType::BOOL as i32) == from_dot_net.type_type {
                                let val = if from_dot_net.type_value == "True" {
                                    true
                                } else {
                                    false
                                };

                                stru.set_field(
                                    from_dot_net.type_name.clone().into(),
                                    Value::Bool(val)
                                );
                            }
                            else if (DotNetType::IMAGE as i32) == value.type_type {
                                let path = Path::new(&value.type_value);
                                let img = Image::load_from_path(path).unwrap();


                                stru.set_field(
                                    from_dot_net.type_name.clone().into(),
                                    Value::Image(img)
                                );
                            } else {
                                panic!("Type {} was not resolved", value.type_type);
                            }
                        }
                    }
                }

                // then set the struct back to the component
                strong_ref.set_property(name, Value::Struct(stru)).unwrap();
            }
            _ => {
                panic!("undefined struct type found or you are trying to access a non struct typep property");
            }
        }
    });
}

#[net]
pub fn set_property(value: DotNetValue) {
    printdebug!("set_property()");

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        if (DotNetType::STRING as i32) == value.type_type {
            strong_ref
                .set_property(&value.type_name, Value::String(value.type_value.into()))
                .unwrap();
        } else if (DotNetType::NUMBER as i32) == value.type_type {
            strong_ref
                .set_property(
                    &value.type_name,
                    Value::Number(value.type_value.parse::<f64>().unwrap()),
                )
                .unwrap();
        } else if (DotNetType::BOOL as i32) == value.type_type {
            let val = if value.type_value == "True" { true } else { false };

            strong_ref.set_property(&value.type_name, Value::Bool(val)).unwrap();
        } else if (DotNetType::IMAGE as i32) == value.type_type {
            let path = Path::new(&value.type_value);
            let img = Image::load_from_path(path).unwrap();

            strong_ref.set_property(&value.type_name, Value::Image(img)).unwrap();
        } else {
            panic!("Type {} was not resolved", value.type_type);
        }
    });
}

#[net]
pub fn get_struct(name: &str) -> DotNetValue {
    printdebug!("get_struct()");

    let mut ret: DotNetValue = DotNetValue {
        type_name: "".to_string(),
        type_type: 0,
        type_value: "".to_string(),
        is_struct: true,
        struct_props: Vec::new(),
    };

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        let val = strong_ref.get_property(name).unwrap();

        ret.type_name = name.into();
        ret.type_type = val.value_type() as i32;
        // there is no "value"
        ret.type_value = "".to_string();

        match val {
            Value::Struct(stru) => {
                for field in stru.iter() {
                    let s_name = field.0.to_string();
                    let s_type = field.1.value_type();
                    let sval_type;
                    let sval_struct = false;
                    let sval_val = format!("{:?}", field.1);

                    printdebug!("struct field {} value {}", s_name, sval_val);

                    // FIX-ME: for now we do not accept
                    // struct inside struct
                    match s_type {
                        ValueType::String => sval_type = DotNetType::STRING,
                        ValueType::Number => sval_type = DotNetType::NUMBER,
                        ValueType::Bool => sval_type = DotNetType::BOOL,
                        ValueType::Image => sval_type = DotNetType::IMAGE,
                        ValueType::Struct => {
                            panic!("struct inside struct not supported");
                        }
                        _ => {
                            panic!("Slint type not supported inside a struct");
                        }
                    }

                    ret.struct_props.push(DotNetValue {
                        type_name: s_name,
                        type_type: sval_type as i32,
                        type_value: sval_val,
                        is_struct: sval_struct,
                        struct_props: Vec::new(),
                    });
                }
            }
            _ => {
                panic!("undefined struct type found ????");
            }
        }
    });

    ret
}

#[net]
pub fn get_property(name: &str) -> DotNetValue {
    let mut ret: DotNetValue = DotNetValue {
        type_name: "".to_string(),
        type_type: 0,
        type_value: "".to_string(),
        is_struct: false,
        struct_props: Vec::new(),
    };

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        let val = strong_ref.get_property(name).unwrap();

        let val_str = format!("{:?}", val);
        ret.type_name = name.into();
        ret.type_type = val.value_type() as i32;
        ret.type_value = val_str.clone();

        printdebug!("{}", val_str);
    });

    ret
}

#[net]
pub fn get_callbacks() -> Vec<String> {
    printdebug!("get_callbacks()");

    let mut ret: Vec<String> = Vec::new();

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        let binding = strong_ref.definition();
        let calls = binding.callbacks();
        ret = calls.collect();
    });

    ret
}

#[net]
pub fn set_callback(name: &str, callback: Delegate0<bool>) {
    printdebug!("set_callback()");

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));

        strong_ref
            .set_callback(name, move |_| {
                callback.call();
                Value::Void
            })
            .unwrap();
    });
}

#[net]
pub fn new_timer(mode: i32, interval: u64, callback: Delegate0<bool>) -> DotNetTimer {
    let ret = DotNetTimer { timer_id: -1, interval };

    TIMER_POOL.with(|pool| {
        let mut mut_pool = pool.borrow_mut().take().unwrap();
        let timer = Timer::default();

        let time_mode = if mode == 1 { TimerMode::Repeated } else { TimerMode::SingleShot };

        let int_duration = Duration::from_millis(interval);

        timer.start(time_mode, int_duration, move || {
            callback.call();
        });

        mut_pool.push(timer);
        pool.replace(Some(mut_pool));
    });

    ret
}

#[net]
pub fn stop_timer(timer: DotNetTimer) {
    TIMER_POOL.with(|pool| {
        let pool = pool.borrow_mut().take().unwrap();
        let mut index = 0;

        // for each timer check if the id match
        for tmr in pool {
            if index == timer.timer_id {
                tmr.stop();
                break;
            }

            index += 1;
        }
    });
}

#[net]
pub fn restart_timer(timer: DotNetTimer) {
    TIMER_POOL.with(|pool| {
        let pool = pool.borrow_mut().take().unwrap();
        let mut index = 0;

        // for each timer check if the id match
        for tmr in pool {
            if index == timer.timer_id {
                tmr.restart();
                break;
            }

            index += 1;
        }
    });
}

#[net]
pub fn run_on_ui_thread(callback: Delegate0<bool>) {
    printdebug!("run_on_ui_thread()");

    // reject modernity, back to the monke
    let weak_ref = unsafe { MAIN_WEAK_INSTANCE.take().unwrap() };
    unsafe {
        MAIN_WEAK_INSTANCE = Some(weak_ref.clone());
    };

    weak_ref
        .upgrade_in_event_loop(move |_| {
            callback.call();
        })
        .unwrap();
}

#[net]
pub fn run() {
    printdebug!("run()");

    CURRENT_INSTANCE.with(|current| {
        let strong_ref = current.borrow_mut().take().unwrap();
        current.replace(Some(strong_ref.clone_strong()));
        let weak_ref = strong_ref.as_weak();

        unsafe {
            MAIN_WEAK_INSTANCE = Some(weak_ref);
        };

        strong_ref.run().unwrap();
    });
}
