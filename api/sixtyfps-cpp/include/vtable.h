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
