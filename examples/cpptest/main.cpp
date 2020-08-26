/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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

    component.run();
}
