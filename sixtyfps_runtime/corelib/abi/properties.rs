/*!
    Property binding engine.

    The current implementation uses lots of heap allocation but that can be optimized later using
    thin dst container, and intrusive linked list
*/

mod single_linked_list_pin {
    ///! A singled linked list whose nodes are pinned
    use core::pin::Pin;
    type NodePtr<T> = Option<Pin<Box<SingleLinkedListPinNode<T>>>>;
    struct SingleLinkedListPinNode<T> {
        next: NodePtr<T>,
        value: T,
    }

    pub struct SingleLinkedListPinHead<T>(NodePtr<T>);
    impl<T> Default for SingleLinkedListPinHead<T> {
        fn default() -> Self {
            Self(None)
        }
    }

    impl<T> SingleLinkedListPinHead<T> {
        pub fn push_front(&mut self, value: T) -> Pin<&T> {
            self.0 = Some(Box::pin(SingleLinkedListPinNode { next: self.0.take(), value }));
            // Safety: we can project from SingleLinkedListPinNode
            unsafe { Pin::new_unchecked(&self.0.as_ref().unwrap().value) }
        }

        #[allow(unused)]
        pub fn iter<'a>(&'a self) -> impl Iterator<Item = Pin<&T>> + 'a {
            struct I<'a, T>(&'a NodePtr<T>);

            impl<'a, T> Iterator for I<'a, T> {
                type Item = Pin<&'a T>;
                fn next(&mut self) -> Option<Self::Item> {
                    if let Some(x) = &self.0 {
                        let r = unsafe { Pin::new_unchecked(&x.value) };
                        self.0 = &x.next;
                        Some(r)
                    } else {
                        None
                    }
                }
            }
            I(&self.0)
        }
    }

    #[test]
    fn test_list() {
        let mut head = SingleLinkedListPinHead::default();
        head.push_front(1);
        head.push_front(2);
        head.push_front(3);
        assert_eq!(
            head.iter().map(|x: Pin<&i32>| *x.get_ref()).collect::<Vec<i32>>(),
            vec![3, 2, 1]
        );
    }
}

use core::cell::{Cell, RefCell, UnsafeCell};
use core::pin::Pin;

use crate::abi::datastructures::Color;
use crate::abi::primitives::PropertyAnimation;
use crate::ComponentRefPin;

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

/// The return value of a binding
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum BindingResult {
    /// The binding is a normal binding, and we keep it to re-evaluate it ince it is dirty
    KeepBinding,
    /// The value of the property is now constant after the binding was evaluated, so
    /// the binding can be removed.
    RemoveBinding,
}

struct BindingVTable {
    drop: unsafe fn(_self: *mut BindingHolder),
    evaluate: unsafe fn(
        _self: *mut BindingHolder,
        value: *mut (),
        context: &EvaluationContext,
    ) -> BindingResult,
    mark_dirty: unsafe fn(_self: *const BindingHolder),
}

/// A binding trait object can be used to dynamically produces values for a property.
trait BindingCallable {
    /// This function is called by the property to evaluate the binding and produce a new value. The
    /// previous property value is provided in the value parameter.
    unsafe fn evaluate(
        self: Pin<&Self>,
        value: *mut (),
        context: &EvaluationContext,
    ) -> BindingResult;

    /// This function is used to notify the binding that one of the dependencies was changed
    /// and therefore this binding may evaluate to a different value, too.
    fn mark_dirty(self: Pin<&Self>) {}
}

impl<F: Fn(*mut (), &EvaluationContext) -> BindingResult> BindingCallable for F {
    unsafe fn evaluate(
        self: Pin<&Self>,
        value: *mut (),
        context: &EvaluationContext,
    ) -> BindingResult {
        self(value, context)
    }
}

scoped_tls_hkt::scoped_thread_local!(static mut CURRENT_BINDING : for<'a> Pin<&'a mut BindingHolder>);

impl<'a, 'b: 'a> scoped_tls_hkt::ReborrowMut<'a> for Pin<&'b mut BindingHolder> {
    type Result = Pin<&'a mut BindingHolder>;
    fn reborrow_mut(&'a mut self) -> Self::Result {
        self.as_mut()
    }
}

