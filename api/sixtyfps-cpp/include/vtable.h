/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include <cstddef>
#include <new>

namespace vtable {

template<typename T>
struct VRefMut
{
    const T *vtable;
    void *instance;
};

struct Layout {
    std::size_t size;
    std::align_val_t align;
};

// For the C++'s purpose, they are all the same
template<typename T>
using VRef = VRefMut<T>;
template<typename T>
using VBox = VRefMut<T>;

template<typename T>
using Pin = T;

/*
template<typename T>
struct VBox {
    const T *vtable;
    void *instance;
};

template<typename T>
struct VRef {
    const T *vtable;
    const void *instance;
};
*/

struct AllowPin;

template<typename Base, typename T, typename Flag = void>
struct VOffset
{
    const T *vtable;
    std::uintptr_t offset;
};


}
