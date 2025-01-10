<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Generated Code

The Slint compiler [called by the build system](cmake_reference.md#slint_target_sources)
will generate a header file for the root `.slint` file.

This header file will contain a `class` for every exported component from the main file that inherits from `Window` or `Dialog`.

These classes have the same name as the component will have the following public member functions:

* A `create` constructor function and a destructor.
* A `show` function, which will show the component on the screen.
  You still need to spin the event loop by {cpp:func}`slint::run_event_loop()`
  or using the convenience `run` function in this class to render and react to
  user input!
* A `hide` function, which de-registers the component from the windowing system.
* A `window` function that provides access to the {cpp:class}`slint::Window`,
  to allow for further customization towards the windowing system.
* A `run` convenience function, which will show the component and starts the
  event loop.
* For each property:
  * A getter `get_<property_name>` returning the property type.
  * A setter `set_<property_name>` taking the new value of the property by
    const reference
* For each callback:
  * `invoke_<callback_name>` function which takes the callback argument as parameter and call the callback.
  * `on_<callback_name>` function which takes a functor as an argument and sets the callback handler
     for this callback. the functor must accept the type parameter of the callback
* For each public function declared in the root component, an `invoke_<function_name>` function to call the function.
* A `global` function to access exported global singletons.

The `create` function creates a new instance of the component, which is wrapped
in {cpp:class}`slint::ComponentHandle`. This is a smart pointer that owns the
actual instance and keeps it alive as long as at least one
{cpp:class}`slint::ComponentHandle` is in scope, similar to `std::shared_ptr<T>`.

For more complex user interfaces it's common to supply data in the form of an
abstract data model, that's used with {{ '[`for` - `in`]({})'.format(slint_href_Models) }}
repetitions or {{ '[ListView]({})'.format(slint_href_ListView) }} elements in the
`.slint` language. All models in C++ are sub-classes of the
{cpp:class}`slint::Model` and you can sub-class it yourself. For convenience,
the {cpp:class}`slint::VectorModel` provides an implementation that's backed
by a `std::vector<T>`.

## Example

Let's assume we've this code in our `.slint` file:

```slint,no-preview
export component SampleComponent inherits Window {
    in-out property<int> counter;
    // note that dashes will be replaced by underscores in the generated code
    in-out property<string> user_name;
    callback hello;
    public function do-something(x: int) -> bool { return x > 0; }
    // ... maybe more elements here
}

```

This generates a header with the following contents (edited for documentation purpose)

```cpp
#include <array>
#include <limits>
#include <slint.h>


class SampleComponent {
public:
    /// Constructor function
    inline auto create () -> slint::ComponentHandle<MainWindow>;
    /// Destructor
    inline ~SampleComponent ();

    /// Show this component, and runs the event loop
    inline void run () const;

    /// Show the window that renders this component. Call `slint::run_event_loop()`
    /// to continuously render the contents and react to user input.
    inline void show () const;

    /// Hide the window that renders this component.
    inline void hide () const;

    /// Getter for the `counter` property
    inline int get_counter () const;
    /// Setter for the `counter` property
    inline void set_counter (const int &value) const;

    /// Getter for the `user_name` property
    inline slint::SharedString get_user_name () const;
    /// Setter for the `user_name` property
    inline void set_user_name (const slint::SharedString &value) const;

    /// Call this function to call the `hello` callback
    inline void invoke_hello () const;
    /// Sets the callback handler for the `hello` callback.
    template<typename Functor> inline void on_hello (Functor && callback_handler) const;

    /// Call this function to call the `do-something` function.
    inline bool invoke_do_something (int x) const;

    /// Returns a reference to a global singleton that's exported.
    ///
    /// **Note:** Only globals that are exported or re-exported from the main .slint file will
    /// be exposed in the API
    inline template<typename T>
    const T &global() const;

private:
    /// private fields omitted
};
```

## Global Singletons

You can declare <a href="../slint/src/reference/globals.html">globally available singletons</a> in your
`.slint` files. If exported, these singletons are available via the
`global()` getter function on the generated C++ class. Each global singleton
maps to a class with getter/setter functions for properties and callbacks,
similar to API that's created for your `.slint` component.

For example the following `.slint` markup defines a global `Logic` singleton that's also exported:

```slint,ignore
export global Logic {
    callback to_uppercase(string) -> string;
}
```

Assuming this global is used together with the `SampleComponent` from the
previous section, you can access `Logic` like this:

```cpp
    auto app = SampleComponent::create();
    // ...
    app->global<Logic>().on_to_uppercase([](SharedString str) -> SharedString {
        std::string arg(str);
        std::transform(arg.begin(), arg.end(), arg.begin(), toupper);
        return SharedString(arg);
    });
```

:::{note}
Global singletons are instantiated once per component. When declaring multiple components for `export` to C++,
each instance will have their own instance of associated globals singletons.
:::
