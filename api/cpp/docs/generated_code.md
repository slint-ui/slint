# Generated code

The Slint compiler called by the build system will generate a header file for the root `.slint`
file. This header file will contain a `class` with the same name as the component.

This class will have the following public member functions:

* A `create` constructor function and a destructor.
* A `show` function, which will show the component on the screen. Note that in order to render
  and react to user input, it's still necessary to spin the event loop, by calling {cpp:func}`slint::run_event_loop()`
  or using the convenience `fun` function in this class.
* A `hide` function, which de-registers the component from the windowing system.
* A `window` function that provides access to the {cpp:class}`slint::Window`, allow for further customization
  towards the windowing system.
* A `run` convenience function, which will show the component and starts the event loop.
* for each properties:
  * A getter `get_<property_name>` returning the property type.
  * A setter `set_<property_name>` taking the new value of the property by const reference
* for each callbacks:
  * `invoke_<callback_name>` function which takes the callback argument as parameter and call the callback.
  * `on_<callback_name>` function which takes a functor as an argument and sets the callback handler
     for this callback. the functor must accept the type parameter of the callback
* A `global` function, to provide access to any exported global singletons.

The class is instantiated with the `create` function, which returns the type wrapped in {cpp:class}`slint::ComponentHandle`.
This is a smart pointer that owns the actual instance and keeps it alive as long as at least one {cpp:class}`slint::ComponentHandle`
is in scope, similar to `std::shared_ptr<T>`.

For more complex UIs it is common to supply data in the form of an abstract data model, that is used with
[`for` - `in`](markdown/langref.md#repetition) repetitions or [`ListView`](markdown/widgets.md#listview) elements in the `.slint` language.
All models in C++ are sub-classes of the {cpp:class}`slint::Model` and you can sub-class it yourself. For convenience,
the {cpp:class}`slint::VectorModel` provides an implementation that is backed by a `std::vector<T>`.

## Example

Let's assume we have this code in our `.slint` file

```slint,no-preview
SampleComponent := Window {
    property<int> counter;
    property<string> user_name;
    callback hello;
    // ... maybe more elements here
}
```

This will generate a header with the following contents (edited for documentation purpose)

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

In `.slint` files it is possible to declare [singletons that are globally available](markdown/langref.md#global-singletons).
You can access them from to your C++ code by exporting them and using the `global()` getter function in the
C++ class generated for your entry component. Each global singleton creates a class that has getter/setter functions
for properties and callbacks, similar to API that's created for your `.slint` component, as demonstrated in the previous section.

For example the following `.slint` markup defines a global `Logic` singleton that's also exported:

```slint,ignore
export global Logic := {
    callback to_uppercase(string) -> string;
}
```

If this were used together with the `SampleComponent` from the previous section, then you can access it
like this:

```cpp
    auto app = SampleComponent::create();
    // ...
    app->global<Logic>().on_to_uppercase([](SharedString str) -> SharedString {
        std::string arg(str);
        std::transform(arg.begin(), arg.end(), arg.begin(), toupper);
        return SharedString(arg);
    });
```
