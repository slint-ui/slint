/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
#pragma once

template<typename T>
struct VRefMut
{
    const T *vtable;
    void *instance;
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
    uintptr_t offset;
};