#[repr(C)]
struct BindingHolder<B = ()> {
    /// Access to the list of binding which depends on this binding
    dependencies: Cell<usize>,
    /// The binding own the nodes used in the dependencies lists of the properties
    /// From which we depend.
    dep_nodes: single_linked_list_pin::SingleLinkedListPinHead<DependencyNode>,
    vtable: &'static BindingVTable,
    dirty: Cell<bool>,
    binding: B,
}

fn alloc_binding_holder<B: BindingCallable + 'static>(binding: B) -> *mut BindingHolder {
    /// Safety: _self must be a pointer that comes from a `Box<BindingHolder<B>>::into_raw()`
    unsafe fn binding_drop<B>(_self: *mut BindingHolder) {
        Box::from_raw(_self as *mut BindingHolder<B>);
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    /// and value must be a pointer to T
    unsafe fn evaluate<B: BindingCallable>(
        _self: *mut BindingHolder,
        value: *mut (),
        context: &EvaluationContext,
    ) -> BindingResult {
        let pinned_holder = Pin::new_unchecked(&mut *_self);
        CURRENT_BINDING.set(pinned_holder, || {
            Pin::new_unchecked(&((*(_self as *mut BindingHolder<B>)).binding))
                .evaluate(value, context)
        })
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    unsafe fn mark_dirty<B: BindingCallable>(_self: *const BindingHolder) {
        Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding)).mark_dirty()
    }

    trait HasBindingVTable {
        const VT: &'static BindingVTable;
    }
    impl<B: BindingCallable> HasBindingVTable for B {
        const VT: &'static BindingVTable = &BindingVTable {
            drop: binding_drop::<B>,
            evaluate: evaluate::<B>,
            mark_dirty: mark_dirty::<B>,
        };
    }

    let holder: BindingHolder<B> = BindingHolder {
        dependencies: Cell::new(0),
        dep_nodes: Default::default(),
        vtable: <B as HasBindingVTable>::VT,
        dirty: Cell::new(true), // starts dirty so it evaluates the property when used
        binding,
    };
    Box::into_raw(Box::new(holder)) as *mut BindingHolder
}

#[repr(transparent)]
struct DependencyListHead(Cell<usize>);

impl DependencyListHead {
    unsafe fn mem_move(from: *mut Self, to: *mut Self) {
        (*to).0.set((*from).0.get());
        if let Some(next) = ((*from).0.get() as *const DependencyNode).as_ref() {
            next.debug_assert_valid();
            next.prev.set(to as *const _);
            next.debug_assert_valid();
        }
    }
    unsafe fn drop(_self: *mut Self) {
        if let Some(next) = ((*_self).0.get() as *const DependencyNode).as_ref() {
            next.debug_assert_valid();
            next.prev.set(core::ptr::null());
            next.debug_assert_valid();
        }
    }
    unsafe fn append(_self: *mut Self, node: *const DependencyNode) {
        (*node).debug_assert_valid();
        let old = (*_self).0.get() as *const DependencyNode;
        old.as_ref().map(|x| x.debug_assert_valid());
        (*_self).0.set(node as usize);
        let node = &*node;
        node.next.set(old);
        node.prev.set(_self as *const _);
        if let Some(old) = old.as_ref() {
            old.prev.set((&node.next) as *const _);
            old.debug_assert_valid();
        }
        (*node).debug_assert_valid();
    }
}

/// The node is owned by the binding; so the binding is always valid
/// The next and pref
struct DependencyNode {
    next: Cell<*const DependencyNode>,
    /// This is either null, or a pointer to a pointer to ourself
    prev: Cell<*const Cell<*const DependencyNode>>,
    binding: *const BindingHolder,
}

impl DependencyNode {
    fn for_binding(binding: Pin<&BindingHolder>) -> Self {
        Self {
            next: Cell::new(core::ptr::null()),
            prev: Cell::new(core::ptr::null()),
            binding: binding.get_ref() as *const _,
        }
    }

