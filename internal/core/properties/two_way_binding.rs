// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::Cell;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::marker::PhantomPinned;
use core::pin::Pin;

struct TwoWayBinding<T> {
    common_property: Pin<Rc<Property<T>>>,
}
unsafe impl<T: PartialEq + Clone + 'static> BindingCallable<T> for TwoWayBinding<T> {
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        *value = self.common_property.as_ref().get();
        BindingResult::KeepBinding
    }

    fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
        self.common_property.as_ref().set(value.clone());
        true
    }

    unsafe fn intercept_set_binding(self: Pin<&Self>, new_binding: *mut BindingHolder) -> bool {
        self.common_property.handle.set_binding_impl(new_binding);
        true
    }

    const IS_TWO_WAY_BINDING: bool = true;
}

impl<T: PartialEq + Clone + 'static> Property<T> {
    /// If the property is a two way binding, return the common property
    pub(crate) fn check_common_property(self: Pin<&Self>) -> Option<Pin<Rc<Property<T>>>> {
        let handle_val = self.handle.handle.get();
        if let Some(holder) = PropertyHandle::pointer_to_binding(handle_val) {
            // Safety: the handle is a pointer to a binding
            if unsafe { (*holder).is_two_way_binding } {
                // Safety: the handle is a pointer to a binding whose B is a TwoWayBinding<T>
                return Some(unsafe {
                    (*(holder as *const BindingHolder<TwoWayBinding<T>>))
                        .binding
                        .common_property
                        .clone()
                });
            }
        }
        None
    }

    /// Link two property such that any change to one property is affecting the other property as if they
    /// where, in fact, a single property.
    /// The value or binding of prop2 is kept.
    pub fn link_two_way(prop1: Pin<&Self>, prop2: Pin<&Self>) {
        #[cfg(slint_debug_property)]
        let debug_name =
            alloc::format!("<{}<=>{}>", prop1.debug_name.borrow(), prop2.debug_name.borrow());

        let value = prop2.get_untracked();

        if let Some(common_property) = prop1.check_common_property() {
            // Safety: TwoWayBinding is a BindingCallable for type T
            unsafe {
                prop2.handle.set_binding(
                    TwoWayBinding::<T> { common_property },
                    #[cfg(slint_debug_property)]
                    debug_name.as_str(),
                );
            }
            prop2.set(value);
            return;
        }

        if let Some(common_property) = prop2.check_common_property() {
            // Safety: TwoWayBinding is a BindingCallable for type T
            unsafe {
                prop1.handle.set_binding(
                    TwoWayBinding::<T> { common_property },
                    #[cfg(slint_debug_property)]
                    debug_name.as_str(),
                );
            }
            return;
        }

        let prop2_handle_val = prop2.handle.handle.get();
        let handle = if PropertyHandle::is_pointer_to_binding(prop2_handle_val) {
            debug_assert!(
                PropertyHandle::pointer_to_binding(prop2_handle_val)
                    .is_none_or(|holder| unsafe { !(*holder).is_struct_member_bindings }),
                "whole-struct two-way link on a struct property with member links: \
                 the compiler should have decomposed this link"
            );
            // If prop2 is a binding, just "steal it"
            prop2.handle.handle.set(core::ptr::null_mut());
            PropertyHandle { handle: Cell::new(prop2_handle_val) }
        } else {
            PropertyHandle::default()
        };

        let common_property = Rc::pin(Property {
            handle,
            value: UnsafeCell::new(value),
            pinned: PhantomPinned,
            #[cfg(slint_debug_property)]
            debug_name: debug_name.clone().into(),
        });
        // Safety: TwoWayBinding's T is the same as the type for both properties
        unsafe {
            prop1.handle.set_binding(
                TwoWayBinding { common_property: common_property.clone() },
                #[cfg(slint_debug_property)]
                debug_name.as_str(),
            );
            prop2.handle.set_binding(
                TwoWayBinding { common_property },
                #[cfg(slint_debug_property)]
                debug_name.as_str(),
            );
        }
    }

    /// Link a property to another property of a different type, with mapping function to go between them.
    ///
    /// the value of the `prop1` (of type `T`) is kept. (This is the opposite of [`Self::link_two_way`])
    /// `T2` must be able to be derived from `T` using the `map_to` function.
    /// `T` may contain more information than `T2` and the value of prop1 will be updated with the `map_from` function when `prop2` changes
    pub fn link_two_way_with_map<T2: PartialEq + Clone + 'static>(
        prop1: Pin<&Self>,
        prop2: Pin<&Property<T2>>,
        map_to: impl Fn(&T) -> T2 + Clone + 'static, // Rename map_to_t2
        map_from: impl Fn(&mut T, &T2) + Clone + 'static,
    ) {
        let common_property = if let Some(common_property) = prop1.check_common_property() {
            common_property
        } else {
            let prop1_handle_val = prop1.handle.handle.get();
            let handle = if PropertyHandle::is_pointer_to_binding(prop1_handle_val) {
                // If prop1 is a binding, just "steal it"
                prop1.handle.handle.set(core::ptr::null_mut());
                PropertyHandle { handle: Cell::new(prop1_handle_val) }
            } else {
                PropertyHandle::default()
            };

            #[cfg(slint_debug_property)]
            let debug_name = alloc::format!("{}*", prop1.debug_name.borrow());

            let common_property = Rc::pin(Property {
                handle,
                value: UnsafeCell::new(prop1.get_internal()),
                pinned: PhantomPinned,
                #[cfg(slint_debug_property)]
                debug_name: debug_name.clone().into(),
            });
            // Safety: TwoWayBinding's T is the same as the type for both properties
            unsafe {
                prop1.handle.set_binding(
                    TwoWayBinding::<T> { common_property: common_property.clone() },
                    #[cfg(slint_debug_property)]
                    debug_name.as_str(),
                );
            }
            common_property
        };
        Self::link_two_way_with_map_to_common_property(
            common_property,
            prop2,
            map_to,
            map_from,
            false,
        );
    }

    /// Make a two way binding between the common property and the binding prop2.
    /// Two-way bindings on prop2 are always forwarded through the chain.
    /// Regular closure bindings on prop2 are preserved when
    /// `preserve_prop2_binding` is true, or dropped (so the common
    /// property's value wins) when false.
    pub(crate) fn link_two_way_with_map_to_common_property<T2: PartialEq + Clone + 'static>(
        common_property: Pin<Rc<Self>>,
        prop2: Pin<&Property<T2>>,
        map_to: impl Fn(&T) -> T2 + Clone + 'static,
        map_from: impl Fn(&mut T, &T2) + Clone + 'static,
        preserve_prop2_binding: bool,
    ) {
        struct TwoWayBindingWithMap<T, T2, M1, M2> {
            common_property: Pin<Rc<Property<T>>>,
            map_to: M1,
            map_from: M2,
            marker: PhantomData<(T, T2)>,
        }
        unsafe impl<
            T: PartialEq + Clone + 'static,
            T2: PartialEq + Clone + 'static,
            M1: Fn(&T) -> T2 + Clone + 'static,
            M2: Fn(&mut T, &T2) + Clone + 'static,
        > BindingCallable<T2> for TwoWayBindingWithMap<T, T2, M1, M2>
        {
            fn evaluate(self: Pin<&Self>, value: &mut T2) -> BindingResult {
                *value = (self.map_to)(&self.common_property.as_ref().get());
                BindingResult::KeepBinding
            }

            fn intercept_set(self: Pin<&Self>, value: &T2) -> bool {
                let mut old = self.common_property.as_ref().get();
                (self.map_from)(&mut old, value);
                self.common_property.as_ref().set(old);
                true
            }

            unsafe fn intercept_set_binding(
                self: Pin<&Self>,
                new_binding: *mut BindingHolder,
            ) -> bool {
                let new_new_binding = alloc_binding_holder(BindingMapper::<T, T2, M1, M2> {
                    b: new_binding,
                    map_to: self.map_to.clone(),
                    map_from: self.map_from.clone(),
                    marker: PhantomData,
                });
                self.common_property.handle.set_binding_impl(new_new_binding);
                true
            }
        }

        /// Given a binding for T2, maps to a binding for T
        struct BindingMapper<T, T2, M1, M2> {
            /// Binding that returns a `T2`
            b: *mut BindingHolder,
            map_to: M1,
            map_from: M2,
            marker: PhantomData<(T, T2)>,
        }
        unsafe impl<
            T: PartialEq + Clone + 'static,
            T2: PartialEq + Clone + 'static,
            M1: Fn(&T) -> T2 + 'static,
            M2: Fn(&mut T, &T2) + 'static,
        > BindingCallable<T> for BindingMapper<T, T2, M1, M2>
        {
            fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
                let mut sub_value = (self.map_to)(value);
                // Safety: `self.b` is a BindingHolder that expects a `T2`
                unsafe {
                    ((*self.b).vtable.evaluate)(
                        self.b,
                        (&mut sub_value as *mut T2).cast::<c_void>(),
                    );
                }
                (self.map_from)(value, &sub_value);
                BindingResult::KeepBinding
            }

            fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
                let sub_value = (self.map_to)(value);
                // Safety: `self.b` is a BindingHolder that expects a `T2`
                unsafe {
                    ((*self.b).vtable.intercept_set)(
                        self.b,
                        (&sub_value as *const T2).cast::<c_void>(),
                    )
                }
            }
        }
        impl<T, T2, M1, M2> Drop for BindingMapper<T, T2, M1, M2> {
            fn drop(&mut self) {
                unsafe {
                    ((*self.b).vtable.drop)(self.b);
                }
            }
        }

        #[cfg(slint_debug_property)]
        let debug_name = alloc::format!(
            "<{}<=>{}>",
            common_property.debug_name.borrow(),
            prop2.debug_name.borrow()
        );

        let old_binding = prop2.handle.detach_binding();

        unsafe {
            if let Some(old) = old_binding {
                let new_binding = alloc_binding_holder(TwoWayBindingWithMap {
                    common_property,
                    map_to,
                    map_from,
                    marker: PhantomData,
                });
                if ((*old).vtable.intercept_set_binding)(old, new_binding) {
                    prop2.handle.set_binding_impl(old);
                } else {
                    prop2.handle.set_binding_impl(new_binding);
                    if preserve_prop2_binding {
                        // Re-attach so TwoWayBindingWithMap wraps it
                        // as a BindingMapper for reactivity.
                        prop2.handle.set_binding_impl(old);
                    } else {
                        ((*old).vtable.drop)(old);
                    }
                }
            } else {
                prop2.handle.set_binding(
                    TwoWayBindingWithMap { common_property, map_to, map_from, marker: PhantomData },
                    #[cfg(slint_debug_property)]
                    debug_name.as_str(),
                );
            }
        }
    }
}

