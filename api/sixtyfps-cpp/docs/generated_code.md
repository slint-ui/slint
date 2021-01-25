# Generated code

As of now, only the last component of a .60 source is generated. It is planed to generate all
exported components.

The SixtyFPS compiler called by the build system will generate a header file for the root .60
file. This header file will contain a `class` with the same name as the component.

This class will have the following public member functions:

 - A default constructor and a destructor.
 - A `show` function, which will show the component on the screen. Note that in order to render
   and react to user input, it's still necessary to spin the event loop, by calling `sixtyfps::run_event_loop()`
   or using the convenience `fun` function in this class.
 - A `hide` function, which de-registers the component from the windowing system.
 - A `run` convenience function, which will show the component and starts the event loop.
 - for each properties:
    * A getter `get_<property_name>` returning the property type.
    * A setter `set_<property_name>` taking the new value of the property by const reference
 - for each callbacks:
    * `call_<callback_name>` function which takes the callback argument as parameter and call the callback.
    * `on_<callback_name>` functin wich takes a functor as an argument and sets the callback handler
     for this callback. the functor must accept the type parameter of the callback

## Example

Let's assume we have this code in our `.60` file

```60
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
#include <sixtyfps.h>


class SampleComponent {
public:
    /// Contructor
    inline auto create () -> sixtyfps::ComponentHandle<MainWindow>;
    /// Destructor
    inline ~SampleComponent ();

    /// Show this component, and runs the event loop
    inline void run () const;

    /// Getter for the `counter` property
    inline int get_counter () const -> int;
    /// Setter for the `counter` property
    inline void set_counter (const int &value) const;

    /// Getter for the `user_name` property
    inline sixtyfps::SharedString get_user_name () const;
    /// Setter for the `user_name` property
    inline void set_user_name (const sixtyfps::SharedString &value) const;

    /// Call this function to call the `hello` callback
    inline void call_hello () const;
    /// Sets the callback handler for the `hello` callback.
    template<typename Functor> inline void on_hello (Functor && callback_handler) const;

private:
    /// private fields omitted
};




```
