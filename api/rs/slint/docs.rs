// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(doc)]
/*!
    This is a pseudo module which only exist for documentation purposes as a way to show
    the Slint documentation as part of rustdoc.

    - The [`generated_code`] module contains an [commented example](generated_code::SampleComponent)
      of what is generated from the `.slint` file
*/

// cSpell: ignore rustdoc

/// This module exists only to explain the API of the code generated from `.slint` design markup. Its described structure
/// is not really contained in the compiled crate.
pub mod generated_code {

    use crate::ComponentHandle;
    use crate::Global;
    use crate::Weak;
    use crate::Window;

    /// This an example of the API that is generated for a component in `.slint` design markup. This may help you understand
    /// what functions you can call and how you can pass data in and out.
    ///
    /// This is the source code:
    ///
    /// ```slint,no-preview
    /// export component SampleComponent inherits Window {
    ///     in-out property<int> counter;
    ///     // note that dashes will be replaced by underscores in the generated code
    ///     in-out property<string> user-name;
    ///     callback hello;
    ///     public function do-something(x: int) -> bool { return x > 0; }
    ///     // ... maybe more elements here
    /// }
    /// ```
    #[derive(Clone)]
    pub struct SampleComponent {
        _marker: core::marker::PhantomData<*mut ()>,
    }
    impl SampleComponent {
        /// Creates a new instance that is reference counted and pinned in memory.
        pub fn new() -> Result<Self, crate::PlatformError> {
            unimplemented!()
        }

        /// A getter is generated for each property declared at the root of the component.
        /// In this case, this is the getter that returns the value of the `counter`
        /// property declared in the `.slint` design markup.
        pub fn get_counter(&self) -> i32 {
            unimplemented!()
        }
        /// A setter is generated for each property declared at the root of the component,
        /// In this case, this is the setter that sets the value of the `counter` property
        /// declared in the `.slint` design markup.
        pub fn set_counter(&self, value: i32) {}
        /// Returns the value of the `user_name` property declared in the `.slint` design markup.
        pub fn get_user_name(&self) -> crate::SharedString {
            unimplemented!()
        }
        /// Assigns a new value to the `user_name` property.
        pub fn set_user_name(&self, value: crate::SharedString) {}

        /// For each callback declared at the root of the component, a function to synchronously call that
        /// callback is generated. This is the function that calls the `hello` callback declared
        /// in the `.slint` design markup.
        pub fn invoke_hello(&self) {}
        /// For each callback declared at the root of the component, a function connect to that callback
        /// is generated. This is the function that registers the function f as callback when the
        /// callback `hello` is emitted. In order to access
        /// the component in the callback, you'd typically capture a weak reference obtained using
        /// [`ComponentHandle::as_weak`]
        /// and then upgrade it to a strong reference when the callback is run:
        /// ```ignore
        ///     let sample = SampleComponent::new().unwrap();
        ///     let sample_weak = sample.as_weak();
        ///     sample.as_ref().on_hello(move || {
        ///         let sample = sample_weak.unwrap();
        ///         sample.as_ref().set_counter(42);
        ///     });
        /// ```
        pub fn on_hello(&self, f: impl Fn() + 'static) {}

        /// For each public function declared at the root of the component, a function to synchronously call
        /// that function is generated. This is the function that calls the `do-something` function
        /// declared in the `.slint` design markup.
        pub fn invoke_do_something(&self, d: i32) -> bool {
            unimplemented!()
        }
    }

    impl ComponentHandle for SampleComponent {
        #[doc(hidden)]
        type Inner = SampleComponent;

        /// Returns a new weak pointer.
        fn as_weak(&self) -> Weak<Self> {
            unimplemented!()
        }

        /// Returns a clone of this handle that's a strong reference.
        fn clone_strong(&self) -> Self {
            unimplemented!();
        }

        #[doc(hidden)]
        fn from_inner(
            _: vtable::VRc<crate::private_unstable_api::re_exports::ItemTreeVTable, Self::Inner>,
        ) -> Self {
            unimplemented!();
        }

        /// Convenience function for [`crate::Window::show()`]. This shows the window on the screen
        /// and maintains an extra strong reference while the window is visible. To react
        /// to events from the windowing system, such as draw requests or mouse/touch input, it is
        /// still necessary to spin the event loop, using [`crate::run_event_loop`].
        fn show(&self) -> Result<(), crate::PlatformError> {
            unimplemented!();
        }

        /// Convenience function for [`crate::Window::hide()`]. Hides the window, so that it is not
        /// visible anymore. The additional strong reference on the associated component, that was
        /// created when show() was called, is dropped.
        fn hide(&self) -> Result<(), crate::PlatformError> {
            unimplemented!();
        }

        /// Returns the Window associated with this component. The window API can be used
        /// to control different aspects of the integration into the windowing system,
        /// such as the position on the screen.
        fn window(&self) -> &Window {
            unimplemented!()
        }

        /// This is a convenience function that first calls [`Self::show`], followed by [`crate::run_event_loop()`]
        /// and [`Self::hide`].
        fn run(&self) -> Result<(), crate::PlatformError> {
            unimplemented!();
        }

        /// This function provides access to instances of global singletons exported in `.slint`.
        fn global<'a, T: Global<'a, Self>>(&'a self) -> T {
            unimplemented!()
        }
    }
}

pub mod mcu {
    #![doc = include_str!("mcu.md")]
    #[cfg(feature = "renderer-software")]
    use crate::platform::software_renderer::*;
    use crate::platform::*;
    mod slint {
        pub use crate::*;
    }
}

#[i_slint_core_macros::slint_doc]
pub mod cargo_features {
    //! # Feature flags and backend selection.
    //! Use the following feature flags in your Cargo.toml to enable additional features.
    //!
    #![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
    //!
    //! More information about the backend and renderers is available in the
    //![Slint Documentation](slint:backends_and_renderers)")]
    use crate::*;
}

pub mod type_mappings {
    #![doc = include_str!("type-mappings.md")]
    use crate::*;
}