/// State shared between a [`StructMemberBindings`] wrapper installed on a
/// struct property and the [`DriverProjection`] bindings it installs on the
/// narrow common properties of the two-way classes its fields belong to.
struct DriverSlot<T> {
    /// The struct property's own value-producing binding ("real binding"),
    /// if any. Owned by this slot; evaluated by the wrapper (to produce the
    /// whole struct) and by the projections (to drive the commons).
    holder: Cell<Option<*mut BindingHolder>>,
    /// Seed value used to evaluate the real binding outside of the struct
    /// property's own storage (projections have no access to the property).
    cache: RefCell<T>,
}

impl<T> DriverSlot<T> {
    /// Drop the real binding, if any. Projections evaluate to a no-op and
    /// remove themselves once the slot is empty.
    fn clear(&self) {
        if let Some(holder) = self.holder.take() {
            unsafe { ((*holder).vtable.drop)(holder) }
        }
    }
}

impl<T> Drop for DriverSlot<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Binding installed on a narrow common property so that the struct
/// property's real binding drives the two-way class ("driver election"):
/// it evaluates the real binding and extracts the mapped field.
///
/// Installed whenever a real binding is (re-)assigned to a wrapped struct
/// property; the last installed binding on the common wins, like any other
/// binding installed on a two-way class.
struct DriverProjection<T, T2, GetField> {
    slot: Rc<DriverSlot<T>>,
    get_field: GetField,
    marker: PhantomData<fn(T) -> T2>,
}

// Safety: IS_TWO_WAY_BINDING and IS_STRUCT_MEMBER_BINDINGS are false
unsafe impl<
    T: PartialEq + Clone + 'static,
    T2: PartialEq + Clone + 'static,
    GetField: Fn(&T) -> T2 + 'static,
> BindingCallable<T2> for DriverProjection<T, T2, GetField>
{
    fn evaluate(self: Pin<&Self>, value: &mut T2) -> BindingResult {
        let Some(real_binding) = self.slot.holder.get() else {
            // The driver was dropped (e.g. a value was set on the struct
            // property); keep the common's current value and clean up.
            return BindingResult::RemoveBinding;
        };
        let mut struct_value = self.slot.cache.borrow().clone();
        // Safety: `real_binding` is a BindingHolder producing a `T`
        let result = unsafe {
            ((*real_binding).vtable.evaluate)(
                real_binding,
                (&mut struct_value as *mut T).cast::<c_void>(),
            )
        };
        *value = (self.get_field)(&struct_value);
        self.slot.cache.replace(struct_value);
        if result == BindingResult::RemoveBinding {
            self.slot.clear();
        }
        BindingResult::KeepBinding
    }

    fn mark_dirty(self: Pin<&Self>) {
        if let Some(real_binding) = self.slot.holder.get() {
            unsafe { ((*real_binding).vtable.mark_dirty)(real_binding, false) }
        }
    }
}

/// One field of a struct property that participates in a two-way binding
/// class, held by [`StructMemberBindings`].
struct StructMemberMapping<T> {
    /// Field path within the struct (e.g. `"field"` or `"outer.inner"`),
    /// used to find and replace the mapping when the same link is
    /// re-established (conditional/repeated component re-instantiation).
    key: &'static str,
    /// `value.<key> = common.get()`; reading the common registers the
    /// dependency so the struct re-evaluates when the class changes.
    apply_from_common: Box<dyn Fn(&mut T)>,
    /// `common.set(get_field(value))`
    push_to_common: Box<dyn Fn(&T)>,
    /// Installs a [`DriverProjection`] for this mapping's field onto the
    /// common, making the given slot's real binding drive the class.
    install_projection: Box<dyn Fn(&Rc<DriverSlot<T>>)>,
    /// The narrow common property (an `Rc<Property<T2>>`, semantically
    /// pinned), type-erased for the field-keyed reuse lookup.
    common: Rc<dyn core::any::Any>,
}

/// The binding wrapper installed on a struct property that has two-way
/// bindings onto its *fields*. Each mapped field is synchronized with a
/// narrow common property of the field's type that is shared by every
/// member of the two-way binding class; the struct's own value-producing
/// binding, if any, lives in the [`DriverSlot`] and both produces the
/// unmapped fields and drives the commons via [`DriverProjection`]s.
struct StructMemberBindings<T> {
    slot: Rc<DriverSlot<T>>,
    mappings: RefCell<Vec<StructMemberMapping<T>>>,
}

