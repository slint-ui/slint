#include "hello.h"
#include <iostream>

int main() {
    static Hello component;

    static int counter = 0;

    component._foobar.set_handler([](auto...){
        std::cout << "Hello from C++" << std::endl;
    });

    component._plus_clicked.set_handler([](auto...){
        counter += 1;
        // FIXME: this _13 is an internal detail and should be private anyway.  We muse use some
        // alias or way to expose the property  (same for the _ before signals)
        component.counter_13.text.set(std::string_view(std::to_string(counter)));
        std::cout << "PLUS: " << std::string_view(component.counter_13.text.get()) << std::endl;
    });

    component._minus_clicked.set_handler([](auto...){
        counter -= 1;
        component.counter_13.text.set(std::string_view(std::to_string(counter)));
        std::cout << "MINUS: " << std::string_view(component.counter_13.text.get()) << std::endl;
    });

    sixtyfps::run(&component);

}
