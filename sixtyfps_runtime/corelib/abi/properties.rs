/*!
    Property binding engine.

    The current implementation uses lots of heap allocation but that can be optimized later using
    thin dst container, and intrusive linked list
*/

use crate::ComponentRefPin;
use core::cell::*;
use core::ops::DerefMut;
use std::rc::{Rc, Weak};

thread_local!(static CURRENT_BINDING : RefCell<Option<Rc<dyn PropertyNotify>>> = Default::default());

trait Binding<T> {
    fn evaluate(self: Rc<Self>, value: &mut T, context: &EvaluationContext);

    /// This function is used to notify the binding that one of the dependencies was changed
    /// and therefore this binding may evaluate to a different value, too.
    fn mark_dirty(self: Rc<Self>, _reason: DirtyReason) {}

    fn set_notify_callback(self: Rc<Self>, _callback: Rc<dyn PropertyNotify>) {}
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
enum DirtyReason {
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
trait PropertyNotify {
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
        for d in &v {
            if let Some(d) = d.upgrade() {
                d.mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
            }
        }
    }

    fn register_current_binding_as_dependency(self: Rc<Self>) {
        CURRENT_BINDING.with(|cur_dep| {
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
    pub fn get(&self, context: &EvaluationContext) -> T {
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
        self.set_binding_object(binding_object);
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

    /// Set a binding object to this property.
    ///
    /// Bindings are evaluated lazily from calling get, and the return value of the binding
    /// is the new value.
    ///
    /// If other properties have bindings depending of this property, these properties will
    /// be marked as dirty.
    fn set_binding_object(&self, binding_object: Rc<dyn Binding<T>>) -> Option<Rc<dyn Binding<T>>> {
        binding_object.clone().set_notify_callback(self.inner.clone());
        let old_binding =
            std::mem::replace(&mut self.inner.borrow_mut().binding, Some(binding_object));
        self.inner.clone().mark_dirty(DirtyReason::ValueOrDependencyHasChanged);
        old_binding
    }

    /// Call the binding if the property is dirty to update the stored value
    fn update(&self, context: &EvaluationContext) {
        if !self.inner.borrow().dirty {
            return;
        }
        let mut old: Option<Rc<dyn PropertyNotify>> = Some(self.inner.clone());
        let (mut lock, mut value) =
            self.try_borrow_mut().expect("Circular dependency in binding evaluation");
        if let Some(binding) = &lock.binding {
            CURRENT_BINDING.with(|cur_dep| {
                let mut m = cur_dep.borrow_mut();
                std::mem::swap(m.deref_mut(), &mut old);
            });
            binding.clone().evaluate(value.deref_mut(), context);
            lock.dirty = false;
            CURRENT_BINDING.with(|cur_dep| {
                let mut m = cur_dep.borrow_mut();
                std::mem::swap(m.deref_mut(), &mut old);
                //somehow ptr_eq does not work as expected despite the pointer are equal
                //debug_assert!(Rc::ptr_eq(&(self.inner.clone() as Rc<dyn PropertyNotify>), &old.unwrap()));
            });
        }
    }
}

impl<T: Clone + InterpolatedPropertyValue + 'static> Property<T> {
    /// Set a binding to this property.
    ///
    /// Bindings are evaluated lazily from calling get, and the return value of the binding
    /// is the new value. Any new values reported by the binding are animated (interpolated) according to the
    /// parameters described by the PropertyAnimationData object.
    ///
    /// If other properties have bindings depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_binding(
        &self,
        f: impl (Fn(&EvaluationContext) -> T) + 'static,
        animation_data: &crate::abi::primitives::PropertyAnimation,
    ) -> Rc<RefCell<PropertyAnimation<T>>> {
        let animation =
            Rc::new(RefCell::new(PropertyAnimation::new_with_binding(f, animation_data)));
        self.set_binding_object(animation.clone());
        animation
    }

    /// Change the value of this property, by animating (interpolating) from the current property's value
    /// to the specified parameter value. The animation is done according to the parameters described by
    /// the PropertyAnimationData object.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_value(
        &self,
        value: T,
        animation_data: &crate::abi::primitives::PropertyAnimation,
    ) -> Rc<RefCell<PropertyAnimation<T>>> {
        let animation =
            Rc::new(RefCell::new(PropertyAnimation::new_with_value(value, animation_data)));
        self.set_binding_object(animation.clone());
        animation
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
    let mut old: Option<Rc<dyn PropertyNotify>> = Some(inner.clone());
    let mut lock = inner.try_borrow_mut().expect("Circular dependency in binding evaluation");
    if let Some(binding) = &lock.binding {
        CURRENT_BINDING.with(|cur_dep| {
            let mut m = cur_dep.borrow_mut();
            std::mem::swap(m.deref_mut(), &mut old);
        });
        binding.clone().evaluate(&mut *val, &*context);
        lock.dirty = false;
        CURRENT_BINDING.with(|cur_dep| {
            let mut m = cur_dep.borrow_mut();
            std::mem::swap(m.deref_mut(), &mut old);
            //somehow ptr_eq does not work as expected despite the pointer are equal
            //debug_assert!(Rc::ptr_eq(&(inner.clone() as Rc<dyn PropertyNotify>), &old.unwrap()));
        });
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

    struct CFunctionBinding {
        binding_function: extern "C" fn(*mut c_void, &EvaluationContext, *mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for CFunctionBinding {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl Binding<()> for CFunctionBinding {
        fn evaluate(self: Rc<Self>, value_ptr: &mut (), context: &EvaluationContext) {
            (self.binding_function)(self.user_data, context, value_ptr);
        }
    }

    let binding =
        Rc::new(CFunctionBinding { binding_function: binding, user_data, drop_user_data });

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

#[derive(Default)]
/// PropertyAnimation provides a linear animation of values of a property, when they are changed
/// through bindings or direct set() calls.
pub struct PropertyAnimation<T: InterpolatedPropertyValue> {
    dirty: bool,
    current_property_value: T,
    current_animated_value: Option<T>,
    from_value: T,
    to_value: T,
    notify: Option<Weak<dyn PropertyNotify>>,
    binding: Option<Rc<dyn Binding<T>>>,
    animation_handle: Option<crate::animations::AnimationHandle>,
    details: crate::abi::primitives::PropertyAnimation,
}

impl<T: InterpolatedPropertyValue> crate::abi::properties::Binding<T>
    for RefCell<PropertyAnimation<T>>
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

    fn set_notify_callback(self: Rc<Self>, notifier: Rc<dyn PropertyNotify>) {
        self.borrow_mut().notify = Some(Rc::downgrade(&notifier));
    }
}

impl<T: InterpolatedPropertyValue> crate::animations::Animated for RefCell<PropertyAnimation<T>> {
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
            AnimationState::Stopped => {}
        }
    }
    fn duration(self: Rc<Self>) -> std::time::Duration {
        std::time::Duration::from_millis(self.borrow().details.duration as _)
    }
}

impl<T: InterpolatedPropertyValue> PropertyAnimation<T> {
    /// Creates a new property animation that is set up to animate to the specified target value.
    pub fn new_with_value(
        target_value: T,
        animation_data: &crate::abi::primitives::PropertyAnimation,
    ) -> Self {
        let mut this: PropertyAnimation<T> = Default::default();
        this.details = animation_data.clone();
        this.current_property_value = target_value;
        this
    }
    /// Creates a new property animation that is set up to animate between the values produced
    /// by the give binding function.
    pub fn new_with_binding(
        binding_function: impl (Fn(&EvaluationContext) -> T) + 'static,
        animation_data: &crate::abi::primitives::PropertyAnimation,
    ) -> Self {
        let mut this: PropertyAnimation<T> = Default::default();
        this.details = animation_data.clone();
        this.binding = Some(Property::make_binding(binding_function));
        this
    }
}

impl<T: InterpolatedPropertyValue> Drop for PropertyAnimation<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.animation_handle {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.borrow_mut().free_animation(handle));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::animations::*;

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        width_times_two: Property<i32>,
        feed_property: Property<i32>, // used by binding to feed values into width
    }

    #[test]
    fn properties_test_animation_triggered_by_set() {
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
            compo.width.get(context) * 2
        });

        let animation_details = crate::abi::primitives::PropertyAnimation { duration: 10000 };

        compo.width.set(100);
        assert_eq!(compo.width.get(&dummy_eval_context), 100);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 200);