// Safety: IS_STRUCT_MEMBER_BINDINGS is true and Self is StructMemberBindings
unsafe impl<T: PartialEq + Clone + 'static> BindingCallable<T> for StructMemberBindings<T> {
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        if let Some(real_binding) = self.slot.holder.get() {
            // Safety: `real_binding` is a BindingHolder producing a `T`;
            // dependencies register on the holder currently being evaluated
            // (this wrapper's), as with any wrapped binding.
            let result = unsafe {
                ((*real_binding).vtable.evaluate)(real_binding, (value as *mut T).cast::<c_void>())
            };
            self.slot.cache.replace(value.clone());
            if result == BindingResult::RemoveBinding {
                self.slot.clear();
            }
        }
        for mapping in self.mappings.borrow().iter() {
            (mapping.apply_from_common)(value);
        }
        BindingResult::KeepBinding
    }

    fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
        // Setting a value drops the real binding (as it would on an
        // unwrapped property) and pushes the mapped fields into their
        // classes. The unmapped fields are stored by `Property::set`.
        self.slot.clear();
        for mapping in self.mappings.borrow().iter() {
            (mapping.push_to_common)(value);
        }
        true
    }

    unsafe fn intercept_set_binding(self: Pin<&Self>, new_binding: *mut BindingHolder) -> bool {
        unsafe {
            debug_assert!(
                !(*new_binding).is_two_way_binding,
                "whole-struct two-way link on a struct property with member links: \
                 the compiler should have decomposed this link"
            );
            // Drop stale dependency registrations in case this holder was
            // evaluated while installed elsewhere; new registrations land on
            // this wrapper's holder when it evaluates the binding.
            *(*new_binding).dep_nodes.get() = Default::default();
        }
        if let Some(old) = self.slot.holder.replace(Some(new_binding)) {
            unsafe { ((*old).vtable.drop)(old) }
        }
        // The new binding must drive the classes of all mapped fields.
        // Installing the projections also marks the commons' dependents
        // dirty (including this wrapper and the struct's dependents),
        // which `set_binding_impl` skips for intercepted assignments.
        for mapping in self.mappings.borrow().iter() {
            (mapping.install_projection)(&self.slot);
        }
        true
    }

    fn mark_dirty(self: Pin<&Self>) {
        if let Some(real_binding) = self.slot.holder.get() {
            unsafe { ((*real_binding).vtable.mark_dirty)(real_binding, false) }
        }
    }

    const IS_STRUCT_MEMBER_BINDINGS: bool = true;
}

