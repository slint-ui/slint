/*!
    Property binding engine.

    The current implementation uses lots of heap allocation but that can be optimized later using
    thin dst container, and intrusive linked list
*/

use std::cell::RefCell;
use std::{
    ops::DerefMut,
    rc::{Rc, Weak},
};

thread_local!(static CURRENT_PROPERTY : RefCell<Option<Rc<dyn PropertyNotify>>> = Default::default());

type Binding = Box<dyn Fn(*mut (), &EvaluationContext)>;

#[derive(Default)]
struct PropertyImpl {
    /// Invariant: Must only be called with a pointer to the binding
    binding: Option<Binding>,
    dependencies: Vec<Weak<dyn PropertyNotify>>,
    dirty: bool,
    //updating: bool,
}

/// PropertyNotify is the interface that allows keeping track of dependencies between
/// property bindings.
    /// mark_dirty() is called to notify a property that its binding may need to be re-evaluated
    /// because one of its dependencies may have changed.
    fn mark_dirty(self: Rc<Self>);
    /// notify() is called to register the property (self) with the currently (thread-local) evaluating
    /// property binding.
    fn notify(self: Rc<Self>);
}

impl PropertyNotify for RefCell<PropertyImpl> {
    fn mark_dirty(self: Rc<Self>) {
        let mut v = vec![];
        {
            let mut dep = self.borrow_mut();
            dep.dirty = true;
            std::mem::swap(&mut dep.dependencies, &mut v);
        }
        for d in &v {
            if let Some(d) = d.upgrade() {
                d.mark_dirty();
            }
        }
    }

    fn notify(self: Rc<Self>) {
        CURRENT_PROPERTY.with(|cur_dep| {
            if let Some(m) = &(*cur_dep.borrow()) {
                self.borrow_mut().dependencies.push(Rc::downgrade(m));
            }
        });
    }
}

/// This structure contains what is required for the property engine to evaluate properties
///
/// One must pass it to the getter of the property, or emit of signals, and it can
/// be accessed from the bindings
#[repr(C)]
pub struct EvaluationContext<'a> {
    /// The component which contains the Property or the Signal
    pub component: vtable::VRef<'a, crate::abi::datastructures::ComponentVTable>,
}

type PropertyHandle = Rc<RefCell<PropertyImpl>>;
/// A Property that allow binding that track changes
///
/// Property van have be assigned value, or bindings.
/// When a binding is assigned, it is lazily evaluated on demand
/// when calling `get()`.
/// When accessing another property from a binding evaluation,
/// a dependency will be registered, such that when the property
/// change, the binding will automatically be updated
#[repr(C)]
#[derive(Default)]
pub struct Property<T: 'static> {
    inner: PropertyHandle,
    /// Only access when holding a lock of the inner refcell.
    value: core::cell::UnsafeCell<T>,
}

impl<T: Clone + 'static> Property<T> {
    /// Get the value of the property
    ///
    /// This may evaluate the binding if there is a binding and it is dirty
    ///
    /// If the function is called directly or indirectly from a binding evaluation
    /// of another Property, a dependency will be registered.
    ///
    /// The context must be the constext matching the Component which contains this
    /// property
    pub fn get(&self, context: &EvaluationContext) -> T {
        self.update(context);
        self.inner.clone().notify();
        let _lock = self.inner.borrow();
        unsafe { (*(self.value.get() as *const T)).clone() }
    }

    /// Change the value of this property
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set(&self, t: T) {
        {
            let mut lock = self.inner.borrow_mut();
            lock.binding = None;
            lock.dirty = false;
            unsafe { *self.value.get() = t };
        }
        self.inner.clone().mark_dirty();
        self.inner.borrow_mut().dirty = false;
    }

    /// Set a binding to this property.
    ///
    /// Binding are evaluated lazily from calling get, and the return value of the binding
    /// is the new value.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_binding(&self, f: impl (Fn(&EvaluationContext) -> T) + 'static) {
        let real_binding = move |ptr: *mut (), context: &EvaluationContext| {
            // The binding must be called with a pointer of T
            unsafe { *(ptr as *mut T) = f(context) };
        };

        self.inner.borrow_mut().binding = Some(Box::new(real_binding));
        self.inner.clone().mark_dirty();
    }

    /// Call the binding if the property is dirty to update the stored value
    fn update(&self, context: &EvaluationContext) {
        if !self.inner.borrow().dirty {
            return;
        }
        let mut old: Option<Rc<dyn PropertyNotify>> = Some(self.inner.clone());
        let mut lock =
            self.inner.try_borrow_mut().expect("Circular dependency in binding evaluation");
        if let Some(binding) = &lock.binding {
            CURRENT_PROPERTY.with(|cur_dep| {
                let mut m = cur_dep.borrow_mut();
                std::mem::swap(m.deref_mut(), &mut old);
            });
            binding(self.value.get() as *mut _, context);
            lock.dirty = false;
            CURRENT_PROPERTY.with(|cur_dep| {
                let mut m = cur_dep.borrow_mut();
                std::mem::swap(m.deref_mut(), &mut old);
                //somehow ptr_eq does not work as expected despite the pointer are equal
                //debug_assert!(Rc::ptr_eq(&(self.inner.clone() as Rc<dyn PropertyNotify>), &old.unwrap()));
            });
        }
    }
}

