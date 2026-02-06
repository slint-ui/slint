// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use alloc::rc::Rc;
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
            if unsafe { *&raw const (*holder).is_two_way_binding } {
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

        let value = prop2.get_internal();

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
            // If prop2 is a binding, just "steal it"
            prop2.handle.handle.set(0);
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
                prop1.handle.handle.set(0);
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
        Self::link_two_way_with_map_to_common_property(common_property, prop2, map_to, map_from);
    }

    /// Make a two way binding between the common property and the binding prop2.
    /// if prop2 has a binding, it will be preserved
    pub(crate) fn link_two_way_with_map_to_common_property<T2: PartialEq + Clone + 'static>(
        common_property: Pin<Rc<Self>>,
        prop2: Pin<&Property<T2>>,
        map_to: impl Fn(&T) -> T2 + Clone + 'static,
        map_from: impl Fn(&mut T, &T2) + Clone + 'static,
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
                    ((*self.b).vtable.evaluate)(self.b, &mut sub_value as *mut T2 as *mut ());
                }
                (self.map_from)(value, &sub_value);
                BindingResult::KeepBinding
            }

            fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
                let sub_value = (self.map_to)(value);
                // Safety: `self.b` is a BindingHolder that expects a `T2`
                unsafe {
                    ((*self.b).vtable.intercept_set)(self.b, &sub_value as *const T2 as *const ())
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

        let old_handle = prop2.handle.handle.get();
        let old_pointer = PropertyHandle::pointer_to_binding(old_handle);
        if old_pointer.is_some() {
            prop2.handle.handle.set(0);
        }

        unsafe {
            prop2.handle.set_binding(
                TwoWayBindingWithMap { common_property, map_to, map_from, marker: PhantomData },
                #[cfg(slint_debug_property)]
                debug_name.as_str(),
            );

            if let Some(binding) = old_pointer {
                prop2.handle.set_binding_impl(binding);
            }
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