impl<T: PartialEq + Clone + 'static> Property<T> {
    /// If the property currently wears a [`StructMemberBindings`] wrapper,
    /// call `f` with it.
    fn with_struct_member_bindings<R>(
        self: Pin<&Self>,
        f: impl FnOnce(&StructMemberBindings<T>) -> R,
    ) -> Option<R> {
        let handle_val = self.handle.handle.get();
        let holder = PropertyHandle::pointer_to_binding(handle_val)?;
        // Safety: the handle is a pointer to a binding
        unsafe {
            if (*holder).is_struct_member_bindings {
                // Safety: the flag guarantees the binding is a
                // StructMemberBindings for this property's type
                let wrapper =
                    &(*(holder as *const BindingHolder<StructMemberBindings<T>>)).binding;
                Some(f(wrapper))
            } else {
                None
            }
        }
    }

    /// The narrow common property that the given field of this struct
    /// property is synchronized with, if any.
    fn struct_member_common<T2: PartialEq + Clone + 'static>(
        self: Pin<&Self>,
        field_key: &'static str,
    ) -> Option<Pin<Rc<Property<T2>>>> {
        self.with_struct_member_bindings(|wrapper| {
            wrapper
                .mappings
                .borrow()
                .iter()
                .find(|mapping| mapping.key == field_key)
                .and_then(|mapping| mapping.common.clone().downcast::<Property<T2>>().ok())
                // Safety: the Rc was created pinned (`Rc::pin`) and never unpinned
                .map(|rc| unsafe { Pin::new_unchecked(rc) })
        })
        .flatten()
    }

    /// Make sure this struct property wears a [`StructMemberBindings`]
    /// wrapper, moving a pre-existing value-producing binding into the
    /// wrapper's driver slot.
    fn ensure_struct_member_bindings(self: Pin<&Self>) {
        if self.with_struct_member_bindings(|_| ()).is_some() {
            return;
        }
        let slot = Rc::new(DriverSlot {
            holder: Cell::new(None),
            cache: RefCell::new(self.get_internal()),
        });
        if let Some(old) = self.handle.detach_binding() {
            debug_assert!(
                unsafe { !(*old).is_two_way_binding },
                "whole-struct two-way link on a struct property with member links: \
                 the compiler should have decomposed this link"
            );
            // Drop stale dependency registrations from evaluations that
            // happened while the binding was still installed directly; new
            // registrations land on the wrapper when it evaluates the binding.
            unsafe { *(*old).dep_nodes.get() = Default::default() };
            slot.holder.set(Some(old));
        }
        // Safety: StructMemberBindings<T> is a BindingCallable for type T
        unsafe {
            self.handle.set_binding(
                StructMemberBindings { slot, mappings: RefCell::new(Vec::new()) },
                #[cfg(slint_debug_property)]
                &alloc::format!("<members of {}>", self.debug_name.borrow()),
            );
        }
    }

    /// Add (or replace, keyed by `field_key`) a mapping synchronizing
    /// `field_key` of this struct property with `common`.
    fn add_struct_member_mapping<T2: PartialEq + Clone + 'static>(
        self: Pin<&Self>,
        field_key: &'static str,
        common: Pin<Rc<Property<T2>>>,
        get_field: impl Fn(&T) -> T2 + Clone + 'static,
        set_field: impl Fn(&mut T, &T2) + Clone + 'static,
    ) {
        self.ensure_struct_member_bindings();
        self.with_struct_member_bindings(|wrapper| {
            let mapping = StructMemberMapping {
                key: field_key,
                apply_from_common: Box::new({
                    let common = common.clone();
                    move |value: &mut T| {
                        let field_value = common.as_ref().get();
                        set_field(value, &field_value);
                    }
                }),
                push_to_common: Box::new({
                    let common = common.clone();
                    let get_field = get_field.clone();
                    move |value: &T| common.as_ref().set(get_field(value))
                }),
                install_projection: Box::new({
                    let common = common.clone();
                    move |slot: &Rc<DriverSlot<T>>| {
                        // Safety: DriverProjection is a BindingCallable for T2
                        unsafe {
                            common.handle.set_binding(
                                DriverProjection {
                                    slot: slot.clone(),
                                    get_field: get_field.clone(),
                                    marker: PhantomData,
                                },
                                #[cfg(slint_debug_property)]
                                &alloc::format!("<driver of {}>", common.debug_name.borrow()),
                            );
                        }
                    }
                }),
                // Safety: the Rc stays semantically pinned; it is only ever
                // re-pinned in `struct_member_common`
                common: unsafe { Pin::into_inner_unchecked(common.clone()) },
            };
            // A pre-existing real binding must also drive this field's class
            // (it may be a new class, or the class' driver may have been
            // dropped by the value push that preceded this call).
            if wrapper.slot.holder.get().is_some() {
                (mapping.install_projection)(&wrapper.slot);
            }
            let mut mappings = wrapper.mappings.borrow_mut();
            if let Some(existing) = mappings.iter_mut().find(|m| m.key == field_key) {
                *existing = mapping;
            } else {
                mappings.push(mapping);
            }
        });
        // The wrapper must re-evaluate to pick up the new mapping's common
        // (and register the dependency on it, so that later changes of the
        // class keep dirtying it): mark it and the struct's dependents dirty.
        if let Some(holder) = PropertyHandle::pointer_to_binding(self.handle.handle.get()) {
            // Safety: the handle points to the wrapper's binding holder
            unsafe { (*holder).dirty.set(true) };
        }
        if !self.handle.is_constant() {
            self.handle.mark_dirty(
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            );
        }
    }

    /// Link `field_key` of the struct property `struct_prop` two-way with the
    /// (typically scalar) property `member_prop`, such that they always have
    /// the same value, as if they were a single property.
    ///
    /// This is the runtime counterpart of `member <=> strct.field` in Slint;
    /// `struct_prop` is the right-hand side and its current field value wins.
    /// `get_field`/`set_field` read/write the field at `field_key` of the
    /// struct.
    pub fn link_two_way_to_member<T2: PartialEq + Clone + 'static>(
        struct_prop: Pin<&Self>,
        member_prop: Pin<&Property<T2>>,
        field_key: &'static str,
        get_field: impl Fn(&T) -> T2 + Clone + 'static,
        set_field: impl Fn(&mut T, &T2) + Clone + 'static,
    ) {
        let member_common = member_prop.check_common_property();
        let member_was_linked = member_common.is_some();
        let struct_common = struct_prop.struct_member_common::<T2>(field_key);

        let common = match (member_common, struct_common) {
            (Some(member_common), Some(struct_common)) => {
                if !core::ptr::eq(&*member_common, &*struct_common) {
                    // Both sides are already in (distinct) classes: unify
                    // them by linking the two commons like ordinary scalar
                    // properties.
                    Property::link_two_way(member_common.as_ref(), struct_common.as_ref());
                }
                struct_common
            }
            (Some(common), None) | (None, Some(common)) => common,
            (None, None) => {
                // Seed a new class with the struct's genuine field value,
                // read before the mapping for this link is installed: the
                // right-hand side of `<=>` wins, as with `link_two_way`.
                Rc::pin(Property::new(get_field(&struct_prop.get_untracked())))
            }
        };

        // Push the struct's field value into a reused class, unless the
        // class is driven by a binding — the binding stays authoritative
        // (and a push would drop it, e.g. sever a model-row binding).
        if !common.has_binding() {
            common.as_ref().set(get_field(&struct_prop.get_untracked()));
        }

        struct_prop.add_struct_member_mapping(field_key, common.clone(), get_field, set_field);

        if !member_was_linked {
            #[cfg(slint_debug_property)]
            let debug_name = alloc::format!(
                "<{}<=>{}.{}>",
                member_prop.debug_name.borrow(),
                struct_prop.debug_name.borrow(),
                field_key
            );
            let old_binding = member_prop.handle.detach_binding();
            unsafe {
                if let Some(old) = old_binding {
                    let new_binding =
                        alloc_binding_holder(TwoWayBinding { common_property: common });
                    if ((*old).vtable.intercept_set_binding)(old, new_binding) {
                        member_prop.handle.set_binding_impl(old);
                    } else {
                        member_prop.handle.set_binding_impl(new_binding);
                        // The member's own binding is dropped: the struct's
                        // value wins (as with `link_two_way_with_map`).
                        ((*old).vtable.drop)(old);
                    }
                } else {
                    member_prop.handle.set_binding(
                        TwoWayBinding { common_property: common },
                        #[cfg(slint_debug_property)]
                        debug_name.as_str(),
                    );
                }
            }
        }
    }

    /// Link `field_key_a` of the struct property `prop_a` two-way with
    /// `field_key_b` of the struct property `prop_b` (both fields have the
    /// same type `T2`), such that they always have the same value.
    ///
    /// This is the runtime counterpart of a whole-struct `<=>` that the
    /// compiler decomposed into per-field links because the class also
    /// contains field links; `prop_b` is the right-hand side and its current
    /// field value wins.
    pub fn link_two_way_members<TB: PartialEq + Clone + 'static, T2: PartialEq + Clone + 'static>(
        prop_a: Pin<&Self>,
        field_key_a: &'static str,
        get_field_a: impl Fn(&T) -> T2 + Clone + 'static,
        set_field_a: impl Fn(&mut T, &T2) + Clone + 'static,
        prop_b: Pin<&Property<TB>>,
        field_key_b: &'static str,
        get_field_b: impl Fn(&TB) -> T2 + Clone + 'static,
        set_field_b: impl Fn(&mut TB, &T2) + Clone + 'static,
    ) {
        let common_a = prop_a.struct_member_common::<T2>(field_key_a);
        let common_b = prop_b.struct_member_common::<T2>(field_key_b);

        let common = match (common_a, common_b) {
            (Some(common_a), Some(common_b)) => {
                if !core::ptr::eq(&*common_a, &*common_b) {
                    Property::link_two_way(common_a.as_ref(), common_b.as_ref());
                }
                common_b
            }
            (Some(common), None) | (None, Some(common)) => common,
            (None, None) => {
                // Seed a new class with `prop_b`'s genuine field value (the
                // right-hand side of `<=>` wins, as with `link_two_way`).
                Rc::pin(Property::new(get_field_b(&prop_b.get_untracked())))
            }
        };

        // Push `prop_b`'s field value into a reused class, unless the class
        // is driven by a binding — the binding stays authoritative.
        if !common.has_binding() {
            common.as_ref().set(get_field_b(&prop_b.get_untracked()));
        }

        // `prop_b`'s mapping is installed last, so a real binding on `prop_b`
        // wins the driver election over one on `prop_a`.
        prop_a.add_struct_member_mapping(field_key_a, common.clone(), get_field_a, set_field_a);
        prop_b.add_struct_member_mapping(field_key_b, common, get_field_b, set_field_b);
    }

    /// Link `field_key` of the struct property `struct_prop` two-way with a
    /// value stored in a model row (`getter`/`setter` read and write the
    /// row), such that they always have the same value.
    ///
    /// This is the runtime counterpart of a whole-row `strct <=> model-data`
    /// that the compiler decomposed into per-field links because the struct
    /// property's fields also participate in two-way binding classes. The
    /// model is authoritative: the row binding is installed on the class'
    /// common property and drives it.
    pub fn link_two_way_member_to_model_data<T2: PartialEq + Clone + 'static, ItemTree: 'static>(
        struct_prop: Pin<&Self>,
        field_key: &'static str,
        get_field: impl Fn(&T) -> T2 + Clone + 'static,
        set_field: impl Fn(&mut T, &T2) + Clone + 'static,
        item_tree: ItemTree,
        getter: impl Fn(&ItemTree) -> Option<T2> + 'static,
        setter: impl Fn(&ItemTree, &T2) + 'static,
    ) {
        let common = struct_prop.struct_member_common::<T2>(field_key).unwrap_or_else(|| {
            Rc::pin(Property::new(get_field(&struct_prop.get_untracked())))
        });
        struct_prop.add_struct_member_mapping(field_key, common.clone(), get_field, set_field);
        // Install the row binding on the common (replacing a driver
        // projection a real binding on `struct_prop` may just have
        // installed): the model wins, as with `link_two_way_to_model_data`.
        common.as_ref().link_two_way_to_model_data(item_tree, getter, setter);
    }
}

struct TwoWayBindingModel<T, ItemTree, Getter, Setter> {
    phantom: PhantomData<fn(T) -> T>,
    item_tree: ItemTree,
    getter: Getter,
    setter: Setter,
}

// Safety: IS_TWO_WAY_BINDING is false
unsafe impl<T, ItemTree, Getter, Setter> BindingCallable<T>
    for TwoWayBindingModel<T, ItemTree, Getter, Setter>
where
    Getter: Fn(&ItemTree) -> Option<T>,
    Setter: Fn(&ItemTree, &T),
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        if let Some(v) = (self.getter)(&self.item_tree) {
            *value = v;
        }
        BindingResult::KeepBinding
    }

    unsafe fn intercept_set_binding(self: Pin<&Self>, _new_binding: *mut BindingHolder) -> bool {
        false
    }

    fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
        (self.setter)(&self.item_tree, value);
        true
    }
}

