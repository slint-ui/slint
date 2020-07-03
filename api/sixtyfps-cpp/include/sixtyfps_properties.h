#pragma once
#include <string_view>

namespace sixtyfps {
namespace internal {
struct PropertyAnimation;
}
}

#include "sixtyfps_properties_internal.h"

namespace sixtyfps {

template<typename T>
struct Property
{
    Property() { internal::sixtyfps_property_init(&inner); }
    ~Property() { internal::sixtyfps_property_drop(&inner); }
    Property(const Property &) = delete;
    Property(Property &&) = delete;
    Property &operator=(const Property &) = delete;

    /* Should it be implicit?
    void operator=(const T &value) {
        set(value);
    }*/

    void set(const T &value) const
    {
        this->value = value;
        internal::sixtyfps_property_set_changed(&inner);
    }

    const T &get(const internal::EvaluationContext *context) const
    {
        internal::sixtyfps_property_update(&inner, context, &value);
        return value;
    }

    template<typename F>
    void set_binding(F binding) const
    {
        internal::sixtyfps_property_set_binding(
                &inner,
                [](void *user_data, const internal::EvaluationContext *context, void *value) {
                    *reinterpret_cast<T *>(value) = (*reinterpret_cast<F *>(user_data))(context);
                },
                new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
    }

    inline void set_animated_value(const T &value,
                                   const internal::PropertyAnimation &animation_data);
    template<typename F>
    inline void set_animated_binding(F binding, const internal::PropertyAnimation &animation_data);

private:
    internal::PropertyHandleOpaque inner;
    mutable T value{};
};

template<>
void Property<int32_t>::set_animated_value(const int32_t &new_value,
                                           const internal::PropertyAnimation &animation_data)
{
    internal::sixtyfps_property_set_animated_value_int(&inner, value, new_value, &animation_data);
}

template<>
void Property<float>::set_animated_value(const float &new_value,
                                         const internal::PropertyAnimation &animation_data)
{
    internal::sixtyfps_property_set_animated_value_float(&inner, value, new_value, &animation_data);
}

template<>
template<typename F>
void Property<int32_t>::set_animated_binding(F binding,
                                             const internal::PropertyAnimation &animation_data)
{
    internal::sixtyfps_property_set_animated_binding_int(
            &inner,
            [](void *user_data, const internal::EvaluationContext *context, int32_t *value) {
                *reinterpret_cast<int32_t *>(value) = (*reinterpret_cast<F *>(user_data))(context);
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

template<>
template<typename F>
void Property<float>::set_animated_binding(F binding,
                                           const internal::PropertyAnimation &animation_data)
{
    internal::sixtyfps_property_set_animated_binding_float(
            &inner,
            [](void *user_data, const internal::EvaluationContext *context, float *value) {
                *reinterpret_cast<float *>(value) = (*reinterpret_cast<F *>(user_data))(context);
            },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
            &animation_data);
}

struct PropertyListenerScope
{
    PropertyListenerScope() { internal::sixtyfps_property_listener_scope_init(&inner); }
    ~PropertyListenerScope() { internal::sixtyfps_property_listener_scope_drop(&inner); }
    PropertyListenerScope(const PropertyListenerScope &) = delete;
    PropertyListenerScope &operator=(const PropertyListenerScope &) = delete;

    bool is_dirty() const {
        return internal::sixtyfps_property_listener_scope_is_dirty(&inner);
    }

    template<typename F>
    void evaluate(const F &f) const {
        internal::sixtyfps_property_listener_scope_evaluate(
            &inner,
            [](void *f){ (*reinterpret_cast<const F*>(f))(); },
            const_cast<F*>(&f)
        );
    }

private:
    internal::PropertyListenerOpaque inner;
};

} // namespace sixtyfps
