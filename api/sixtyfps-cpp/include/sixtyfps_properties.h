/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once
#include <string_view>
#include <memory>

namespace sixtyfps {
namespace cbindgen_private {
struct PropertyAnimation;
}
}

#include "sixtyfps_properties_internal.h"

namespace sixtyfps {

template<typename T>
struct Property
{
    Property() { cbindgen_private::sixtyfps_property_init(&inner); }
    ~Property() { cbindgen_private::sixtyfps_property_drop(&inner); }
    Property(const Property &) = delete;
    Property(Property &&) = delete;
    Property &operator=(const Property &) = delete;
    explicit Property(const T &value) : value(value)
    {
        cbindgen_private::sixtyfps_property_init(&inner);
    }

    /* Should it be implicit?
    void operator=(const T &value) {
        set(value);
    }*/

    void set(const T &value) const
    {
        if (this->value != value) {
            this->value = value;
            cbindgen_private::sixtyfps_property_set_changed(&inner, &this->value);
        }
    }

    const T &get() const
    {
        cbindgen_private::sixtyfps_property_update(&inner, &value);
        return value;
    }

    template<typename F>
    void set_binding(F binding) const
    {
        cbindgen_private::sixtyfps_property_set_binding(
                &inner,
                [](void *user_data, void *value) {
                    *reinterpret_cast<T *>(value) = (*reinterpret_cast<F *>(user_data))();
                },
                new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
                nullptr, nullptr);
    }

    inline void set_animated_value(const T &value,
                                   const cbindgen_private::PropertyAnimation &animation_data);
    template<typename F>
    inline void set_animated_binding(F binding,
                                     const cbindgen_private::PropertyAnimation &animation_data);

    bool is_dirty() const { return cbindgen_private::sixtyfps_property_is_dirty(&inner); }

    static void link_two_way(Property<T> *p1, Property<T> *p2) {
        auto value = p2->get();
        cbindgen_private::PropertyHandleOpaque handle{};
        if ((p2->inner._0 & 0b10) == 0b10) {
            std::swap(handle,p2->inner);
        }
        auto common_property = std::make_shared<Property<T>>(handle, std::move(value));
        struct TwoWayBinding {
            std::shared_ptr<Property<T>> common_property;
        };
        auto del_fn = [](void *user_data) { delete reinterpret_cast<TwoWayBinding *>(user_data); };
        auto call_fn = [](void *user_data, void *value) {
            *reinterpret_cast<T *>(value) =
                reinterpret_cast<TwoWayBinding *>(user_data)->common_property->get();
        };
        auto intercept_fn = [] (void *user_data, const void *value) {
            reinterpret_cast<TwoWayBinding *>(user_data)->common_property->set(
                *reinterpret_cast<const T *>(value));
            return true;
        };
        auto intercept_binding_fn = [] (void *user_data, void *value) {
            cbindgen_private::sixtyfps_property_set_binding_internal(
                &reinterpret_cast<TwoWayBinding *>(user_data)->common_property->inner,
                value);
            return true;
        };
        cbindgen_private::sixtyfps_property_set_binding(&p1->inner, call_fn,
            new TwoWayBinding{common_property}, del_fn, intercept_fn, intercept_binding_fn);
        cbindgen_private::sixtyfps_property_set_binding(&p2->inner, call_fn,
            new TwoWayBinding{common_property}, del_fn, intercept_fn, intercept_binding_fn);
    }

    /// Internal (private) constructor used by link_two_way
    explicit Property(cbindgen_private::PropertyHandleOpaque inner, T value)
        : inner(inner), value(std::move(value)) {}

private:
    cbindgen_private::PropertyHandleOpaque inner;
    mutable T value {};
};

template<>
void Property<int32_t>::set_animated_value(
        const int32_t &new_value, const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_value_int(&inner, value, new_value,
                                                               &animation_data);
}

template<>
void Property<float>::set_animated_value(const float &new_value,
                                         const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_value_float(&inner, value, new_value,
                                                                 &animation_data);
}

template<>
template<typename F>
void Property<int32_t>::set_animated_binding(
        F binding, const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_binding_int(
            &inner,
            [](void *user_data, int32_t *value) {
                *reinterpret_cast<int32_t *>(value) = (*reinterpret_cast<F *>(user_data))();
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

template<>
template<typename F>
void Property<float>::set_animated_binding(
        F binding, const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_binding_float(
            &inner,
            [](void *user_data, float *value) {
                *reinterpret_cast<float *>(value) = (*reinterpret_cast<F *>(user_data))();
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

struct PropertyTracker
{
    PropertyTracker() { cbindgen_private::sixtyfps_property_tracker_init(&inner); }
    ~PropertyTracker() { cbindgen_private::sixtyfps_property_tracker_drop(&inner); }
    PropertyTracker(const PropertyTracker &) = delete;
    PropertyTracker &operator=(const PropertyTracker &) = delete;

    bool is_dirty() const { return cbindgen_private::sixtyfps_property_tracker_is_dirty(&inner); }

    template<typename F>
    auto evaluate(const F &f) const -> std::enable_if_t<std::is_same_v<decltype(f()), void>>
    {
        cbindgen_private::sixtyfps_property_tracker_evaluate(
                &inner, [](void *f) { (*reinterpret_cast<const F *>(f))(); }, const_cast<F *>(&f));
    }

    template<typename F>
    auto evaluate(const F &f) const
            -> std::enable_if_t<!std::is_same_v<decltype(f()), void>, decltype(f())>
    {
        decltype(f()) result;
        this->evaluate([&] { result = f(); });
        return result;
    }

private:
    cbindgen_private::PropertyTrackerOpaque inner;
};

} // namespace sixtyfps