    /// Assert that the invariant of `next` and `prev` are met.
    fn debug_assert_valid(&self) {
        unsafe {
            debug_assert!(
                self.prev.get().is_null()
                    || (*self.prev.get()).get() == self as *const DependencyNode
            );
            debug_assert!(
                self.next.get().is_null()
                    || (*self.next.get()).prev.get()
                        == (&self.next) as *const Cell<*const DependencyNode>
            );
            // infinite loop?
            debug_assert_ne!(self.next.get(), self as *const DependencyNode);
            debug_assert_ne!(self.prev.get(), (&self.next) as *const Cell<*const DependencyNode>);
        }
    }

    fn remove(&self) {
        self.debug_assert_valid();
        if self.prev.get().is_null() {
            return;
        }
        unsafe {
            if let Some(prev) = self.prev.get().as_ref() {
                prev.set(self.next.get());
            }
            if let Some(next) = self.next.get().as_ref() {
                next.prev.set(self.prev.get());
                next.debug_assert_valid();
            }
        }
        self.prev.set(std::ptr::null());
        self.next.set(std::ptr::null());
    }
}

impl Drop for DependencyNode {
    fn drop(&mut self) {
        self.remove();
    }
}

#[repr(transparent)]
#[derive(Debug, Default)]
struct PropertyHandle {
    handle: Cell<usize>,
}

impl PropertyHandle {
    /// The lock flag specify that we can get reference to the Cell or unsafe cell
    fn lock_flag(&self) -> bool {
        self.handle.get() & 0b1 == 1
    }
    /// Sets the lock_flag.
    /// Safety: the lock flag must not be unsat if there exist reference to what's inside the cell
    unsafe fn set_lock_flag(&self, set: bool) {
        self.handle.set(if set { self.handle.get() | 0b1 } else { self.handle.get() & !0b1 })
    }

    /// Access the value.
    /// Panics if the function try to recursively access the value
    fn access<R>(&self, f: impl FnOnce(Option<Pin<&mut BindingHolder>>) -> R) -> R {
        assert!(!self.lock_flag(), "Recursion detected");
        unsafe {
            self.set_lock_flag(true);
            let handle = self.handle.get();
            let binding = if handle & 0b10 == 0b10 {
                Some(Pin::new_unchecked(&mut *((handle & !0b11) as *mut BindingHolder)))
            } else {
                None
            };
            let r = f(binding);
            self.set_lock_flag(false);
            r
        }
    }

    fn remove_binding(&self) {
        assert!(!self.lock_flag(), "Recursion detected");
        let val = self.handle.get();
        if val & 0b10 == 0b10 {
            unsafe {
                self.set_lock_flag(true);
                let binding = (val & !0b11) as *mut BindingHolder;
                DependencyListHead::mem_move(
                    (&mut (*binding).dependencies) as *mut _ as *mut _,
                    self.handle.as_ptr() as *mut _,
                );
                ((*binding).vtable.drop)(binding);
            }
            debug_assert!(self.handle.get() & 0b11 == 0);
        }
    }

    fn set_binding<B: BindingCallable + 'static>(&self, binding: B) {
        self.remove_binding();
        let binding = alloc_binding_holder::<B>(binding);
        debug_assert!((binding as usize) & 0b11 == 0);
        debug_assert!(self.handle.get() & 0b11 == 0);
        unsafe {
            DependencyListHead::mem_move(
                self.handle.as_ptr() as *mut _,
                (&mut (*binding).dependencies) as *mut _ as *mut _,
            );
            self.handle.set((binding as usize) | 0b10);
        }
    }

    fn dependencies(&self) -> *mut DependencyListHead {
        assert!(!self.lock_flag(), "Recursion detected");
        if (self.handle.get() & 0b10) != 0 {
            self.access(|binding| binding.unwrap().dependencies.as_ptr() as *mut DependencyListHead)
        } else {
            self.handle.as_ptr() as *mut DependencyListHead
        }
    }

    // `value` is the content of the unsafe cell and will be only dereferenced if the
    // handle is not locked. (Upholding the requirements of UnsafeCell)
    unsafe fn update<T>(&self, value: *mut T, context: &EvaluationContext) {
        let remove = self.access(|binding| {
            if let Some(mut binding) = binding {
                if binding.dirty.get() {
                    // clear all the nodes so that we can start from scratch
                    binding.dep_nodes = single_linked_list_pin::SingleLinkedListPinHead::default();
                    let r = (binding.vtable.evaluate)(
                        binding.as_mut().get_unchecked_mut() as *mut BindingHolder,
                        value as *mut (),
                        context,
                    );
                    binding.dirty.set(false);
                    if r == BindingResult::RemoveBinding {
                        return true;
                    }
                }
            }
            false
        });
        if remove {
            self.remove_binding()
        }
    }

    /// Register this property as a dependency to the current binding being evaluated
    fn register_as_dependency_to_current_binding(&self) {
        if CURRENT_BINDING.is_set() {
            CURRENT_BINDING.with(|mut cur_binding| {
                let node = DependencyNode::for_binding(cur_binding.as_mut().as_ref());
                let mut dep_nodes =
                    unsafe { cur_binding.as_mut().map_unchecked_mut(|x| &mut x.dep_nodes) };
                let node = dep_nodes.push_front(node);
                unsafe {
                    DependencyListHead::append(self.dependencies(), node.get_ref() as *const _)
                }
            });
        }
    }

    fn mark_dirty(&self) {
        unsafe { mark_dependencies_dirty(self.dependencies()) };
    }
}

