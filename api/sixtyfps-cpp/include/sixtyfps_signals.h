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

template<typename... Arg>
struct Signal
{
    Signal() { cbindgen_private::sixtyfps_signal_init(&inner); }
    ~Signal() { cbindgen_private::sixtyfps_signal_drop(&inner); }
    Signal(const Signal &) = delete;
    Signal(Signal &&) = delete;
    Signal &operator=(const Signal &) = delete;

    template<typename F>
    void set_handler(F binding) const
    {
        cbindgen_private::sixtyfps_signal_set_handler(
                &inner,
                [](void *user_data, const void *arg) {
                    std::apply(*reinterpret_cast<F *>(user_data),
                               *reinterpret_cast<const Tuple*>(arg));
                },
                new F(std::move(binding)),
                [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    void emit(const Arg &...arg) const
    {
        Tuple tuple{arg...};
        cbindgen_private::sixtyfps_signal_emit(&inner, &tuple);
    }

private:
    using Tuple = std::tuple<Arg...>;
    cbindgen_private::SignalOpaque inner;
};
}