impl<T: 'static> Property<T> {
    /// Bind this property two-way to a value stored in a model row.
    /// `getter` reads the current row value (and registers a dependency on
    /// it); `setter` writes a new value back into the row.
    pub fn link_two_way_to_model_data<ItemTree: 'static>(
        self: Pin<&Self>,
        item_tree: ItemTree,
        getter: impl Fn(&ItemTree) -> Option<T> + 'static,
        setter: impl Fn(&ItemTree, &T) + 'static,
    ) {
        let binding = TwoWayBindingModel { phantom: PhantomData, item_tree, getter, setter };
        // Safety: TwoWayBindingModel implements BindingCallable<T> for the same T as `Self`.
        unsafe {
            self.handle.set_binding(
                binding,
                #[cfg(slint_debug_property)]
                &alloc::format!("{}<=>[model]", self.debug_name.borrow()),
            )
        };
    }
}

#[test]
fn property_two_ways_test() {
    let p1 = Rc::pin(Property::new(42));
    let p2 = Rc::pin(Property::new(88));

    let depends = Box::pin(Property::new(0));
    depends.as_ref().set_binding({
        let p1 = p1.clone();
        move || p1.as_ref().get() + 8
    });
    assert_eq!(depends.as_ref().get(), 42 + 8);
    Property::link_two_way(p1.as_ref(), p2.as_ref());
    assert_eq!(p1.as_ref().get(), 88);
    assert_eq!(p2.as_ref().get(), 88);
    assert_eq!(depends.as_ref().get(), 88 + 8);
    p2.as_ref().set(5);
    assert_eq!(p1.as_ref().get(), 5);
    assert_eq!(p2.as_ref().get(), 5);
    assert_eq!(depends.as_ref().get(), 5 + 8);
    p1.as_ref().set(22);
    assert_eq!(p1.as_ref().get(), 22);
    assert_eq!(p2.as_ref().get(), 22);
    assert_eq!(depends.as_ref().get(), 22 + 8);
}

#[test]
fn property_two_ways_test_binding() {
    let p1 = Rc::pin(Property::new(42));
    let p2 = Rc::pin(Property::new(88));
    let global = Rc::pin(Property::new(23));
    p2.as_ref().set_binding({
        let global = global.clone();
        move || global.as_ref().get() + 9
    });

    let depends = Box::pin(Property::new(0));
    depends.as_ref().set_binding({
        let p1 = p1.clone();
        move || p1.as_ref().get() + 8
    });

    Property::link_two_way(p1.as_ref(), p2.as_ref());
    assert_eq!(p1.as_ref().get(), 23 + 9);
    assert_eq!(p2.as_ref().get(), 23 + 9);
    assert_eq!(depends.as_ref().get(), 23 + 9 + 8);
    global.as_ref().set(55);
    assert_eq!(p1.as_ref().get(), 55 + 9);
    assert_eq!(p2.as_ref().get(), 55 + 9);
    assert_eq!(depends.as_ref().get(), 55 + 9 + 8);
}

#[test]
fn property_two_ways_recurse_from_binding() {
    let xx = Rc::pin(Property::new(0));

    let p1 = Rc::pin(Property::new(42));
    let p2 = Rc::pin(Property::new(88));
    let global = Rc::pin(Property::new(23));

    let done = Rc::new(Cell::new(false));
    xx.set_binding({
        let p1 = p1.clone();
        let p2 = p2.clone();
        let global = global.clone();
        let xx_weak = pin_weak::rc::PinWeak::downgrade(xx.clone());
        move || {
            if !done.get() {
                done.set(true);
                Property::link_two_way(p1.as_ref(), p2.as_ref());
                let xx_weak = xx_weak.clone();
                p1.as_ref().set_binding(move || xx_weak.upgrade().unwrap().as_ref().get() + 9);
            }
            global.as_ref().get() + 2
        }
    });
    assert_eq!(xx.as_ref().get(), 23 + 2);
    assert_eq!(p1.as_ref().get(), 23 + 2 + 9);
    assert_eq!(p2.as_ref().get(), 23 + 2 + 9);

    global.as_ref().set(55);
    assert_eq!(p1.as_ref().get(), 55 + 2 + 9);
    assert_eq!(p2.as_ref().get(), 55 + 2 + 9);
    assert_eq!(xx.as_ref().get(), 55 + 2);
}

#[test]
fn property_two_ways_binding_of_two_way_binding_first() {
    let p1_1 = Rc::pin(Property::new(2));
    let p1_2 = Rc::pin(Property::new(4));
    Property::link_two_way(p1_1.as_ref(), p1_2.as_ref());

    assert_eq!(p1_1.as_ref().get(), 4);
    assert_eq!(p1_2.as_ref().get(), 4);

    let p2 = Rc::pin(Property::new(3));
    Property::link_two_way(p1_1.as_ref(), p2.as_ref());

    assert_eq!(p1_1.as_ref().get(), 3);
    assert_eq!(p1_2.as_ref().get(), 3);
    assert_eq!(p2.as_ref().get(), 3);

    p1_1.set(6);

    assert_eq!(p1_1.as_ref().get(), 6);
    assert_eq!(p1_2.as_ref().get(), 6);
    assert_eq!(p2.as_ref().get(), 6);

    p1_2.set(8);

    assert_eq!(p1_1.as_ref().get(), 8);
    assert_eq!(p1_2.as_ref().get(), 8);
    assert_eq!(p2.as_ref().get(), 8);

    p2.set(7);

    assert_eq!(p1_1.as_ref().get(), 7);
    assert_eq!(p1_2.as_ref().get(), 7);
    assert_eq!(p2.as_ref().get(), 7);
}

#[test]
fn property_two_ways_binding_of_two_way_binding_second() {
    let p1 = Rc::pin(Property::new(2));
    let p2_1 = Rc::pin(Property::new(3));
    let p2_2 = Rc::pin(Property::new(5));
    Property::link_two_way(p2_1.as_ref(), p2_2.as_ref());

    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    Property::link_two_way(p1.as_ref(), p2_2.as_ref());

    assert_eq!(p1.as_ref().get(), 5);
    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    p1.set(6);

    assert_eq!(p1.as_ref().get(), 6);
    assert_eq!(p2_1.as_ref().get(), 6);
    assert_eq!(p2_2.as_ref().get(), 6);

    p2_1.set(7);

    assert_eq!(p1.as_ref().get(), 7);
    assert_eq!(p2_1.as_ref().get(), 7);
    assert_eq!(p2_2.as_ref().get(), 7);

    p2_2.set(9);

    assert_eq!(p1.as_ref().get(), 9);
    assert_eq!(p2_1.as_ref().get(), 9);
    assert_eq!(p2_2.as_ref().get(), 9);
}

#[test]
fn property_two_ways_binding_of_two_two_way_bindings() {
    let p1_1 = Rc::pin(Property::new(2));
    let p1_2 = Rc::pin(Property::new(4));
    Property::link_two_way(p1_1.as_ref(), p1_2.as_ref());
    assert_eq!(p1_1.as_ref().get(), 4);
    assert_eq!(p1_2.as_ref().get(), 4);

    let p2_1 = Rc::pin(Property::new(3));
    let p2_2 = Rc::pin(Property::new(5));
    Property::link_two_way(p2_1.as_ref(), p2_2.as_ref());

    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    Property::link_two_way(p1_1.as_ref(), p2_2.as_ref());

    assert_eq!(p1_1.as_ref().get(), 5);
    assert_eq!(p1_2.as_ref().get(), 5);
    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    p1_1.set(6);
    assert_eq!(p1_1.as_ref().get(), 6);
    assert_eq!(p1_2.as_ref().get(), 6);
    assert_eq!(p2_1.as_ref().get(), 6);
    assert_eq!(p2_2.as_ref().get(), 6);

    p1_2.set(8);
    assert_eq!(p1_1.as_ref().get(), 8);
    assert_eq!(p1_2.as_ref().get(), 8);
    assert_eq!(p2_1.as_ref().get(), 8);
    assert_eq!(p2_2.as_ref().get(), 8);

    p2_1.set(7);
    assert_eq!(p1_1.as_ref().get(), 7);
    assert_eq!(p1_2.as_ref().get(), 7);
    assert_eq!(p2_1.as_ref().get(), 7);
    assert_eq!(p2_2.as_ref().get(), 7);

    p2_2.set(9);
    assert_eq!(p1_1.as_ref().get(), 9);
    assert_eq!(p1_2.as_ref().get(), 9);
    assert_eq!(p2_1.as_ref().get(), 9);
    assert_eq!(p2_2.as_ref().get(), 9);
}