impl Drop for PropertyHandle {
    fn drop(&mut self) {
        self.remove_binding();
        debug_assert!(self.handle.get() & 0b11 == 0);
        unsafe {
            DependencyListHead::drop(self.handle.as_ptr() as *mut _);
        }
    }
}

/// Safety: the dependency list must be valid and consistant
unsafe fn mark_dependencies_dirty(deps: *mut DependencyListHead) {
    let mut next = (*deps).0.get() as *const DependencyNode;
    while let Some(node) = next.as_ref() {
        node.debug_assert_valid();
        next = node.next.get();
        let binding = &*node.binding;
        binding.dirty.set(true);
        (binding.vtable.mark_dirty)(node.binding);
        mark_dependencies_dirty(binding.dependencies.as_ptr() as *mut DependencyListHead)
    }
}

/// A Property that allow binding that track changes
///
/// Property van have be assigned value, or bindings.
/// When a binding is assigned, it is lazily evaluated on demand
/// when calling `get()`.
/// When accessing another property from a binding evaluation,
/// a dependency will be registered, such that when the property
/// change, the binding will automatically be updated
#[repr(C)]
#[derive(Debug)]
pub struct Property<T> {
    /// This is usually a pointer, but the least significant bit tells what it is
    handle: PropertyHandle,
    /// This is only safe to access when the lock flag is not set on the handle.
    value: UnsafeCell<T>,
    pinned: core::marker::PhantomPinned,
}

impl<T: Default> Default for Property<T> {
    fn default() -> Self {
        Self {
            handle: Default::default(),
            value: Default::default(),
            pinned: core::marker::PhantomPinned,
        }
    }
}

impl<T: Clone> Property<T> {
    /// Create a new property with this value
    pub fn new(value: T) -> Self {
        Self {
            handle: Default::default(),
            value: UnsafeCell::new(value),
            pinned: core::marker::PhantomPinned,
        }
    }

    /// Get the value of the property
    ///
    /// This may evaluate the binding if there is a binding and it is dirty
    ///
    /// If the function is called directly or indirectly from a binding evaluation
    /// of another Property, a dependency will be registered.
    ///
    /// Panics if this property is get while evaluating its own binding or
    /// cloning the value.
    pub fn get(self: Pin<&Self>, context: &EvaluationContext) -> T {
        unsafe { self.handle.update(self.value.get(), context) };
        self.handle.register_as_dependency_to_current_binding();
        self.get_internal()
    }

    /// Get the value without registering any dependencies or executing any binding
    fn get_internal(&self) -> T {
        self.handle.access(|_| {
            // Safety: PropertyHandle::access ensure that the value is locked
            unsafe { (*self.value.get()).clone() }
        })
    }

    /// Change the value of this property
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    // FIXME  pub fn set(self: Pin<&Self>, t: T) {
    pub fn set(&self, t: T) {
        self.handle.remove_binding();
        // Safety: PropertyHandle::access ensure that the value is locked
        self.handle.access(|_| unsafe { *self.value.get() = t });
        self.handle.mark_dirty();
    }