#[test]
fn properties_simple_test() {
    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        height: Property<i32>,
        area: Property<i32>,
    }
    let dummy_eval_context = EvaluationContext {
        component: unsafe {
            vtable::VRef::from_raw(core::ptr::NonNull::dangling(), core::ptr::NonNull::dangling())
        },
    };
    let compo = Rc::new(Component::default());
    let w = Rc::downgrade(&compo);
    compo.area.set_binding(move |ctx| {
        let compo = w.upgrade().unwrap();
        compo.width.get(ctx) * compo.height.get(ctx)
    });
    compo.width.set(4);
    compo.height.set(8);
    assert_eq!(compo.width.get(&dummy_eval_context), 4);
    assert_eq!(compo.height.get(&dummy_eval_context), 8);
    assert_eq!(compo.area.get(&dummy_eval_context), 4 * 8);

    let w = Rc::downgrade(&compo);
    compo.width.set_binding(move |ctx| {
        let compo = w.upgrade().unwrap();
        compo.height.get(ctx) * 2
    });
    assert_eq!(compo.width.get(&dummy_eval_context), 8 * 2);
    assert_eq!(compo.height.get(&dummy_eval_context), 8);
    assert_eq!(compo.area.get(&dummy_eval_context), 8 * 8 * 2);
}

#[allow(non_camel_case_types)]
type c_void = ();
#[repr(C)]
/// Has the same layout as PropertyHandle
pub struct PropertyHandleOpaque(*const c_void);

/// Initialize the first pointer of the Property. Does not initialize the content
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_init(out: *mut PropertyHandleOpaque) {
    assert_eq!(
        core::mem::size_of::<PropertyHandle>(),
        core::mem::size_of::<PropertyHandleOpaque>()
    );
    core::ptr::write(out as *mut PropertyHandle, PropertyHandle::default());
}

/// To be called before accessing the value
///
/// (same as Property::update and PopertyImpl::notify)
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_update(
    out: *const PropertyHandleOpaque,
    context: *const EvaluationContext,
    val: *mut c_void,
) {
    let inner = &*(out as *const PropertyHandle);

    if !inner.borrow().dirty {
        inner.clone().notify();
        return;
    }
    let mut old: Option<Rc<dyn PropertyNotify>> = Some(inner.clone());
    let mut lock = inner.try_borrow_mut().expect("Circular dependency in binding evaluation");
    if let Some(binding) = &lock.binding {
        CURRENT_PROPERTY.with(|cur_dep| {
            let mut m = cur_dep.borrow_mut();
            std::mem::swap(m.deref_mut(), &mut old);
        });
        binding(val, &*context);
        lock.dirty = false;
        CURRENT_PROPERTY.with(|cur_dep| {
            let mut m = cur_dep.borrow_mut();
            std::mem::swap(m.deref_mut(), &mut old);
            //somehow ptr_eq does not work as expected despite the pointer are equal
            //debug_assert!(Rc::ptr_eq(&(inner.clone() as Rc<dyn PropertyNotify>), &old.unwrap()));
        });
    }
    core::mem::drop(lock);
    inner.clone().notify();
}

/// Mark the fact that the property was changed and that its binding need to be removed, and
/// The dependencies marked dirty
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_changed(out: *const PropertyHandleOpaque) {
    let inner = &*(out as *const PropertyHandle);
    inner.clone().mark_dirty();
    inner.borrow_mut().dirty = false;
    inner.borrow_mut().binding = None;
}

/// Set a binding
/// The binding has signature fn(user_data, context, pointer_to_value)
///
/// The current implementation will do usually two memory alocation:
///  1. the allocation from the calling code to allocate user_data
///  2. the box allocation within this binding
/// It might be possible to reduce that by passing something with a
/// vtable, so there is the need for less memory allocation.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_binding(
    out: *const PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    let inner = &*(out as *const PropertyHandle);

    struct UserData {
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for UserData {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }
    let ud = UserData { user_data, drop_user_data };

    let real_binding = move |ptr: *mut (), ctx: &EvaluationContext| {
        binding(ud.user_data, ctx, ptr);
    };
    inner.borrow_mut().binding = Some(Box::new(real_binding));
    inner.clone().mark_dirty();
}

/// Destroy handle
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_drop(handle: *mut PropertyHandleOpaque) {
    core::ptr::read(handle as *mut PropertyHandle);
}