#[test]
fn test_two_way_with_map() {
    #[derive(PartialEq, Clone, Default, Debug)]
    struct Struct {
        foo: i32,
        bar: alloc::string::String,
    }
    let p1 = Rc::pin(Property::new(Struct { foo: 42, bar: "hello".into() }));
    let p2 = Rc::pin(Property::new(88));
    let p3 = Rc::pin(Property::new(alloc::string::String::from("xyz")));
    Property::link_two_way_with_map(p1.as_ref(), p2.as_ref(), |s| s.foo, |s, foo| s.foo = *foo);
    assert_eq!(p1.as_ref().get(), Struct { foo: 42, bar: "hello".into() });
    assert_eq!(p2.as_ref().get(), 42);

    p2.as_ref().set(81);
    assert_eq!(p1.as_ref().get(), Struct { foo: 81, bar: "hello".into() });
    assert_eq!(p2.as_ref().get(), 81);

    p1.as_ref().set(Struct { foo: 78, bar: "world".into() });
    assert_eq!(p1.as_ref().get(), Struct { foo: 78, bar: "world".into() });
    assert_eq!(p2.as_ref().get(), 78);

    Property::link_two_way_with_map(
        p1.as_ref(),
        p3.as_ref(),
        |s| s.bar.clone(),
        |s, bar| s.bar = bar.clone(),
    );
    assert_eq!(p1.as_ref().get(), Struct { foo: 78, bar: "world".into() });
    assert_eq!(p2.as_ref().get(), 78);
    assert_eq!(p3.as_ref().get(), "world");

    p3.as_ref().set("abc".into());
    assert_eq!(p1.as_ref().get(), Struct { foo: 78, bar: "abc".into() });
    assert_eq!(p2.as_ref().get(), 78);
    assert_eq!(p3.as_ref().get(), "abc");

    let p4 = Rc::pin(Property::new(123));
    p2.set_binding({
        let p4 = p4.clone();
        move || p4.as_ref().get() + 1
    });

    assert_eq!(p1.as_ref().get(), Struct { foo: 124, bar: "abc".into() });
    assert_eq!(p2.as_ref().get(), 124);
    assert_eq!(p3.as_ref().get(), "abc");

    p4.as_ref().set(456);
    assert_eq!(p1.as_ref().get(), Struct { foo: 457, bar: "abc".into() });
    assert_eq!(p2.as_ref().get(), 457);
    assert_eq!(p3.as_ref().get(), "abc");

    p3.as_ref().set("def".into());
    assert_eq!(p1.as_ref().get(), Struct { foo: 457, bar: "def".into() });
    assert_eq!(p2.as_ref().get(), 457);
    assert_eq!(p3.as_ref().get(), "def");

    p4.as_ref().set(789);
    // Note that the binding with `p2 : p4+1` is broken
    assert_eq!(p1.as_ref().get(), Struct { foo: 457, bar: "def".into() });
    assert_eq!(p2.as_ref().get(), 457);
    assert_eq!(p3.as_ref().get(), "def");
}

#[cfg(test)]
mod struct_member_tests {
    use super::*;
    use alloc::string::String;

    #[derive(PartialEq, Clone, Default, Debug)]
    struct S1 {
        s1: String,
        i1: i32,
    }
    #[derive(PartialEq, Clone, Default, Debug)]
    struct S2 {
        s2: String,
        i2: i32,
    }

    fn link_s1(strct: Pin<&Property<S1>>, member: Pin<&Property<String>>) {
        Property::link_two_way_to_member(
            strct,
            member,
            "s1",
            |s: &S1| s.s1.clone(),
            |s: &mut S1, v: &String| s.s1 = v.clone(),
        );
    }
    fn link_s2(strct: Pin<&Property<S2>>, member: Pin<&Property<String>>) {
        Property::link_two_way_to_member(
            strct,
            member,
            "s2",
            |s: &S2| s.s2.clone(),
            |s: &mut S2, v: &String| s.s2 = v.clone(),
        );
    }

    /// The scenario of `two_way_binding_struct_non_const.slint`: one scalar
    /// two-way bound to fields of TWO struct properties which both carry
    /// live bindings. The class must converge while the independent fields
    /// keep tracking their own sources.
    #[test]
    fn two_structs_with_live_bindings_share_scalar() {
        let ext1 = Rc::pin(Property::new(String::from("a")));
        let ext2 = Rc::pin(Property::new(String::from("b")));
        let int1 = Rc::pin(Property::new(42));
        let int2 = Rc::pin(Property::new(43));

        let s1 = Rc::pin(Property::<S1>::default());
        let s2 = Rc::pin(Property::<S2>::default());
        let scalar = Rc::pin(Property::new(String::from("XXX")));

        // Link first, install the bindings after, like the generated Rust
        // code does (links in `init` before `property_init`).
        link_s1(s1.as_ref(), scalar.as_ref());
        link_s2(s2.as_ref(), scalar.as_ref());

        s1.as_ref().set_binding({
            let (ext1, int1) = (ext1.clone(), int1.clone());
            move || S1 { s1: ext1.as_ref().get(), i1: int1.as_ref().get() }
        });
        s2.as_ref().set_binding({
            let (ext2, int2) = (ext2.clone(), int2.clone());
            move || S2 { s2: ext2.as_ref().get(), i2: int2.as_ref().get() }
        });

        // The class converges (last installed binding drives it).
        let (v1, v2, vs) = (s1.as_ref().get(), s2.as_ref().get(), scalar.as_ref().get());
        assert_eq!(v1.s1, v2.s2);
        assert_eq!(v1.s1, vs);
        assert_eq!(v1.s1, "b");
        // Independent fields keep tracking their sources.
        assert_eq!(v1.i1, 42);
        assert_eq!(v2.i2, 43);
        int1.as_ref().set(1042);
        assert_eq!(s1.as_ref().get().i1, 1042);
        assert_eq!(s1.as_ref().get().s1, s2.as_ref().get().s2);

        // A write to the scalar reaches every member of the class.
        scalar.as_ref().set("written".into());
        assert_eq!(s1.as_ref().get().s1, "written");
        assert_eq!(s2.as_ref().get().s2, "written");
        assert_eq!(s1.as_ref().get().i1, 1042);
        assert_eq!(s2.as_ref().get().i2, 43);
        // The drivers were dropped by the write; sources no longer drive
        // the shared field, but still drive the independent fields.
        ext1.as_ref().set("ignored".into());
        ext2.as_ref().set("ignored".into());
        int2.as_ref().set(2043);
        assert_eq!(s1.as_ref().get().s1, "written");
        assert_eq!(s2.as_ref().get().s2, "written");
        assert_eq!(s2.as_ref().get().i2, 2043);
    }