    /// Set a binding to this property.
    ///
    /// Bindings are evaluated lazily from calling get, and the return value of the binding
    /// is the new value.
    ///
    /// If other properties have bindings depending of this property, these properties will
    /// be marked as dirty.
    //FIXME pub fn set_binding(self: Pin<&Self>, f: impl (Fn(&EvaluationContext) -> T) + 'static) {
    pub fn set_binding(&self, f: impl (Fn(&EvaluationContext) -> T) + 'static) {
        self.handle.set_binding(move |val: *mut (), context: &EvaluationContext| unsafe {
            *(val as *mut T) = f(context);
            BindingResult::KeepBinding
        });
        self.handle.mark_dirty();
    }
}

impl<T: Clone + InterpolatedPropertyValue + 'static> Property<T> {
    /// Change the value of this property, by animating (interpolating) from the current property's value
    /// to the specified parameter value. The animation is done according to the parameters described by
    /// the PropertyAnimation object.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_value(&self, value: T, animation_data: &PropertyAnimation) {
        // FIXME if the current value is a dirty binding, we must run it, but we do not have the context
        let d = PropertyValueAnimationData::new(self.get_internal(), value, animation_data.clone());
        self.handle.set_binding(move |val: *mut (), _context: &EvaluationContext| unsafe {
            let (value, finished) = d.compute_interpolated_value();
            *(val as *mut T) = value;
            if finished {
                BindingResult::RemoveBinding
            } else {
                crate::animations::CURRENT_ANIMATION_DRIVER
                    .with(|driver| driver.set_has_active_animations());
                BindingResult::KeepBinding
            }
        });
    }

    /// Set a binding to this property.
    ///
    pub fn set_animated_binding(
        &self,
        f: impl (Fn(&EvaluationContext) -> T) + 'static,
        animation_data: &PropertyAnimation,
    ) {
        self.handle.set_binding(AnimatedBindingCallable::<T> {
            original_binding: PropertyHandle {
                handle: Cell::new(
                    (alloc_binding_holder(move |val: *mut (), context: &EvaluationContext| unsafe {
                        *(val as *mut T) = f(context);
                        BindingResult::KeepBinding
                    }) as usize)
                        | 0b10,
                ),
            },
            state: Cell::new(AnimatedBindingState::NotAnimating),
            animation_data: RefCell::new(PropertyValueAnimationData::new(
                T::default(),
                T::default(),
                animation_data.clone(),
            )),
        });
        self.handle.mark_dirty();
    }
}

struct PropertyValueAnimationData<T> {
    from_value: T,
    to_value: T,
    details: crate::abi::primitives::PropertyAnimation,
    start_time: instant::Instant,
}

impl<T: InterpolatedPropertyValue> PropertyValueAnimationData<T> {
    fn new(from_value: T, to_value: T, details: crate::abi::primitives::PropertyAnimation) -> Self {
        let start_time =
            crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick());

        Self { from_value, to_value, details, start_time }
    }

    fn compute_interpolated_value(&self) -> (T, bool) {
        let new_tick =
            crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick());
        let time_progress = new_tick.duration_since(self.start_time).as_millis();
        if time_progress >= self.details.duration as _ {
            return (self.to_value.clone(), true);
        }
        let progress = time_progress as f32 / self.details.duration as f32;
        assert!(progress <= 1.);
        let val = self.from_value.interpolate(self.to_value, progress);
        (val, false)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum AnimatedBindingState {
    Animating,
    NotAnimating,
    ShouldStart,
}

struct AnimatedBindingCallable<T> {
    original_binding: PropertyHandle,
    state: Cell<AnimatedBindingState>,
    animation_data: RefCell<PropertyValueAnimationData<T>>,
}