        let animation = compo.width.set_animated_value(200, &animation_details);
        assert_eq!(compo.width.get(&dummy_eval_context), 100);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 200);
        assert_eq!(animation.borrow().from_value, 100);
        assert_eq!(animation.borrow().to_value, 200);

        animation.clone().update_animation_state(AnimationState::Running { progress: 0.5 });
        assert_eq!(compo.width.get(&dummy_eval_context), 150);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 300);

        animation.clone().update_animation_state(AnimationState::Running { progress: 1.0 });
        assert_eq!(compo.width.get(&dummy_eval_context), 200);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 400);
    }

    #[test]
    fn properties_test_animation_triggered_by_binding() {
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
            compo.width.get(context) * 2
        });

        let w = Rc::downgrade(&compo);

        let animation_details = crate::abi::primitives::PropertyAnimation { duration: 10000 };

        let animation = compo.width.set_animated_binding(
            move |context| {
                let compo = w.upgrade().unwrap();
                compo.feed_property.get(context)
            },
            &animation_details,
        );

        compo.feed_property.set(100);
        assert_eq!(compo.width.get(&dummy_eval_context), 100);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 200);

        compo.feed_property.set(200);
        assert_eq!(compo.width.get(&dummy_eval_context), 100);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 200);

        animation.clone().update_animation_state(AnimationState::Running { progress: 0.5 });

        assert_eq!(compo.width.get(&dummy_eval_context), 150);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 300);
        assert_eq!(animation.borrow().from_value, 100);
        assert_eq!(animation.borrow().to_value, 200);

        animation.clone().update_animation_state(AnimationState::Running { progress: 1.0 });

        assert_eq!(compo.width.get(&dummy_eval_context), 200);
        assert_eq!(compo.width_times_two.get(&dummy_eval_context), 400);
    }
}
