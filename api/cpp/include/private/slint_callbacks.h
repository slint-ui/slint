// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <tuple>
#include <utility>
#include "private/slint_properties_internal.h"

namespace slint::private_api {

namespace detail {
// Custom apply implementation to replace std::apply.
// NDK r27's libc++ rejects std::apply with const tuple references in some configurations.
template<typename F, typename Tuple, std::size_t... I>
decltype(auto) apply_impl(F &&f, Tuple &&t, std::index_sequence<I...>)
{
    return std::forward<F>(f)(std::get<I>(std::forward<Tuple>(t))...);
}
template<typename F, typename Tuple>
decltype(auto) apply(F &&f, Tuple &&t)
{
    return apply_impl(
            std::forward<F>(f), std::forward<Tuple>(t),
            std::make_index_sequence<std::tuple_size_v<std::remove_reference_t<Tuple>>> {});
}
} // namespace detail

/// A Callback stores a function pointer with no parameters and no return value.
/// It's possible to set that pointer via set_handler() and it can be invoked via call(). This is
/// used to implement callbacks in the `.slint` language.
template<typename = void()>
struct Callback;
/// A Callback stores a function pointer with \a Arg parameters and a return value of type \a Ret.
/// It's possible to set that pointer via set_handler() and it can be invoked via call(). This is
/// used to implement callbacks in the `.slint` language.
template<typename Ret, typename... Arg>
struct Callback<Ret(Arg...)>
{
    /// Constructs an empty callback that contains no handler.
    Callback() { cbindgen_private::slint_callback_init(&inner); }
    /// Destructs the callback.
    ~Callback() { cbindgen_private::slint_callback_drop(&inner); }
    Callback(const Callback &) = delete;
    Callback(Callback &&) = delete;
    Callback &operator=(const Callback &) = delete;

    /// Sets a new handler \a binding for this callback, that will be invoked when call() is called.
    template<typename F>
    void set_handler(F binding) const
    {
        cbindgen_private::slint_callback_set_handler(
                &inner,
                [](void *user_data, const void *arg, void *ret) {
                    Tuple args = *reinterpret_cast<const Tuple *>(arg);
                    *reinterpret_cast<Ret *>(ret) =
                            detail::apply(*reinterpret_cast<F *>(user_data), std::move(args));
                },
                new F(std::move(binding)),
                [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    /// Invokes a previously set handler with the parameters \a arg and returns the return value of
    /// the handler.
    Ret call(const Arg &...arg) const
    {
        Ret r {};
        Tuple tuple { arg... };
        cbindgen_private::slint_callback_call(&inner, &tuple, &r);
        return r;
    }

private:
    using Tuple = std::tuple<Arg...>;
    cbindgen_private::CallbackOpaque inner;
};

/// A Callback stores a function pointer with \a Arg parameters and no return value.
/// It's possible to set that pointer via set_handler() and it can be invoked via call(). This is
/// used to implement callbacks in the `.slint` language.
template<typename... Arg>
struct Callback<void(Arg...)>
{
    /// Constructs an empty callback that contains no handler.
    Callback() { cbindgen_private::slint_callback_init(&inner); }
    /// Destructs the callback.
    ~Callback() { cbindgen_private::slint_callback_drop(&inner); }
    Callback(const Callback &) = delete;
    Callback(Callback &&) = delete;
    Callback &operator=(const Callback &) = delete;

    /// Sets a new handler \a binding for this callback, that will be invoked when call() is called.
    template<typename F>
    void set_handler(F binding) const
    {
        cbindgen_private::slint_callback_set_handler(
                &inner,
                [](void *user_data, const void *arg, void *) {
                    Tuple args = *reinterpret_cast<const Tuple *>(arg);
                    detail::apply(*reinterpret_cast<F *>(user_data), std::move(args));
                },
                new F(std::move(binding)),
                [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    /// Invokes a previously set handler with the parameters \a arg.
    void call(const Arg &...arg) const
    {
        Tuple tuple { arg... };
        cbindgen_private::slint_callback_call(&inner, &tuple, reinterpret_cast<void *>(0x1));
    }

private:
    using Tuple = std::tuple<Arg...>;
    cbindgen_private::CallbackOpaque inner;
};

template<typename A, typename R>
struct CallbackSignatureHelper
{
    using Result = R(A);
};
template<typename R>
struct CallbackSignatureHelper<void, R>
{
    using Result = R();
};
template<typename A, typename R = void>
using CallbackHelper = Callback<typename CallbackSignatureHelper<A, R>::Result>;

} // namespace slint
