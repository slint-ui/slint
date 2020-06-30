/*!
    Property binding engine.

    The current implementation uses lots of heap allocation but that can be optimized later using
    thin dst container, and intrusive linked list
*/

use crate::abi::datastructures::Color;
use crate::abi::primitives::PropertyAnimation;
use crate::ComponentRefPin;
use core::cell::*;
use core::ops::DerefMut;
use core::pin::Pin;
use std::rc::{Rc, Weak};

scoped_tls_hkt::scoped_thread_local!(static CURRENT_BINDING : Rc<dyn PropertyNotify>);

/// A binding trait object can be used to dynamically produces values for a property.
pub trait Binding<T> {
    /// This function is called by the property to evaluate the binding and produce a new value. The
    /// previous property value is provided in the value parameter.
    fn evaluate(self: Rc<Self>, value: &mut T, context: &EvaluationContext);

    /// This function is used to notify the binding that one of the dependencies was changed
    /// and therefore this binding may evaluate to a different value, too.
    fn mark_dirty(self: Rc<Self>, _reason: DirtyReason) {}

    /// This function allows the property to query the binding if it is still needed. This is
    /// primarily used for value animation bindings to indicate that they are no longer needed.
    fn keep_alive(self: Rc<Self>) -> bool {
        return true;
    }
}

#[derive(Default)]
struct PropertyImpl<T> {
    /// Invariant: Must only be called with a pointer to the binding
    binding: Option<Rc<dyn Binding<T>>>,
    dependencies: Vec<Weak<dyn PropertyNotify>>,
    dirty: bool,
    //updating: bool,
}

/// DirtyReason is used to convey to a dependency the reason for the request to
/// mark itself as dirty.
pub enum DirtyReason {
    /// The dependency shall be considered dirty because a property's value or
    /// subsequent dependency has changed.
    ValueOrDependencyHasChanged,
    /// The dependency shall be considered dirty because a property's binding
    /// has actively changed. This is typically used by animations to trigger
    /// a change.
    BindingHasChanged,
}

/// PropertyNotify is the interface that allows keeping track of dependencies between
/// property bindings.
pub trait PropertyNotify {
    /// mark_dirty() is called to notify a property that its binding may need to be re-evaluated
    /// because one of its dependencies may have changed.
    fn mark_dirty(self: Rc<Self>, reason: DirtyReason);
    /// notify() is called to register the currently (thread-local) evaluating binding as a
    /// dependency for this property (self).
    fn register_current_binding_as_dependency(self: Rc<Self>);
}

impl<T> PropertyNotify for RefCell<PropertyImpl<T>> {
    fn mark_dirty(self: Rc<Self>, reason: DirtyReason) {
        let mut v = vec![];
        {
            let mut dep = self.borrow_mut();
            dep.dirty = true;
            if let Some(binding) = &dep.binding {
                binding.clone().mark_dirty(reason);
            }
            std::mem::swap(&mut dep.dependencies, &mut v);
        }
        for d in v {
            if let Some(d) = d.upgrade() {
                d.mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
            }
        }
    }

    fn register_current_binding_as_dependency(self: Rc<Self>) {
        if CURRENT_BINDING.is_set() {
            CURRENT_BINDING.with(|cur_dep| {
                self.borrow_mut().dependencies.push(Rc::downgrade(cur_dep));
            });
        }
    }
}

impl<T> PropertyImpl<T> {
    fn set_binding(imp: Rc<RefCell<Self>>, binding: Option<Rc<dyn Binding<T>>>) {
        imp.borrow_mut().binding = binding;
        imp.clone().mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
    }
}

/// This structure contains what is required for the property engine to evaluate properties
///
/// One must pass it to the getter of the property, or emit of signals, and it can
/// be accessed from the bindings
#[repr(C)]
pub struct EvaluationContext<'a> {
    /// The component which contains the Property or the Signal
    pub component: core::pin::Pin<vtable::VRef<'a, crate::abi::datastructures::ComponentVTable>>,

    /// The context of the parent component
    pub parent_context: Option<&'a EvaluationContext<'a>>,
}

impl<'a> EvaluationContext<'a> {
    /// Create a new context related to the root component
    ///
    /// The component need to be a root component, otherwise fetching properties
    /// might panic.
    pub fn for_root_component(component: ComponentRefPin<'a>) -> Self {
        Self { component, parent_context: None }
    }