    /// Three struct members converge structurally (single shared common).
    #[test]
    fn three_structs_converge() {
        let s1 = Rc::pin(Property::new(S1 { s1: "one".into(), i1: 1 }));
        let s2 = Rc::pin(Property::new(S2 { s2: "two".into(), i2: 2 }));
        let s3 = Rc::pin(Property::new(S1 { s1: "three".into(), i1: 3 }));
        let scalar = Rc::pin(Property::new(String::from("XXX")));

        link_s1(s1.as_ref(), scalar.as_ref());
        link_s2(s2.as_ref(), scalar.as_ref());
        link_s1(s3.as_ref(), scalar.as_ref());

        // Last linked right-hand side wins.
        assert_eq!(scalar.as_ref().get(), "three");
        assert_eq!(s1.as_ref().get().s1, "three");
        assert_eq!(s2.as_ref().get().s2, "three");
        assert_eq!(s3.as_ref().get().s1, "three");

        // Writes converge from every member, in both directions.
        s2.as_ref().set(S2 { s2: "via-s2".into(), i2: 22 });
        assert_eq!(scalar.as_ref().get(), "via-s2");
        assert_eq!(s1.as_ref().get().s1, "via-s2");
        assert_eq!(s3.as_ref().get().s1, "via-s2");
        assert_eq!(s1.as_ref().get().i1, 1);
        scalar.as_ref().set("via-scalar".into());
        assert_eq!(s1.as_ref().get().s1, "via-scalar");
        assert_eq!(s2.as_ref().get().s2, "via-scalar");
        assert_eq!(s3.as_ref().get().s1, "via-scalar");
        assert_eq!(s2.as_ref().get().i2, 22);
    }

    /// A `set` on the struct pushes the mapped field into the class and
    /// drops the struct's binding, like on an unwrapped property.
    #[test]
    fn set_on_struct_pushes_and_clears_binding() {
        let source = Rc::pin(Property::new(String::from("from-binding")));
        let s1 = Rc::pin(Property::<S1>::default());
        let scalar = Rc::pin(Property::<String>::default());

        link_s1(s1.as_ref(), scalar.as_ref());
        s1.as_ref().set_binding({
            let source = source.clone();
            move || S1 { s1: source.as_ref().get(), i1: 7 }
        });
        assert_eq!(scalar.as_ref().get(), "from-binding");
        assert_eq!(s1.as_ref().get().i1, 7);

        s1.as_ref().set(S1 { s1: "set-value".into(), i1: 8 });
        assert_eq!(scalar.as_ref().get(), "set-value");
        assert_eq!(s1.as_ref().get(), S1 { s1: "set-value".into(), i1: 8 });
        // binding dropped
        source.as_ref().set("changed".into());
        assert_eq!(scalar.as_ref().get(), "set-value");
        assert_eq!(s1.as_ref().get(), S1 { s1: "set-value".into(), i1: 8 });
    }

    /// A member write propagates into the struct without disturbing its
    /// other fields, and dependents of both get notified.
    #[test]
    fn member_write_dirties_dependents() {
        let s1 = Rc::pin(Property::new(S1 { s1: "init".into(), i1: 1 }));
        let scalar = Rc::pin(Property::<String>::default());
        link_s1(s1.as_ref(), scalar.as_ref());

        let depends = Box::pin(Property::new(String::new()));
        depends.as_ref().set_binding({
            let s1 = s1.clone();
            move || s1.as_ref().get().s1
        });
        assert_eq!(depends.as_ref().get(), "init");
        scalar.as_ref().set("poked".into());
        assert_eq!(depends.as_ref().get(), "poked");
        assert_eq!(s1.as_ref().get().i1, 1);
    }

    /// Re-assigning a binding to a linked struct property must dirty the
    /// struct's dependents even though the assignment is intercepted by the
    /// wrapper (regression test for the `set_binding_impl` early return).
    #[test]
    fn rebinding_after_link_dirties_dependents() {
        let s1 = Rc::pin(Property::new(S1 { s1: "init".into(), i1: 1 }));
        let scalar = Rc::pin(Property::<String>::default());
        link_s1(s1.as_ref(), scalar.as_ref());

        let tracker = Box::pin(<PropertyTracker>::default());
        let value = tracker.as_ref().evaluate({
            let s1 = s1.clone();
            move || s1.as_ref().get()
        });
        assert_eq!(value.s1, "init");
        assert!(!tracker.as_ref().is_dirty());

        s1.as_ref().set_binding(move || S1 { s1: "rebound".into(), i1: 2 });
        assert!(tracker.as_ref().is_dirty(), "dependents must be notified of the new binding");
        assert_eq!(s1.as_ref().get(), S1 { s1: "rebound".into(), i1: 2 });
        assert_eq!(scalar.as_ref().get(), "rebound");
    }

    /// Driver election: the struct's live binding drives the class (single
    /// driver behaves like today's wide design); with two drivers the last
    /// installed one wins deterministically.
    #[test]
    fn driver_election() {
        let source1 = Rc::pin(Property::new(String::from("one")));
        let source2 = Rc::pin(Property::new(String::from("two")));
        let s1 = Rc::pin(Property::<S1>::default());
        let s2 = Rc::pin(Property::<S2>::default());
        let scalar = Rc::pin(Property::<String>::default());

        link_s1(s1.as_ref(), scalar.as_ref());
        s1.as_ref().set_binding({
            let source1 = source1.clone();
            move || S1 { s1: source1.as_ref().get(), i1: 1 }
        });
        // single driver: the binding's output reaches the member reactively
        assert_eq!(scalar.as_ref().get(), "one");
        source1.as_ref().set("one-b".into());
        assert_eq!(scalar.as_ref().get(), "one-b");

        link_s2(s2.as_ref(), scalar.as_ref());
        s2.as_ref().set_binding({
            let source2 = source2.clone();
            move || S2 { s2: source2.as_ref().get(), i2: 2 }
        });
        // two drivers: last installed wins, values stay converged
        assert_eq!(scalar.as_ref().get(), "two");
        assert_eq!(s1.as_ref().get().s1, "two");
        source2.as_ref().set("two-b".into());
        assert_eq!(scalar.as_ref().get(), "two-b");
        assert_eq!(s1.as_ref().get().s1, "two-b");
        assert_eq!(s2.as_ref().get().s2, "two-b");
        // repeated reads are stable (no flip-flopping between drivers)
        assert_eq!(s1.as_ref().get().s1, "two-b");
        assert_eq!(scalar.as_ref().get(), "two-b");
    }

    /// Re-linking the same field (conditional component re-instantiation)
    /// must not grow the mapping list and must keep the class value.
    #[test]
    fn relink_is_idempotent() {
        let strct = Rc::pin(Property::new(S1 { s1: "start".into(), i1: 1 }));

        let mut previous_scalar: Option<Pin<Rc<Property<String>>>> = None;
        for iteration in 0..5 {
            // fresh "child" scalar every round, like a re-instantiated
            // conditional component
            let scalar = Rc::pin(Property::<String>::default());
            link_s1(strct.as_ref(), scalar.as_ref());

            let mapping_count = strct
                .as_ref()
                .with_struct_member_bindings(|wrapper| wrapper.mappings.borrow().len())
                .unwrap();
            assert_eq!(mapping_count, 1, "mapping list must not grow (iteration {iteration})");

            // the class value survives the re-link
            let expected = if iteration == 0 { "start" } else { "poked" };
            assert_eq!(scalar.as_ref().get(), expected);
            assert_eq!(strct.as_ref().get().s1, expected);

            scalar.as_ref().set("poked".into());
            assert_eq!(strct.as_ref().get().s1, "poked");
            previous_scalar = Some(scalar);
        }
        drop(previous_scalar);
    }

