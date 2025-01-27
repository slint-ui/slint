// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]

//! This module defines a `ComponentFactory` and related code.
use crate::api::ComponentHandle;
use crate::item_tree::{ItemTreeRc, ItemTreeVTable, ItemTreeWeak};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::fmt::Debug;

/// The `FactoryContext` provides extra information to the ComponentFactory
pub struct FactoryContext {
    /// The item tree to embed the factory product into
    pub parent_item_tree: ItemTreeWeak,
    /// The index in the parent item tree with the dynamic node to connect
    /// the factories product to.
    pub parent_item_tree_index: u32,
}

#[derive(Clone)]
struct ComponentFactoryInner(Rc<dyn Fn(FactoryContext) -> Option<ItemTreeRc> + 'static>);

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
/// taking a factory function returning a [`ComponentHandle`].
///
/// The `FactoryContext` is passed to that factory function.
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
    pub fn new<T: ComponentHandle + 'static>(
        factory: impl Fn(FactoryContext) -> Option<T> + 'static,
    ) -> Self
    where
        T::Inner: vtable::HasStaticVTable<ItemTreeVTable> + 'static,
    {
        let factory = Box::new(factory) as Box<dyn Fn(FactoryContext) -> Option<T> + 'static>;

        Self(Some(ComponentFactoryInner(Rc::new(move |ctx| -> Option<ItemTreeRc> {
            let product = (factory)(ctx);
            product.map(|p| vtable::VRc::into_dyn(p.as_weak().inner().upgrade().unwrap()))
        }))))
    }

    /// Build a `Component`
    pub(crate) fn build(&self, ctx: FactoryContext) -> Option<ItemTreeRc> {
        self.0.as_ref().and_then(move |b| (b.0)(ctx))
    }
}
