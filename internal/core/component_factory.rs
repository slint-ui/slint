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
    pub(crate) fn _build(&self) -> Option<ComponentRc> {
        self.0.as_ref().and_then(|b| (b.0)()).into()
    }
}