    /// Create a context for a child component of a component within the current
    /// context.
    pub fn child_context(&'a self, child: ComponentRefPin<'a>) -> Self {
        Self { component: child, parent_context: Some(self) }
    }

    /// Attempt to cast the component to the given type
    pub fn get_component<
        T: vtable::HasStaticVTable<crate::abi::datastructures::ComponentVTable>,
    >(
        &'a self,
    ) -> Option<core::pin::Pin<&'a T>> {
        vtable::VRef::downcast_pin(self.component)
    }
}

type PropertyHandle<T> = Rc<RefCell<PropertyImpl<T>>>;
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
    inner: PropertyHandle<T>,
    /// Only access when holding a lock of the inner refcell.
    /// (so only through Property::borrow and Property::try_borrow_mut)
    value: UnsafeCell<T>,
}

impl<T> Property<T> {
    /// Borrow both `inner` and `value`
    fn try_borrow(&self) -> Result<(Ref<PropertyImpl<T>>, Ref<T>), BorrowError> {
        let lock = self.inner.try_borrow()?;
        // Safety: we use the same locking rules for `inner` and `value`
        Ok(Ref::map_split(lock, |r| unsafe { (r, &*self.value.get()) }))
    }

    /// Borrow both `inner` and `value` as mutable
    fn try_borrow_mut(&self) -> Result<(RefMut<PropertyImpl<T>>, RefMut<T>), BorrowMutError> {
        let lock = self.inner.try_borrow_mut()?;
        // Safety: we use the same locking rules for `inner` and `value`
        Ok(RefMut::map_split(lock, |r| unsafe { (r, &mut *self.value.get()) }))
    }
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
    pub fn get(self: Pin<&Self>, context: &EvaluationContext) -> T {
        self.update(context);
        self.inner.clone().register_current_binding_as_dependency();
        self.try_borrow().expect("Binding loop detected").1.clone()
    }

    /// Change the value of this property
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set(&self, t: T) {
        {
            let (mut lock, mut value) = self.try_borrow_mut().expect("Binding loop detected");
            lock.binding = None;
            lock.dirty = false;
            *value = t;
        }
        self.inner.clone().mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
        self.inner.borrow_mut().dirty = false;
    }

    /// Set a binding to this property.
    ///
    /// Bindings are evaluated lazily from calling get, and the return value of the binding
    /// is the new value.
    ///
    /// If other properties have bindings depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_binding(&self, f: impl (Fn(&EvaluationContext) -> T) + 'static) {
        let binding_object = Property::make_binding(f);
        PropertyImpl::set_binding(self.inner.clone(), Some(binding_object));
    }

    fn make_binding(f: impl (Fn(&EvaluationContext) -> T) + 'static) -> Rc<dyn Binding<T>> {
        struct BindingFunction<F> {
            function: F,
        }

        impl<T, F: Fn(&mut T, &EvaluationContext)> Binding<T> for BindingFunction<F> {
            fn evaluate(self: Rc<Self>, value_ptr: &mut T, context: &EvaluationContext) {
                (self.function)(value_ptr, context)
            }
        }

        let real_binding = move |ptr: &mut T, context: &EvaluationContext| *ptr = f(context);

        Rc::new(BindingFunction { function: real_binding })
    }

    /// Call the binding if the property is dirty to update the stored value
    fn update(&self, context: &EvaluationContext) {
        if !self.inner.borrow().dirty {
            return;
        }
        let (mut lock, mut value) =
            self.try_borrow_mut().expect("Circular dependency in binding evaluation");
        if let Some(binding) = &lock.binding {
            CURRENT_BINDING.set(&(self.inner.clone() as Rc<dyn PropertyNotify>), || {
                binding.clone().evaluate(value.deref_mut(), context);
            });
            if !binding.clone().keep_alive() {
                lock.binding = None;
            }
            lock.dirty = false;
        }
    }
}