    /// The scenario of `issue_11415_two_way_if_struct.slint` after
    /// decomposition: a whole-struct link decomposed into a member-member
    /// cell link, re-established against a fresh struct while the value was
    /// changed only through the class (never read back through the struct).
    #[test]
    fn members_link_relink_keeps_class_value() {
        let root_data = Rc::pin(Property::new(S1 { s1: String::new(), i1: 0 }));
        let counter = Rc::pin(Property::<i32>::default());
        Property::link_two_way_to_member(
            root_data.as_ref(),
            counter.as_ref(),
            "i1",
            |s: &S1| s.i1,
            |s: &mut S1, v: &i32| s.i1 = *v,
        );

        let link_page = |page_data: Pin<&Property<S1>>| {
            Property::link_two_way_members(
                page_data,
                "i1",
                |s: &S1| s.i1,
                |s: &mut S1, v: &i32| s.i1 = *v,
                root_data.as_ref(),
                "i1",
                |s: &S1| s.i1,
                |s: &mut S1, v: &i32| s.i1 = *v,
            );
        };

        let page1_data = Rc::pin(Property::<S1>::default());
        link_page(page1_data.as_ref());
        assert_eq!(root_data.as_ref().get().i1, 0);

        // change through the class only; root_data's cached value stays stale
        counter.as_ref().set(42);

        // toggle: drop the first page, link a fresh one
        drop(page1_data);
        let page2_data = Rc::pin(Property::<S1>::default());
        link_page(page2_data.as_ref());

        assert_eq!(root_data.as_ref().get().i1, 42);
        assert_eq!(page2_data.as_ref().get().i1, 42);
        assert_eq!(counter.as_ref().get(), 42);
    }

    /// Dependency-list transfer when a bound property is linked: dependents
    /// registered on the old binding must survive the move into the wrapper
    /// (analog of `test_two_way_with_map_dependency_list_transfer`).
    #[test]
    fn dependency_list_transfer() {
        let source = Rc::pin(Property::new(10));

        // dropped last, so its dependency node outlives the properties
        let tracker = Box::pin(<PropertyTracker>::default());

        let strct = Rc::pin(Property::<S1>::default());
        strct.as_ref().set_binding({
            let source = source.clone();
            move || S1 { s1: "x".into(), i1: source.as_ref().get() * 2 }
        });
        assert_eq!(strct.as_ref().get().i1, 20);

        let value = tracker.as_ref().evaluate({
            let strct = strct.clone();
            move || strct.as_ref().get().i1
        });
        assert_eq!(value, 20);
        assert!(!tracker.as_ref().is_dirty());

        let scalar = Rc::pin(Property::<String>::default());
        link_s1(strct.as_ref(), scalar.as_ref());

        // the link replaced the struct's direct binding with the wrapper;
        // the tracker's dependency must have been transferred and notified
        assert!(tracker.as_ref().is_dirty());
        assert_eq!(strct.as_ref().get().i1, 20);
        source.as_ref().set(50);
        assert_eq!(strct.as_ref().get().i1, 100);
        assert_eq!(scalar.as_ref().get(), "x");
    }

    /// Nested field paths keyed by their full path work independently.
    #[test]
    fn nested_and_multiple_fields() {
        #[derive(PartialEq, Clone, Default, Debug)]
        struct Outer {
            inner: S1,
            z: i32,
        }
        let outer = Rc::pin(Property::new(Outer {
            inner: S1 { s1: "deep".into(), i1: 5 },
            z: 9,
        }));
        let deep_scalar = Rc::pin(Property::<String>::default());
        let z_scalar = Rc::pin(Property::<i32>::default());
        Property::link_two_way_to_member(
            outer.as_ref(),
            deep_scalar.as_ref(),
            "inner.s1",
            |s: &Outer| s.inner.s1.clone(),
            |s: &mut Outer, v: &String| s.inner.s1 = v.clone(),
        );
        Property::link_two_way_to_member(
            outer.as_ref(),
            z_scalar.as_ref(),
            "z",
            |s: &Outer| s.z,
            |s: &mut Outer, v: &i32| s.z = *v,
        );
        assert_eq!(deep_scalar.as_ref().get(), "deep");
        assert_eq!(z_scalar.as_ref().get(), 9);
        deep_scalar.as_ref().set("deeper".into());
        z_scalar.as_ref().set(10);
        assert_eq!(
            outer.as_ref().get(),
            Outer { inner: S1 { s1: "deeper".into(), i1: 5 }, z: 10 }
        );
        outer.as_ref().set(Outer { inner: S1 { s1: "over".into(), i1: 6 }, z: 11 });
        assert_eq!(deep_scalar.as_ref().get(), "over");
        assert_eq!(z_scalar.as_ref().get(), 11);
    }

    /// Linking a member that is itself already part of a scalar two-way
    /// class reuses that class' common (alias-collapsed scalars).
    #[test]
    fn member_already_linked_scalar() {
        let scalar_a = Rc::pin(Property::new(String::from("A")));
        let scalar_b = Rc::pin(Property::new(String::from("B")));
        Property::link_two_way(scalar_a.as_ref(), scalar_b.as_ref());
        assert_eq!(scalar_a.as_ref().get(), "B");

        let strct = Rc::pin(Property::new(S1 { s1: "S".into(), i1: 0 }));
        link_s1(strct.as_ref(), scalar_a.as_ref());
        // the struct is the right-hand side: its value wins
        assert_eq!(scalar_a.as_ref().get(), "S");
        assert_eq!(scalar_b.as_ref().get(), "S");
        scalar_b.as_ref().set("via-b".into());
        assert_eq!(strct.as_ref().get().s1, "via-b");
        assert_eq!(scalar_a.as_ref().get(), "via-b");
    }
}

/// Regression test for use-after-free in `link_two_way_with_map_to_common_property`.
///
/// When a property already has a binding with dependant properties and is then linked
/// via `link_two_way_with_map`, the old binding's dependency list must be
/// transferred to the new `TwoWayBindingWithMap` binding. Without this
/// transfer, dependency nodes would point into freed memory, causing
/// panics in `DependencyNode::debug_assert_valid` on drop.
///
/// The drop order is arranged so that the tracker (which owns the
/// dependency node) outlives the properties, forcing the node to be
/// removed after the old binding would have been freed.
#[test]
fn test_two_way_with_map_dependency_list_transfer() {
    #[derive(PartialEq, Clone, Default, Debug)]
    struct Wrapper {
        value: i32,
    }

    let source = Rc::pin(Property::new(10));

    // Declare the tracker before the properties so it is dropped *after*
    // them (Rust drops locals in reverse declaration order). This ensures
    // the dependency node outlives the old binding.
    let tracker = Box::pin(<PropertyTracker>::default());

    let p_field = Rc::pin(Property::new(0i32));
    p_field.as_ref().set_binding({
        let source = source.clone();
        move || source.as_ref().get() * 2
    });
    assert_eq!(p_field.as_ref().get(), 20);

    // Evaluate the tracker, which reads p_field and registers a dependency
    // node on p_field's binding's dependency list.
    let val = tracker.as_ref().evaluate({
        let p_field = p_field.clone();
        move || p_field.as_ref().get()
    });
    assert_eq!(val, 20);
    assert!(!tracker.as_ref().is_dirty());

    // link_two_way_with_map replaces p_field's binding with a
    // TwoWayBindingWithMap. The old closure binding is dropped
    // (prop1's value wins), so p_field now reads from the common
    // property initialized from p_struct.
    let p_struct = Rc::pin(Property::new(Wrapper { value: 0 }));
    Property::link_two_way_with_map(
        p_struct.as_ref(),
        p_field.as_ref(),
        |s| s.value,
        |s, v| s.value = *v,
    );

    assert_eq!(p_field.as_ref().get(), 0);
    assert_eq!(p_struct.as_ref().get(), Wrapper { value: 0 });

    // The binding replacement dirtied the tracker via the transferred
    // dependency list.
    assert!(tracker.as_ref().is_dirty());

    // Implicit drop order: p_struct, p_field, tracker, source.
    // p_field's drop frees the TwoWayBindingWithMap binding.
    // tracker is dropped afterwards — its DependencyNode::remove
    // would panic in debug_assert_valid if the dependency list was
    // not properly transferred.
}
