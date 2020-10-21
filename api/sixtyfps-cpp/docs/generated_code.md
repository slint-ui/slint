# Generated code

As of now, only the last component of a .60 source is generated. It is planed to generate all
exported components.

The SixtyFPS compiler called by the build system will generate a header file for the root .60
file. This header file will contain a `class` with the same name as the component.

This class will have the following public member functions:

 - A default constructor and a destructor.
 - A `run` function which will show the component and starts the event loop
 - for each properties:
    * A getter `get_<property_name>` returning the property type.
    * A setter `set_<property_name>` taking the new value of the property by const reference
 - for each signals:
    * `emit_<signal_name>` function which takes the signal argument as parameter and emit the signal.
    * `on_<signal_name>` functin wich takes a functor as an argument and sets the signal handler
     for this signal. the functor must accept the type parameter of the signal

## Example

Let's assume we have this code in our `.60` file

```60
SampleComponent := Window {
    property<int> counter;
    property<string> user_name;
    signal hello;
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
    inline SampleComponent ();
    /// Destructor
    inline ~SampleComponent ();

    /// Show this component, and runs the event loop
    inline void run ();

    /// Getter for the `counter` property
    inline int get_counter () -> int;
    /// Setter for the `counter` property
    inline void set_counter (const int &value);

    /// Getter for the `user_name` property
    inline sixtyfps::SharedString get_user_name ();
    /// Setter for the `user_name` property
    inline void set_user_name (const sixtyfps::SharedString &value);

    /// Call this function to emit the `hello` signal
    inline void emit_hello ();
    /// Sets the signal handler for the `hello` signal.
    template<typename Functor> inline void on_hello (Functor && signal_handler);

private:
    /// private fields omitted
};




```