impl<T: Clone + InterpolatedPropertyValue + 'static> Property<T> {
    /// Set a binding to this property.
    ///
    /// Bindings are evaluated lazily from calling get, and the return value of the binding
    /// is the new value. Any new values reported by the binding are animated (interpolated) according to the
    /// parameters described by the PropertyAnimation object.
    ///
    /// If other properties have bindings depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_binding(
        &self,
        f: impl (Fn(&EvaluationContext) -> T) + 'static,
        animation_data: &PropertyAnimation,
    ) -> Rc<RefCell<PropertyAnimationBinding<T>>> {
        let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_binding(
            Property::make_binding(f),
            animation_data,
            self.inner.clone(),
        )));
        PropertyImpl::set_binding(self.inner.clone(), Some(animation.clone()));
        animation
    }

    /// Change the value of this property, by animating (interpolating) from the current property's value
    /// to the specified parameter value. The animation is done according to the parameters described by
    /// the PropertyAnimation object.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_value(
        &self,
        value: T,
        animation_data: &PropertyAnimation,
    ) -> Rc<RefCell<PropertyAnimationBinding<T>>> {
        let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_value(
            value,
            animation_data,
            self.inner.clone(),
        )));
        PropertyImpl::set_binding(self.inner.clone(), Some(animation.clone()));
        animation
    }
}

#[test]
fn properties_simple_test() {
    fn g(prop: &Property<i32>, ctx: &EvaluationContext) -> i32 {
        unsafe { Pin::new_unchecked(prop).get(ctx) }
    }

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        height: Property<i32>,
        area: Property<i32>,
    }
    let dummy_eval_context = EvaluationContext::for_root_component(unsafe {
        core::pin::Pin::new_unchecked(vtable::VRef::from_raw(
            core::ptr::NonNull::dangling(),
            core::ptr::NonNull::dangling(),
        ))
    });
    let compo = Rc::new(Component::default());
    let w = Rc::downgrade(&compo);
    compo.area.set_binding(move |ctx| {
        let compo = w.upgrade().unwrap();
        g(&compo.width, ctx) * g(&compo.height, ctx)
    });
    compo.width.set(4);
    compo.height.set(8);
    assert_eq!(g(&compo.width, &dummy_eval_context), 4);
    assert_eq!(g(&compo.height, &dummy_eval_context), 8);
    assert_eq!(g(&compo.area, &dummy_eval_context), 4 * 8);

    let w = Rc::downgrade(&compo);
    compo.width.set_binding(move |ctx| {
        let compo = w.upgrade().unwrap();
        g(&compo.height, ctx) * 2
    });
    assert_eq!(g(&compo.width, &dummy_eval_context), 8 * 2);
    assert_eq!(g(&compo.height, &dummy_eval_context), 8);
    assert_eq!(g(&compo.area, &dummy_eval_context), 8 * 8 * 2);
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
        core::mem::size_of::<PropertyHandle<()>>(),
        core::mem::size_of::<PropertyHandleOpaque>()
    );
    // This assume that PropertyHandle<()> has the same layout as PropertyHandle<T> ∀T
    core::ptr::write(out as *mut PropertyHandle<()>, PropertyHandle::default());
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
    let inner = &*(out as *const PropertyHandle<()>);

    if !inner.borrow().dirty {
        inner.clone().register_current_binding_as_dependency();
        return;
    }
    let mut lock = inner.try_borrow_mut().expect("Circular dependency in binding evaluation");
    if let Some(binding) = &lock.binding {
        CURRENT_BINDING.set(&(inner.clone() as Rc<dyn PropertyNotify>), || {
            binding.clone().evaluate(&mut *val, &*context);
        });
        if !binding.clone().keep_alive() {
            lock.binding = None;
        }
        lock.dirty = false;
    }
    core::mem::drop(lock);
    inner.clone().register_current_binding_as_dependency();
}

/// Mark the fact that the property was changed and that its binding need to be removed, and
/// The dependencies marked dirty
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_changed(out: *const PropertyHandleOpaque) {
    let inner = &*(out as *const PropertyHandle<()>);
    inner.clone().mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
    inner.borrow_mut().dirty = false;
    inner.borrow_mut().binding = None;
}