impl<T: InterpolatedPropertyValue> BindingCallable for AnimatedBindingCallable<T> {
    unsafe fn evaluate(
        self: Pin<&Self>,
        value: *mut (),
        context: &EvaluationContext,
    ) -> BindingResult {
        self.original_binding.register_as_dependency_to_current_binding();
        match self.state.get() {
            AnimatedBindingState::Animating => {
                let (val, finished) = self.animation_data.borrow().compute_interpolated_value();
                *(value as *mut T) = val;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating)
                } else {
                    crate::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| driver.set_has_active_animations());
                }
            }
            AnimatedBindingState::NotAnimating => {
                self.original_binding.update(value, context);
            }
            AnimatedBindingState::ShouldStart => {
                let value = &mut *(value as *mut T);
                self.state.set(AnimatedBindingState::Animating);
                let mut animation_data = self.animation_data.borrow_mut();
                animation_data.from_value = value.clone();
                self.original_binding
                    .update((&mut animation_data.to_value) as *mut T as *mut (), context);
                let (val, finished) = animation_data.compute_interpolated_value();
                *value = val;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating)
                } else {
                    crate::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| driver.set_has_active_animations());
                }
            }
        };
        BindingResult::KeepBinding
    }
    fn mark_dirty(self: Pin<&Self>) {
        if self.state.get() == AnimatedBindingState::ShouldStart {
            return;
        }
        let original_dirty = self.original_binding.access(|b| b.unwrap().dirty.get());
        if original_dirty {
            self.state.set(AnimatedBindingState::ShouldStart);
            self.animation_data.borrow_mut().start_time =
                crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick());
        }
    }
}

#[test]
fn properties_simple_test() {
    use std::rc::Rc;
    use weak_pin::rc::WeakPin;
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
    let compo = Rc::pin(Component::default());
    let w = WeakPin::downgrade(compo.clone());
    compo.area.set_binding(move |ctx| {
        let compo = w.upgrade().unwrap();
        g(&compo.width, ctx) * g(&compo.height, ctx)
    });
    compo.width.set(4);
    compo.height.set(8);
    assert_eq!(g(&compo.width, &dummy_eval_context), 4);
    assert_eq!(g(&compo.height, &dummy_eval_context), 8);
    assert_eq!(g(&compo.area, &dummy_eval_context), 4 * 8);

    let w = WeakPin::downgrade(compo.clone());
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
pub struct PropertyHandleOpaque(PropertyHandle);

/// Initialize the first pointer of the Property. Does not initialize the content.
/// `out` is assumed to be uninitialized
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_init(out: *mut PropertyHandleOpaque) {
    core::ptr::write(out, PropertyHandleOpaque(PropertyHandle::default()));
}

/// To be called before accessing the value
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_update(
    handle: &PropertyHandleOpaque,
    context: &EvaluationContext,
    val: *mut c_void,
) {
    handle.0.update(val, context);
    handle.0.register_as_dependency_to_current_binding();
}

/// Mark the fact that the property was changed and that its binding need to be removed, and
/// The dependencies marked dirty
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_changed(handle: &PropertyHandleOpaque) {
    handle.0.remove_binding();
    handle.0.mark_dirty();
}

fn make_c_function_binding(
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) -> impl Fn(*mut (), &EvaluationContext) -> BindingResult {
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

    let b = CFunctionBinding { binding_function: binding, user_data, drop_user_data };

    move |value_ptr, context| {
        (b.binding_function)(b.user_data, context, value_ptr);
        BindingResult::KeepBinding
    }
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
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(
        user_data: *mut c_void,
        context: &EvaluationContext,
        pointer_to_value: *mut c_void,
    ),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    let binding = make_c_function_binding(binding, user_data, drop_user_data);
    handle.0.set_binding(binding);
}

/// Destroy handle
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_drop(handle: *mut PropertyHandleOpaque) {
    core::ptr::read(handle);
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

fn c_set_animated_value<T: InterpolatedPropertyValue>(
    handle: &PropertyHandleOpaque,
    from: T,
    to: T,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let d = PropertyValueAnimationData::new(from, to, animation_data.clone());
    handle.0.set_binding(move |val: *mut (), _: &EvaluationContext| {
        let (value, finished) = d.compute_interpolated_value();
        unsafe {
            *(val as *mut T) = value;
        }
        if finished {
            BindingResult::RemoveBinding
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            BindingResult::KeepBinding
        }
    });
}

/// Internal function to set up a property animation to the specified target value for an integer property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_value_int(
    handle: &PropertyHandleOpaque,
    from: i32,
    to: i32,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    c_set_animated_value(handle, from, to, animation_data)
}

/// Internal function to set up a property animation to the specified target value for a float property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_value_float(
    handle: &PropertyHandleOpaque,
    from: f32,
    to: f32,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    c_set_animated_value(handle, from, to, animation_data)
}

