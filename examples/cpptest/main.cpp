#include "hello.h"
#include <iostream>

int main() {
    static Hello component;

    component.foobar.set_handler([](auto...){
        std::cout << "Hello from C++" << std::endl;
    });

    component.plus_clicked.set_handler([](auto...){
        auto &counter = component.counter;
        counter.set(counter.get() + 1);
        // FIXME: this _13 is an internal detail and should be private anyway.  We muse use some
        // alias or way to expose the property  (same for the _ before signals)
        component.counter_label_11.text.set(std::string_view(std::to_string(counter.get())));
        std::cout << "PLUS: " << std::string_view(component.counter_label_11.text.get()) << std::endl;
    });

    component.minus_clicked.set_handler([](auto...){
        auto &counter = component.counter;
        counter.set(counter.get() - 1);
        component.counter_label_11.text.set(std::string_view(std::to_string(counter.get())));
        std::cout << "MINUS: " << std::string_view(component.counter_label_11.text.get()) << std::endl;
    });

    sixtyfps::run(&component);

}