fn make_c_function_binding<T: 'static>(
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut T),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) -> Rc<dyn Binding<T>> {
    struct CFunctionBinding<T> {
        binding_function: extern "C" fn(*mut c_void, &EvaluationContext, *mut T),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl<T> Drop for CFunctionBinding<T> {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl<T> Binding<T> for CFunctionBinding<T> {
        fn evaluate(self: Rc<Self>, value_ptr: &mut T, context: &EvaluationContext) {
            (self.binding_function)(self.user_data, context, value_ptr);
        }
    }

    Rc::new(CFunctionBinding { binding_function: binding, user_data, drop_user_data })
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
    let inner = &*(out as *const PropertyHandle<()>);

    let binding = make_c_function_binding(binding, user_data, drop_user_data);

    inner.borrow_mut().binding = Some(binding);
    inner.clone().mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
}

/// Destroy handle
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_drop(handle: *mut PropertyHandleOpaque) {
    core::ptr::read(handle as *mut PropertyHandle<()>);
}

/// InterpolatedPropertyValue is a trait used to enable properties to be used with
/// animations that interpolate values. The basic requirement is the ability to apply
/// a progress that's typically between 0 and 1 to a range.
pub trait InterpolatedPropertyValue:
    PartialEq + Clone + Copy + std::fmt::Display + Default + 'static
{
    /// Returns the interpolated value between self and target_value according to the
    /// progress parameter t that's usually between 0 and 1. With certain animation
    /// easing curves it may over- or undershoot though.
    fn interpolate(self, target_value: Self, t: f32) -> Self;
}

impl InterpolatedPropertyValue for f32 {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        self + t * (target_value - self)
    }
}

impl InterpolatedPropertyValue for i32 {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32) as i32
    }
}

impl InterpolatedPropertyValue for u8 {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        ((self as f32) + (t * ((target_value as f32) - (self as f32)))).min(255.).max(0.) as u8
    }
}

#[derive(Default)]
/// PropertyAnimationBinding provides a linear animation of values of a property, when they are changed
/// through bindings or direct set() calls.
pub struct PropertyAnimationBinding<T: InterpolatedPropertyValue> {
    dirty: bool,
    current_property_value: T,
    current_animated_value: Option<T>,
    from_value: T,
    to_value: T,
    notify: Option<Weak<dyn PropertyNotify>>,
    binding: Option<Rc<dyn Binding<T>>>,
    animation_handle: Option<crate::animations::AnimationHandle>,
    details: crate::abi::primitives::PropertyAnimation,
    keep_alive: bool,
}

impl<T: InterpolatedPropertyValue> crate::abi::properties::Binding<T>
    for RefCell<PropertyAnimationBinding<T>>
{
    fn evaluate(self: Rc<Self>, value: &mut T, context: &crate::EvaluationContext) {
        let mut this = self.borrow_mut();
        if this.dirty {
            if let Some(binding) = &this.binding {
                let mut new_value = this.to_value;
                binding.clone().evaluate(&mut new_value, context);
                this.current_property_value = new_value;
            } else if this.current_animated_value.is_none() {
                this.current_animated_value = Some(*value);
            }
            this.dirty = false;

            if this.current_animated_value.is_none() {
                this.from_value = this.current_property_value;
                this.to_value = this.current_property_value;
                this.current_animated_value = Some(this.current_property_value);
            } else if this.current_property_value != this.current_animated_value.unwrap() {
                let driver =
                    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.clone());
                match this.animation_handle {
                    Some(handle) => driver.borrow_mut().restart_animation(handle),
                    None => {
                        this.animation_handle = Some(driver.borrow_mut().start_animation(
                            Rc::downgrade(&(self.clone() as Rc<dyn crate::animations::Animated>)),
                        ));
                    }
                }

                this.from_value = this.current_animated_value.unwrap();
                this.to_value = this.current_property_value;
            }
        }

        *value = this.current_animated_value.unwrap_or_default();
    }

    fn mark_dirty(self: Rc<Self>, reason: DirtyReason) {
        if matches!(reason, DirtyReason::ValueOrDependencyHasChanged) {
            self.borrow_mut().dirty = true;
        }
    }

    fn keep_alive(self: Rc<Self>) -> bool {
        return self.borrow().keep_alive;
    }
}