/// Internal function to set up a property animation to the specified target value for a color property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_value_color(
    handle: &PropertyHandleOpaque,
    from: Color,
    to: Color,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    c_set_animated_value(handle, from, to, animation_data);
}

unsafe fn c_set_animated_binding<T: InterpolatedPropertyValue>(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut T),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    let binding = core::mem::transmute::<
        extern "C" fn(*mut c_void, &EvaluationContext, *mut T),
        extern "C" fn(*mut c_void, &EvaluationContext, *mut ()),
    >(binding);
    handle.0.set_binding(AnimatedBindingCallable::<T> {
        original_binding: PropertyHandle {
            handle: Cell::new(
                (alloc_binding_holder(make_c_function_binding(binding, user_data, drop_user_data))
                    as usize)
                    | 0b10,
            ),
        },
        state: Cell::new(AnimatedBindingState::NotAnimating),
        animation_data: RefCell::new(PropertyValueAnimationData::new(
            T::default(),
            T::default(),
            animation_data.clone(),
        )),
    });
    handle.0.mark_dirty();
}

/// Internal function to set up a property animation between values produced by the specified binding for an integer property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_binding_int(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut i32),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    c_set_animated_binding(handle, binding, user_data, drop_user_data, animation_data);
}

/// Internal function to set up a property animation between values produced by the specified binding for a float property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_binding_float(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut f32),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    c_set_animated_binding(handle, binding, user_data, drop_user_data, animation_data);
}

/// Internal function to set up a property animation between values produced by the specified binding for a color property.
#[no_mangle]
pub unsafe extern "C" fn sixtyfps_property_set_animated_binding_color(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, &EvaluationContext, *mut Color),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    animation_data: &crate::abi::primitives::PropertyAnimation,
) {
    c_set_animated_binding(handle, binding, user_data, drop_user_data, animation_data);
}

#[cfg(test)]
mod animation_tests {
    use super::*;
    use crate::abi::primitives::PropertyAnimation;
    use std::rc::Rc;

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        width_times_two: Property<i32>,
        feed_property: Property<i32>, // used by binding to feed values into width
    }

    const DURATION: instant::Duration = instant::Duration::from_millis(10000);

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
        let compo = Rc::new(Component::default());

        let w = Rc::downgrade(&compo);
        compo.width_times_two.set_binding(move |context| {
            let compo = w.upgrade().unwrap();
            g(&compo.width, context) * 2
        });

        let animation_details = PropertyAnimation { duration: DURATION.as_millis() as _ };

        compo.width.set(100);
        assert_eq!(g(&compo.width, &dummy_eval_context), 100);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 200);

        let start_time =
            crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick());

        compo.width.set_animated_value(200, &animation_details);
        assert_eq!(g(&compo.width, &dummy_eval_context), 100);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(g(&compo.width, &dummy_eval_context), 150);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(g(&compo.width, &dummy_eval_context), 200);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 400);
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        assert_eq!(g(&compo.width, &dummy_eval_context), 200);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
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
        let compo = Rc::new(Component::default());

        let w = Rc::downgrade(&compo);
        compo.width_times_two.set_binding(move |context| {
            let compo = w.upgrade().unwrap();
            g(&compo.width, context) * 2
        });

        let start_time =
            crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick());

        let animation_details = PropertyAnimation { duration: DURATION.as_millis() as _ };

        let w = Rc::downgrade(&compo);
        compo.width.set_animated_binding(
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

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));

        assert_eq!(g(&compo.width, &dummy_eval_context), 150);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));

        assert_eq!(g(&compo.width, &dummy_eval_context), 200);
        assert_eq!(g(&compo.width_times_two, &dummy_eval_context), 400);
    }
}
