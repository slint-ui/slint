// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(doc)]
/*!
    This is a pseudo module which only exist for documentation purposes as a way to show
    the Slint documentation as part of rustdoc.

    - The [`generated_code`] module contains an [commented example](generated_code::SampleComponent)
      of what is generated from the `.slint` file
*/

pub mod recipes {
    #![doc = include_str!("docs/recipes/recipes.md")]

    // So intra-doc links can refer it as `slint::`
    use crate as slint;
}

/// This module exists only to explain the API of the code generated from `.slint` design markup. Its described structure
/// is not really contained in the compiled crate.
pub mod generated_code {

    use crate::ComponentHandle;
    use crate::Global;
    use crate::Weak;
    use crate::Window;

    /// This an example of the API that is generated for a component in `.slint` design markup. This may help you understand
    /// what functions you can call and how you can pass data in and out.
    /// This is the source code:
    /// ```slint
    /// SampleComponent := Window {
    ///     property<int> counter;
    ///     // note that dashes will be replaced by underscores in the generated code
    ///     property<string> user-name;
    ///     callback hello();
    ///     // ... maybe more elements here
    /// }
    /// ```
    #[derive(Clone)]
    pub struct SampleComponent {}
    impl SampleComponent {
        /// Creates a new instance that is reference counted and pinned in memory.
        pub fn new() -> Self {
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
        /// For each callback declared at the root of the component, a function to call that
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
        ///     let sample = SampleComponent::new();
        ///     let sample_weak = sample.as_weak();
        ///     sample.as_ref().on_hello(move || {
        ///         let sample = sample_weak.unwrap();
        ///         sample.as_ref().set_counter(42);
        ///     });
        /// ```
        pub fn on_hello(&self, f: impl Fn() + 'static) {}
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
            _: vtable::VRc<crate::private_unstable_api::re_exports::ComponentVTable, Self::Inner>,
        ) -> Self {
            unimplemented!();
        }

        /// Marks the window of this component to be shown on the screen. This registers
        /// the window with the windowing system. In order to react to events from the windowing system,
        /// such as draw requests or mouse/touch input, it is still necessary to spin the event loop,
        /// using [`crate::run_event_loop`].
        fn show(&self) {
            unimplemented!();
        }

        /// Marks the window of this component to be hidden on the screen. This de-registers
        /// the window from the windowing system and it will not receive any further events.
        fn hide(&self) {
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
        fn run(&self) {
            unimplemented!();
        }

        /// This function provides access to instances of global singletons exported in `.slint`.
        fn global<'a, T: Global<'a, Self>>(&'a self) -> T {
            unimplemented!()
        }
    }
}

pub mod debugging_techniques {
    #![doc = include_str!("docs/debugging_techniques.md")]
    #![doc = ""]
}

pub mod mcu {
    #![doc = include_str!("mcu.md")]
    use crate::platform::software_renderer::*;
    use crate::platform::*;
    mod slint {
        pub use crate::*;
    }
}
