#include "hello.h"
#include <iostream>

int main() {
    static Hello component;

    component._foobar.set_handler([](auto...){
        std::cout << "Hello from C++" << std::endl;
    });

    sixtyfps::run(&component);

}
