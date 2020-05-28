#include "hello.h"
#include <iostream>

int main()
{
    static Hello component;

    component.foobar.set_handler([](auto...) { std::cout << "Hello from C++" << std::endl; });

    component.plus_clicked.set_handler([](auto ctx) {
        auto &counter = component.counter;
        counter.set(counter.get(ctx) + 1);
    });

    component.minus_clicked.set_handler([](auto ctx) {
        auto &counter = component.counter;
        counter.set(counter.get(ctx) - 1);
    });

    sixtyfps::run(&component);
}
