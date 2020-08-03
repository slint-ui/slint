#include "hello.h"
#include <iostream>

int main()
{
    static Hello component;

    component.on_foobar([](auto...) { std::cout << "Hello from C++" << std::endl; });

    component.on_plus_clicked([]() {
        component.set_counter(component.get_counter() + 1);
    });

    component.on_minus_clicked([]() {
        component.set_counter(component.get_counter() - 1);
    });

    sixtyfps::ComponentWindow window;
    window.run(&component);
}