impl<T: InterpolatedPropertyValue> crate::animations::Animated
    for RefCell<PropertyAnimationBinding<T>>
{
    fn update_animation_state(self: Rc<Self>, state: crate::animations::AnimationState) {
        use crate::animations::*;
        // This shouldn't really happen... the animation should only be started if we have a valid animated value
        // to begin with.
        if self.borrow().current_animated_value.is_none() {
            return;
        }

        match state {
            AnimationState::Started => {}
            AnimationState::Running { progress } => {
                //println!("progress {}", progress * 100.);
                {
                    let mut this = self.borrow_mut();
                    this.current_animated_value =
                        Some(this.from_value.interpolate(this.to_value, progress))
                }
                if let Some(notify) =
                    &self.borrow().notify.as_ref().and_then(|weak_notify| weak_notify.upgrade())
                {
                    notify.clone().mark_dirty(DirtyReason::BindingHasChanged);
                }
            }
            AnimationState::Stopped => {
                {
                    let mut this = self.borrow_mut();
                    // When animating merely by value, then we might as well indicate to the property that we're no longer
                    // needed.
                    if this.binding.is_none() {
                        this.keep_alive = false;
                    }
                }
                if let Some(notify) =
                    &self.borrow().notify.as_ref().and_then(|weak_notify| weak_notify.upgrade())
                {
                    notify.clone().mark_dirty(DirtyReason::BindingHasChanged);
                }
            }
        }
    }
    fn duration(self: Rc<Self>) -> std::time::Duration {
        std::time::Duration::from_millis(self.borrow().details.duration as _)
    }
}

impl<T: InterpolatedPropertyValue> PropertyAnimationBinding<T> {
    /// Creates a new property animation that is set up to animate to the specified target value.
    pub fn new_with_value(
        target_value: T,
        animation_data: &crate::abi::primitives::PropertyAnimation,
        notifier: Rc<dyn PropertyNotify>,
    ) -> Self {
        let mut this: PropertyAnimationBinding<T> = Default::default();
        this.keep_alive = true;
        this.details = animation_data.clone();
        this.current_property_value = target_value;
        this.notify = Some(Rc::downgrade(&notifier));
        this
    }
    /// Creates a new property animation that is set up to animate between the values produced
    /// by the give binding function.
    pub fn new_with_binding(
        binding: Rc<dyn Binding<T>>,
        animation_data: &crate::abi::primitives::PropertyAnimation,
        notifier: Rc<dyn PropertyNotify>,
    ) -> Self {
        let mut this: PropertyAnimationBinding<T> = Default::default();
        this.keep_alive = true;
        this.details = animation_data.clone();
        this.binding = Some(binding);
        this.notify = Some(Rc::downgrade(&notifier));
        this
    }
}

impl<T: InterpolatedPropertyValue> Drop for PropertyAnimationBinding<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.animation_handle {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.borrow_mut().free_animation(handle));
        }
    }
}

/// Internal function to set up a property animation to the specified target value for an integer property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_value_int(
    out: *const PropertyHandleOpaque,
    value: i32,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let inner = &*(out as *const PropertyHandle<i32>);
    let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_value(
        value,
        animation_data,
        inner.clone(),
    )));

    PropertyImpl::set_binding(inner.clone(), Some(animation.clone()));
}

/// Internal function to set up a property animation to the specified target value for a float property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_value_float(
    out: *const PropertyHandleOpaque,
    value: f32,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let inner = &*(out as *const PropertyHandle<f32>);
    let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_value(
        value,
        animation_data,
        inner.clone(),
    )));

    PropertyImpl::set_binding(inner.clone(), Some(animation.clone()));
}

/// Internal function to set up a property animation to the specified target value for a color property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_value_color(
    out: *const PropertyHandleOpaque,
    value: &Color,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let inner = &*(out as *const PropertyHandle<Color>);
    let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_value(
        *value,
        animation_data,
        inner.clone(),
    )));

    PropertyImpl::set_binding(inner.clone(), Some(animation.clone()));
}

/// Internal function to set up a property animation between values produced by the specified binding for an integer property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_binding_int(
    out: *const PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut i32),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let inner = &*(out as *const PropertyHandle<i32>);

    let binding = make_c_function_binding(binding, user_data, drop_user_data);

    let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_binding(
        binding,
        animation_data,
        inner.clone(),
    )));
    PropertyImpl::set_binding(inner.clone(), Some(animation.clone()));
}

/// Internal function to set up a property animation between values produced by the specified binding for a float property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_binding_float(
    out: *const PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut f32),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let inner = &*(out as *const PropertyHandle<f32>);

    let binding = make_c_function_binding(binding, user_data, drop_user_data);

    let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_binding(
        binding,
        animation_data,
        inner.clone(),
    )));
    PropertyImpl::set_binding(inner.clone(), Some(animation.clone()));
}

