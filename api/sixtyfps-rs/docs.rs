/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![cfg(doc)]
/*!
    This is a pseudo module which only exist for documentation purposes as a way to show
    the SixtyFPS documentation as part of rustdoc.

    - The [`generated_code`] module contains an [commented example](generated_code::SampleComponent)
      of what is generated from the `.60` file
    - The [`langref`] module is the reference documentation for the `.60` language.
    - The [`widgets`] and [`builtin_elements`] modules contains the documentation of elements usable
      within the `.60` files
*/

pub mod langref {
    #![doc(include = "docs/langref.md")]
    #![doc = ""]
}

#[cfg(all(doc, nightly))]
pub mod builtin_elements {
    #![doc(include = "docs/builtin_elements.md")]
    #![doc = ""]
}

#[cfg(all(doc, nightly))]
pub mod widgets {
    #![doc(include = "docs/widgets.md")]
    #![doc = ""]
}

/// This module exists only to explain the API of the code generated from `.60` design markup. Its described structure
/// is not really contained in the compiled crate.

pub mod generated_code {

    use crate::re_exports;

    /// This an example of the API that is generated for a component in `.60` design markup. This may help you understand
    /// what functions you can call and how you can pass data in and out.
    /// This is the source code:
    /// ```60
    /// SampleComponent := Window {
    ///     property<int> counter;
    ///     property<string> user_name;
    ///     signal hello;
    ///     /// ... maybe more elements here
    /// }
    /// ```
    pub struct SampleComponent {}
    impl SampleComponent {
        /// Creates a new instance that is reference counted and pinned in memory.
        pub fn new() -> core::pin::Pin<std::rc::Rc<Self>> {
            unimplemented!()
        }
        /// Creates a window on the screen, renders this component in it and spins an event loop to react
        /// to user input. A typical sequence of creating an instance and showing it may look like this:
        /// ```ignore
        /// fn main() {
        ///     let sample = SampleComponent::new();
        ///     /// other setup code here, connect to signal handlers, set property values
        ///     sample.run();
        /// }
        /// ```
        pub fn run(self: core::pin::Pin<std::rc::Rc<Self>>) {}
        /// Returns a weak pointer for an instance of this component. You can use this to in captures of
        /// closures, for example signal handlers, to access the component later.
        pub fn as_weak(self: core::pin::Pin<std::rc::Rc<Self>>) -> re_exports::PinWeak<Self> {
            unimplemented!()
        }
        /// A getter is generated for each property declared at the root of the component.
        /// In this case, this is the getter that returns the value of the `counter`
        /// property declared in the `.60` design markup.
        pub fn get_counter(self: ::core::pin::Pin<&Self>) -> i32 {
            unimplemented!()
        }
        /// A setter is generated for each property declared at the root of the component,
        /// In this case, this is the setter that sets the value of the `counter` property
        /// declared in the `.60` design markup.
        pub fn set_counter(&self, value: i32) {}
        /// Returns the value of the `user_name` property declared in the `.60` design markup.
        pub fn get_user_name(self: ::core::pin::Pin<&Self>) -> re_exports::SharedString {
            unimplemented!()
        }
        /// Assigns a new value to the `user_name` property.
        pub fn set_user_name(&self, value: re_exports::SharedString) {}
        /// For each signal declared at the root of the component, a function to emit that
        /// signal is generated. This is the function that emits the `hello` signal declared
        /// in the `.60` design markup.
        pub fn emit_hello(self: ::core::pin::Pin<&Self>) {}
        /// For each signal declared at the root of the component, a function connect to that signal
        /// is generated. This is the function that registers the function f as callback when the
        /// signal `hello` is emitted. In order to access
        /// the component in the callback, you'd typically capture a weak reference obtained using
        /// [`SampleComponent::as_weak`]
        /// and then upgrade it to a strong reference when the callback is run:
        /// ```ignore
        ///     let sample = SampleComponent::new();
        ///     let sample_weak = sample.clone().as_weak();
        ///     sample.as_ref().on_hello(move || {
        ///         let sample = sample_weak.upgrade().unwrap();
        ///         sample.as_ref().set_counter(42);
        ///     });
        /// ```
        pub fn on_hello(self: ::core::pin::Pin<&Self>, f: impl Fn() + 'static) {}
    }
}
