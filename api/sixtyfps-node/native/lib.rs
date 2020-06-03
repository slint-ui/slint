use neon::prelude::*;

struct WrappedComponentType(Option<std::rc::Rc<interpreter::ComponentDescription>>);
struct WrappedComponentBox(Option<corelib::abi::datastructures::ComponentBox>);

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
    let component = component_type.create();

    //component_type.set_property(component, name, value);

    let mut obj = SixtyFpsComponent::new::<_, JsValue, _>(cx, std::iter::empty())?;
    cx.borrow_mut(&mut obj, |mut obj| obj.0 = Some(component));
    Ok(obj.as_value(cx))
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
            gl::sixtyfps_runtime_run_component_with_gl_renderer(component.leak());
            // FIXME: leak (that's because we somehow need a static life time)
            Ok(JsUndefined::new().as_value(&mut cx))
        }
    }
}

register_module!(mut m, {
    m.export_function("load", load)?;
    Ok(())
});
