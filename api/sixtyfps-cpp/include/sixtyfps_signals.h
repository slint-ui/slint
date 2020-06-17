#pragma once
#include <string_view>
#include "sixtyfps_properties_internal.h"

namespace sixtyfps {

// template<typename... Args>
struct Signal
{
    Signal() { internal::sixtyfps_signal_init(&inner); }
    ~Signal() { internal::sixtyfps_signal_drop(&inner); }
    Signal(const Signal &) = delete;
    Signal(Signal &&) = delete;
    Signal &operator=(const Signal &) = delete;

    template<typename F>
    void set_handler(F binding)
    {
        internal::sixtyfps_signal_set_handler(
                &inner,
                [](void *user_data, const internal::EvaluationContext *value) {
                    (*reinterpret_cast<F *>(user_data))(value);
                },
                new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    void emit(const internal::EvaluationContext *context) const
    {
        internal::sixtyfps_signal_emit(&inner, context);
    }

private:
    internal::SignalOpaque inner;
};
}
