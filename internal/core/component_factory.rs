// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#![warn(missing_docs)]

//! This module defines a `ComponentFactory` and related code.
use core::fmt::Debug;

use alloc::boxed::Box;
use alloc::rc::Rc;

use crate::{
    api::ComponentHandle,
    component::{ComponentRc, ComponentVTable},
};

#[derive(Clone)]
struct ComponentFactoryInner(Rc<dyn Fn() -> Option<ComponentRc> + 'static>);

impl PartialEq for ComponentFactoryInner {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Debug for ComponentFactoryInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ComponentFactoryData").finish()
    }
}

/// A `ComponentFactory` can be used to create new Components at runtime,
/// taking a factory function with no arguments and returning
/// a [`ComponentHandle`].
///
/// A `ComponentFactory` implements the `component-factory` type for
/// properties in the Slint language.
///
/// The `component-factory` is used by an `ComponentContainer` element in Slint
/// files to embed UI elements based on the produced component within the
/// `ComponentContainer` element.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentFactory(Option<ComponentFactoryInner>);

impl ComponentFactory {
    /// Create a new `ComponentFactory`
    pub fn new<T: ComponentHandle + 'static>(factory: impl Fn() -> Option<T> + 'static) -> Self
    where
        T::Inner: vtable::HasStaticVTable<ComponentVTable> + 'static,
    {
        let factory = Box::new(factory) as Box<dyn Fn() -> Option<T> + 'static>;

        Self(Some(ComponentFactoryInner(Rc::new(move || -> Option<ComponentRc> {
            let product = (factory)();
            product.map(|p| vtable::VRc::into_dyn(p.as_weak().inner().upgrade().unwrap()))
        }))))
    }

    /// Build a `Component`
    pub(crate) fn build(&self) -> Option<ComponentRc> {
        self.0.as_ref().and_then(|b| (b.0)()).into()
    }
}

#[cfg(feature = "ffi")]
#[allow(unsafe_code)]
pub(crate) mod ffi {
    use core::ffi::c_void;
    use core::marker::PhantomData;

    use super::{ComponentFactory as RustComponentFactory, ComponentFactoryInner, ComponentRc, Rc};

    /// Same layout as `ComponentFactory`
    #[repr(C)]
    pub struct ComponentFactory<'a>(*const c_void, *const c_void, PhantomData<&'a ()>);

    struct FfiData {
        build: unsafe extern "C" fn(*mut c_void, *mut c_void),
        data: *mut c_void,
        drop_data: unsafe extern "C" fn(*mut c_void),
    }

    impl Drop for FfiData {
        fn drop(&mut self) {
            unsafe { (self.drop_data)(self.data) };
            self.data = core::ptr::null_mut();
        }
    }

    unsafe fn component_factory<'a>(
        factory: *const ComponentFactory<'a>,
    ) -> &'a RustComponentFactory {
        assert_eq!(
            core::mem::size_of::<RustComponentFactory>(),
            core::mem::size_of::<ComponentFactory>()
        );
        assert_eq!(
            core::mem::align_of::<RustComponentFactory>(),
            core::mem::align_of::<ComponentFactory>()
        );

        (factory as *const RustComponentFactory).as_ref().unwrap()
    }

    unsafe fn component_factory_mut<'a>(
        factory: *mut ComponentFactory<'a>,
    ) -> &'a mut RustComponentFactory {
        assert_eq!(
            core::mem::size_of::<RustComponentFactory>(),
            core::mem::size_of::<ComponentFactory>()
        );
        assert_eq!(
            core::mem::align_of::<RustComponentFactory>(),
            core::mem::align_of::<ComponentFactory>()
        );

        (factory as *mut RustComponentFactory).as_mut().unwrap()
    }

    /// # Safety
    /// This must be called using a non-null pointer pointing to a `ComponentFactory`
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_factory_init_from_raw(
        this: *mut ComponentFactory,
        build: unsafe extern "C" fn(*mut c_void, *mut c_void),
        data: *mut c_void,
        drop_data: unsafe extern "C" fn(*mut c_void),
    ) {
        let data = FfiData { build, data, drop_data };
        let function = move || -> Option<ComponentRc> {
            let mut result: Option<ComponentRc> = None;
            (data.build)(data.data, &mut result as *mut Option<ComponentRc> as *mut c_void);
            return result;
        };
        component_factory_mut(this).0 = Some(ComponentFactoryInner(Rc::new(function)));
    }

    /// # Safety
    /// This must be called using a non-null pointers pointing to a `ComponentFactory`
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_factory_clone(
        from: *const ComponentFactory,
        to: *mut ComponentFactory,
    ) {
        component_factory_mut(to).0 = component_factory(from).0.clone();
    }

    /// # Safety
    /// This must be called using a non-null pointer pointing to a `ComponentFactory`
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_factory_move(
        from: *mut ComponentFactory,
        to: *mut ComponentFactory,
    ) {
        let default_value = None;
        let value = core::mem::replace(&mut component_factory_mut(from).0, default_value);
        component_factory_mut(to).0 = value;
    }

    /// # Safety
    /// This must be called using a non-null pointer pointing to an initialized `ComponentFactory`
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_factory_free(this: *mut ComponentFactory) {
        drop(component_factory_mut(this));
    }

    /// # Safety
    /// This must be called using a non-null pointer pointing to an initialized `ComponentFactory`
    #[no_mangle]
    pub unsafe extern "C" fn slint_component_factory_eq(
        lhs: *const ComponentFactory,
        rhs: *const ComponentFactory,
    ) -> bool {
        // Allow for aliasing, then forward to Rust...
        core::ptr::eq(lhs, rhs) || component_factory(lhs) == component_factory(rhs)
    }
}
