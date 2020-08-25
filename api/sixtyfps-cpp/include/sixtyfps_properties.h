/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
#pragma once
#include <string_view>

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
    explicit Property(const T &value) : value(value) {
        cbindgen_private::sixtyfps_property_init(&inner);
    }

    /* Should it be implicit?
    void operator=(const T &value) {
        set(value);
    }*/

    void set(const T &value) const
    {
        this->value = value;
        cbindgen_private::sixtyfps_property_set_changed(&inner);
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
                new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    inline void set_animated_value(const T &value,
                                   const cbindgen_private::PropertyAnimation &animation_data);
    template<typename F>
    inline void set_animated_binding(F binding, const cbindgen_private::PropertyAnimation &animation_data);

private:
    cbindgen_private::PropertyHandleOpaque inner;
    mutable T value{};
};

template<>
void Property<int32_t>::set_animated_value(const int32_t &new_value,
                                           const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_value_int(&inner, value, new_value, &animation_data);
}

template<>
void Property<float>::set_animated_value(const float &new_value,
                                         const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_value_float(&inner, value, new_value, &animation_data);
}

template<>
template<typename F>
void Property<int32_t>::set_animated_binding(F binding,
                                             const cbindgen_private::PropertyAnimation &animation_data)
{
    cbindgen_private::sixtyfps_property_set_animated_binding_int(
            &inner,
            [](void *user_data,  int32_t *value) {
                *reinterpret_cast<int32_t *>(value) = (*reinterpret_cast<F *>(user_data))();
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

template<>
template<typename F>
void Property<float>::set_animated_binding(F binding,
                                           const cbindgen_private::PropertyAnimation &animation_data)
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

    bool is_dirty() const {
        return cbindgen_private::sixtyfps_property_tracker_is_dirty(&inner);
    }

    template<typename F>
    void evaluate(const F &f) const {
        cbindgen_private::sixtyfps_property_tracker_evaluate(
            &inner,
            [](void *f){ (*reinterpret_cast<const F*>(f))(); },
            const_cast<F*>(&f)
        );
    }

private:
    cbindgen_private::PropertyTrackerOpaque inner;
};

} // namespace sixtyfps
