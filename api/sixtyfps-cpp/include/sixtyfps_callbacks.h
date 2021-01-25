/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once
#include <tuple>
#include "sixtyfps_properties_internal.h"

namespace sixtyfps {

template<typename = void()> struct Callback;
template<typename Ret, typename... Arg>
struct Callback<Ret(Arg...)>
{
    Callback() { cbindgen_private::sixtyfps_callback_init(&inner); }
    ~Callback() { cbindgen_private::sixtyfps_callback_drop(&inner); }
    Callback(const Callback &) = delete;
    Callback(Callback &&) = delete;
    Callback &operator=(const Callback &) = delete;

    template<typename F>
    void set_handler(F binding) const
    {
        cbindgen_private::sixtyfps_callback_set_handler(
                &inner,
                [](void *user_data, const void *arg) {
                    auto *p = reinterpret_cast<const Pair*>(arg);
                    *p->first = std::apply(*reinterpret_cast<F *>(user_data), p->second);
                },
                new F(std::move(binding)),
                [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    Ret call(const Arg &...arg) const
    {
        Ret r{};
        Pair p = std::pair{ &r, Tuple{arg...} };
        cbindgen_private::sixtyfps_callback_call(&inner, &p);
        return r;
    }

private:
    using Tuple = std::tuple<Arg...>;
    using Pair = std::pair<Ret *, Tuple>;
    cbindgen_private::CallbackOpaque inner;
};


template<typename... Arg>
struct Callback<void(Arg...)>
{
    Callback() { cbindgen_private::sixtyfps_callback_init(&inner); }
    ~Callback() { cbindgen_private::sixtyfps_callback_drop(&inner); }
    Callback(const Callback &) = delete;
    Callback(Callback &&) = delete;
    Callback &operator=(const Callback &) = delete;

    template<typename F>
    void set_handler(F binding) const
    {
        cbindgen_private::sixtyfps_callback_set_handler(
                &inner,
                [](void *user_data, const void *arg) {
                    std::apply(*reinterpret_cast<F *>(user_data),
                               *reinterpret_cast<const Tuple*>(arg));
                },
                new F(std::move(binding)),
                [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    void call(const Arg &...arg) const
    {
        Tuple tuple{arg...};
        cbindgen_private::sixtyfps_callback_call(&inner, &tuple);
    }

private:
    using Tuple = std::tuple<Arg...>;
    cbindgen_private::CallbackOpaque inner;
};


}


