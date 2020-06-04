use core::cell::RefCell;
use corelib::abi::datastructures::ComponentBox;
use neon::prelude::*;
use sixtyfps_compiler::typeregister::Type;

mod persistent_context;

struct WrappedComponentType(Option<std::rc::Rc<interpreter::ComponentDescription>>);
struct WrappedComponentBox(Option<ComponentBox>);

/// We need to do some gymnastic with closures to pass the ExecuteContext with the right lifetime
type GlobalContextCallback =
    dyn for<'b> Fn(&mut ExecuteContext<'b>, &persistent_context::PersistentContext<'b>);
scoped_tls_hkt::scoped_thread_local!(static GLOBAL_CONTEXT: 
    for <'a> &'a dyn Fn(&GlobalContextCallback));

/// Load a .60 files.
///
/// The first argument of this finction is a string to the .60 file
///
/// The return value is a SixtyFpsComponentType
fn load(mut cx: FunctionContext) -> JsResult<JsValue> {
    let path = cx.argument::<JsString>(0)?.value();
    let path = std::path::Path::new(path.as_str());
    let source = std::fs::read_to_string(&path).or_else(|e| cx.throw_error(e.to_string()))?;
    let c = match interpreter::load(source.as_str(), &path) {
        Ok(c) => c,
        Err(diag) => {
            diag.print(source);
            return cx.throw_error("Compilation error");
        }
    };

    let mut obj = SixtyFpsComponentType::new::<_, JsValue, _>(&mut cx, std::iter::empty())?;
    cx.borrow_mut(&mut obj, |mut obj| obj.0 = Some(c));
    Ok(obj.as_value(&mut cx))
}

fn create<'cx>(
    cx: &mut CallContext<'cx, impl neon::object::This>,
    component_type: std::rc::Rc<interpreter::ComponentDescription>,
) -> JsResult<'cx, JsValue> {
    let mut component = component_type.clone().create();
    let persistent_context = persistent_context::PersistentContext::new(cx);

    if let Some(args) = cx.argument_opt(0).and_then(|arg| arg.downcast::<JsObject>().ok()) {
        let properties = component_type.properties();
        for x in args.get_own_property_names(cx)?.to_vec(cx)? {
            let prop_name = x.to_string(cx)?.value();
            let value = args.get(cx, x)?;
            let ty = properties
                .get(&prop_name)
                .ok_or(())
                .or_else(|()| {
                    cx.throw_error(format!("Property {} not found in the component", prop_name))
                })?
                .clone();
            if ty == Type::Signal {
                let _fun = value.downcast_or_throw::<JsFunction, _>(cx)?;
                let fun_idx = persistent_context.allocate(cx, value);
                component_type
                    .set_signal_handler(
                        component.borrow_mut(),
                        prop_name.as_str(),
                        Box::new(move |_eval_ctx, ()| {
                            GLOBAL_CONTEXT.with(|cx_fn| {
                                cx_fn(&move |cx, presistent_context| {
                                    presistent_context
                                        .get(cx, fun_idx)
                                        .unwrap()
                                        .downcast::<JsFunction>()
                                        .unwrap()
                                        .call::<_, _, JsValue, _>(
                                            cx,
                                            JsUndefined::new(),
                                            std::iter::empty(),
                                        )
                                        .unwrap();
                                })
                            })
                        }),
                    )
                    .or_else(|_| cx.throw_error(format!("Cannot set signal")))?;
            } else {
                let value = to_eval_value(value, ty, cx)?;
                component_type
                    .set_property(component.borrow(), prop_name.as_str(), value)
                    .or_else(|_| cx.throw_error(format!("Cannot assign property")))?;
            }
        }
    }

    let mut obj = SixtyFpsComponent::new::<_, JsValue, _>(cx, std::iter::empty())?;
    persistent_context.save_to_object(cx, obj.downcast().unwrap());
    cx.borrow_mut(&mut obj, |mut obj| obj.0 = Some(component));
    Ok(obj.as_value(cx))
}

fn to_eval_value<'a>(
    val: Handle<JsValue>,
    ty: sixtyfps_compiler::typeregister::Type,
    cx: &mut impl Context<'a>,
) -> NeonResult<interpreter::Value> {
    use interpreter::Value;
    match ty {
        Type::Invalid | Type::Component(_) | Type::Builtin(_) | Type::Signal => {
            cx.throw_error("Cannot convert to a Sixtyfps property value")
        }
        Type::Float32 | Type::Int32 => {
            Ok(Value::Number(val.downcast_or_throw::<JsNumber, _>(cx)?.value()))
        }
        Type::String => Ok(Value::String(val.to_string(cx)?.value().as_str().into())),
        Type::Color => todo!(),
        Type::Image => todo!(),
        Type::Bool => Ok(Value::Bool(val.downcast_or_throw::<JsBoolean, _>(cx)?.value())),
    }
}

fn show<'cx>(
    cx: &mut CallContext<'cx, impl neon::object::This>,
    component: ComponentBox,
    presistent_context: persistent_context::PersistentContext<'cx>,
) -> JsResult<'cx, JsUndefined> {
    cx.execute_scoped(|cx| {
        let cx = RefCell::new(cx);
        let cx_fn = move |callback: &GlobalContextCallback| {
            callback(&mut *cx.borrow_mut(), &presistent_context)
        };
        GLOBAL_CONTEXT.set(&&cx_fn, || {
            sixtyfps_rendering_backend_gl::sixtyfps_runtime_run_component_with_gl_renderer(
                component.borrow(),
            );
        })
    });

    Ok(JsUndefined::new())
}

declare_types! {
    class SixtyFpsComponentType for WrappedComponentType {
        init(_) {
            Ok(WrappedComponentType(None))
        }
        method create(mut cx) {
            let this = cx.this();
            let ct = cx.borrow(&this, |x| x.0.clone());
            let ct = ct.ok_or(()).or_else(|()| cx.throw_error("Invalid type"))?;
            create(&mut cx, ct)
        }
        method name(mut cx) {
            let this = cx.this();
            let ct = cx.borrow(&this, |x| x.0.clone());
            let ct = ct.ok_or(()).or_else(|()| cx.throw_error("Invalid type"))?;
            Ok(cx.string(ct.id()).as_value(&mut cx))
        }
    }

    class SixtyFpsComponent for WrappedComponentBox {
        init(_) {
            Ok(WrappedComponentBox(None))
        }
        method show(mut cx) {
            let mut this = cx.this();
            // FIXME: is take() here the right choice?
            let component = cx.borrow_mut(&mut this, |mut x| x.0.take());
            let component = component.ok_or(()).or_else(|()| cx.throw_error("Invalid type"))?;
            let persistent_context = persistent_context::PersistentContext::from_object(&mut cx, this.downcast().unwrap())?;
            show(&mut cx, component, persistent_context)?;
            Ok(JsUndefined::new().as_value(&mut cx))
        }
    }
}

register_module!(mut m, {
    m.export_function("load", load)?;
    Ok(())
});