/// Internal function to set up a property animation between values produced by the specified binding for a color property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_binding_color(
    out: *const PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut Color),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let inner = &*(out as *const PropertyHandle<Color>);

    let binding = make_c_function_binding(binding, user_data, drop_user_data);

    let animation = Rc::new(RefCell::new(PropertyAnimationBinding::new_with_binding(
        binding,
        animation_data,
        inner.clone(),
    )));
    PropertyImpl::set_binding(inner.clone(), Some(animation.clone()));
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::abi::primitives::PropertyAnimation;
    use crate::animations::*;

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        width_times_two: Property<i32>,
        feed_property: Property<i32>, // used by binding to feed values into width
    }

    #[test]
    fn properties_test_animation_triggered_by_set() {
        fn g(prop: &Property<i32>, ctx: &EvaluationContext) -> i32 {
            unsafe { Pin::new_unchecked(prop).get(ctx) }
        }
        let dummy_eval_context = EvaluationContext {
            component: unsafe {
                core::pin::Pin::new_unchecked(vtable::VRef::from_raw(
                    core::ptr::NonNull::dangling(),
                    core::ptr::NonNull::dangling(),
                ))
            },
            parent_context: None,
        };
        let compo = Rc::new(test::Component::default());

        let w = Rc::downgrade(&compo);
        compo.width_times_two.set_binding(move |context| {
            let compo = w.upgrade().unwrap();
            g(&compo.width, context) * 2
        });

        let animation_details = PropertyAnimation { duration: 10000 };

        compo.width.set(100);
        assert_eq!(g(&compo.width, &dummy_eval_context), 100);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 200);

        let animation = compo.width.set_animated_value(200, &animation_details);
        assert_eq!(g(&compo.width, &dummy_eval_context), 100);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 200);
        assert_eq!(animation.borrow().from_value, 100);
        assert_eq!(animation.borrow().to_value, 200);

        animation.clone().update_animation_state(AnimationState::Running { progress: 0.5 });
        assert_eq!(g(&compo.width, &dummy_eval_context), 150);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 300);

        animation.clone().update_animation_state(AnimationState::Running { progress: 1.0 });
        assert_eq!(g(&compo.width, &dummy_eval_context), 200);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 400);

        animation.clone().update_animation_state(AnimationState::Stopped);
        assert_eq!(g(&compo.width, &dummy_eval_context), 200);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 400);

        assert_eq!(Rc::strong_count(&animation), 1);
    }

    #[test]
    fn properties_test_animation_triggered_by_binding() {
        fn g(prop: &Property<i32>, ctx: &EvaluationContext) -> i32 {
            unsafe { Pin::new_unchecked(prop).get(ctx) }
        }
        let dummy_eval_context = EvaluationContext {
            component: unsafe {
                core::pin::Pin::new_unchecked(vtable::VRef::from_raw(
                    core::ptr::NonNull::dangling(),
                    core::ptr::NonNull::dangling(),
                ))
            },
            parent_context: None,
        };
        let compo = Rc::new(test::Component::default());

        let w = Rc::downgrade(&compo);
        compo.width_times_two.set_binding(move |context| {
            let compo = w.upgrade().unwrap();
            g(&compo.width, context) * 2
        });

        let w = Rc::downgrade(&compo);

        let animation_details = PropertyAnimation { duration: 10000 };

        let animation = compo.width.set_animated_binding(
            move |context| {
                let compo = w.upgrade().unwrap();
                g(&compo.feed_property, context)
            },
            &animation_details,
        );

        compo.feed_property.set(100);
        assert_eq!(g(&compo.width, &dummy_eval_context), 100);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 200);

        compo.feed_property.set(200);
        assert_eq!(g(&compo.width, &dummy_eval_context), 100);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 200);

        animation.clone().update_animation_state(AnimationState::Running { progress: 0.5 });

        assert_eq!(g(&compo.width, &dummy_eval_context), 150);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 300);
        assert_eq!(animation.borrow().from_value, 100);
        assert_eq!(animation.borrow().to_value, 200);

        animation.clone().update_animation_state(AnimationState::Running { progress: 1.0 });

        assert_eq!(g(&compo.width, &dummy_eval_context), 200);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 400);
    }
}
