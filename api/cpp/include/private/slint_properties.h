// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <functional>
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace slint::cbindgen_private {
struct PropertyAnimation;
struct ChangeTracker
{
    void *inner;
};
}

#include "private/slint_properties_internal.h"
#include "private/slint_builtin_structs_internal.h"

namespace slint::private_api {

using cbindgen_private::StateInfo;

/// `kind` tags for `slint_property_set_binding_with_kind` /
/// `slint_property_binding_kind_user_data`, identifying the C++ object
/// behind a binding's `user_data` so it can be recovered safely later.
/// A `Property<T>::TwoWayBinding` (holds the two-way class' shared common
/// property).
constexpr uint8_t cpp_two_way_binding_kind = 1;
/// A `Property<T>::StructMemberBindings` (the wrapper binding of a struct
/// property whose fields participate in two-way binding classes).
constexpr uint8_t cpp_struct_member_bindings_kind = 2;

inline void slint_property_set_animated_binding_helper(
        const cbindgen_private::PropertyHandleOpaque *handle, void (*binding)(void *, int *),
        void *user_data, void (*drop_user_data)(void *),
        cbindgen_private::PropertyAnimation (*transition_data)(void *, uint64_t **))
{
    cbindgen_private::slint_property_set_animated_binding_int(handle, binding, user_data,
                                                              drop_user_data, transition_data);
}

inline void slint_property_set_animated_binding_helper(
        const cbindgen_private::PropertyHandleOpaque *handle, void (*binding)(void *, float *),
        void *user_data, void (*drop_user_data)(void *),
        cbindgen_private::PropertyAnimation (*transition_data)(void *, uint64_t **))
{
    cbindgen_private::slint_property_set_animated_binding_float(handle, binding, user_data,
                                                                drop_user_data, transition_data);
}

inline void slint_property_set_animated_binding_helper(
        const cbindgen_private::PropertyHandleOpaque *handle, void (*binding)(void *, Color *),
        void *user_data, void (*drop_user_data)(void *),
        cbindgen_private::PropertyAnimation (*transition_data)(void *, uint64_t **))
{
    cbindgen_private::slint_property_set_animated_binding_color(handle, binding, user_data,
                                                                drop_user_data, transition_data);
}

inline void slint_property_set_animated_binding_helper(
        const cbindgen_private::PropertyHandleOpaque *handle, void (*binding)(void *, Brush *),
        void *user_data, void (*drop_user_data)(void *),
        cbindgen_private::PropertyAnimation (*transition_data)(void *, uint64_t **))
{
    cbindgen_private::slint_property_set_animated_binding_brush(handle, binding, user_data,
                                                                drop_user_data, transition_data);
}

template<typename T>
struct Property
{
    Property() { cbindgen_private::slint_property_init(&inner); }
    ~Property() { cbindgen_private::slint_property_drop(&inner); }
    Property(const Property &) = delete;
    Property(Property &&) = delete;
    Property &operator=(const Property &) = delete;
    explicit Property(const T &value) : value(value)
    {
        cbindgen_private::slint_property_init(&inner);
    }

    /* Should it be implicit?
    void operator=(const T &value) {
        set(value);
    }*/

    void set(const T &value) const
    {
        if ((reinterpret_cast<uintptr_t>(inner._0) & 0b10) == 0b10 || this->value != value) {
            this->value = value;
            cbindgen_private::slint_property_set_changed(&inner, &this->value);
        }
    }

    const T &get() const
    {
        cbindgen_private::slint_property_update(&inner, &value);
        return value;
    }

    /// Register this property as a dependency of the current tracking scope
    /// without evaluating any binding.
    void register_as_dependency() const
    {
        cbindgen_private::slint_property_register_as_dependency(&inner);
    }

    template<typename F>
    void set_binding(F binding) const
    {
        cbindgen_private::slint_property_set_binding(
                &inner,
                [](void *user_data, void *value) {
                    *reinterpret_cast<T *>(value) = (*reinterpret_cast<F *>(user_data))();
                },
                new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
                nullptr, nullptr);
    }

    inline void set_animated_value(const T &value,
                                   const cbindgen_private::PropertyAnimation &animation_data) const;

    template<typename F, typename Trans>
    inline void set_animated_binding(F binding, Trans animation) const
    {
        struct UserData
        {
            F binding;
            Trans animation;
        };
        private_api::slint_property_set_animated_binding_helper(
                &inner,
                [](void *user_data, T *value) {
                    *reinterpret_cast<T *>(value) =
                            reinterpret_cast<UserData *>(user_data)->binding();
                },
                new UserData { binding, animation },
                [](void *user_data) { delete reinterpret_cast<UserData *>(user_data); },
                [](void *user_data, uint64_t **instant) {
                    return reinterpret_cast<UserData *>(user_data)->animation(instant);
                });
    }

    bool is_dirty() const { return cbindgen_private::slint_property_is_dirty(&inner); }
    void mark_dirty() const { cbindgen_private::slint_property_mark_dirty(&inner); }

    static void link_two_way(const Property<T> *p1, const Property<T> *p2)
    {
        auto value = p2->get();
        cbindgen_private::PropertyHandleOpaque handle {};
        if ((reinterpret_cast<uintptr_t>(p2->inner._0) & 0b10) == 0b10) {
            std::swap(handle, const_cast<Property<T> *>(p2)->inner);
        }
        auto common_property = std::make_shared<Property<T>>(handle, std::move(value));
        cbindgen_private::slint_property_set_binding_with_kind(
                &p1->inner, TwoWayBinding::call_fn, new TwoWayBinding { common_property },
                TwoWayBinding::del_fn, TwoWayBinding::intercept_fn,
                TwoWayBinding::intercept_binding_fn, cpp_two_way_binding_kind);
        cbindgen_private::slint_property_set_binding_with_kind(
                &p2->inner, TwoWayBinding::call_fn, new TwoWayBinding { common_property },
                TwoWayBinding::del_fn, TwoWayBinding::intercept_fn,
                TwoWayBinding::intercept_binding_fn, cpp_two_way_binding_kind);
    }

    template<typename T2, typename M1, typename M2>
    static void link_two_way_with_map(const Property<T> *prop1, const Property<T2> *prop2, M1 map1,
                                      M2 map2)
    {
        // TODO: neither this nor link_two_way manages to re-use a common_property like the Rust
        // equivalent does.

        auto value = prop1->get();
        cbindgen_private::PropertyHandleOpaque handle {};
        if ((reinterpret_cast<uintptr_t>(prop1->inner._0) & 0b10) == 0b10) {
            std::swap(handle, const_cast<Property<T> *>(prop1)->inner);
        }
        auto common_property = std::make_shared<Property<T>>(handle, std::move(value));

        struct TwoWayBindingWithMap
        {
            std::shared_ptr<Property<T>> common_property;
            M1 map_to;
            M2 map_from;
        };
        auto del_fn = [](void *user_data) {
            delete reinterpret_cast<TwoWayBindingWithMap *>(user_data);
        };
        auto call_fn = [](void *user_data, void *value) {
            auto self = reinterpret_cast<TwoWayBindingWithMap *>(user_data);
            *reinterpret_cast<T2 *>(value) = self->map_to(self->common_property->get());
        };
        auto intercept_fn = [](void *user_data, const void *value) {
            auto self = reinterpret_cast<TwoWayBindingWithMap *>(user_data);
            T old = self->common_property->get();
            self->map_from(old, *reinterpret_cast<const T2 *>(value));
            self->common_property->set(old);
            return true;
        };
        auto intercept_binding_fn = [](void *user_data, void *t2_binding) {
            struct BindingMapper
            {
                void *t2_binding;
                M1 map_to;
                M2 map_from;
                ~BindingMapper() { cbindgen_private::slint_property_delete_binding(t2_binding); }
                const BindingMapper &operator=(const BindingMapper &) = delete;
            };
            auto self = reinterpret_cast<TwoWayBindingWithMap *>(user_data);
            cbindgen_private::slint_property_set_binding(
                    &self->common_property->inner,
                    [](void *user_data, void *value) {
                        auto self = reinterpret_cast<BindingMapper *>(user_data);
                        T &v = *reinterpret_cast<T *>(value);
                        T2 sub_value = self->map_to(v);
                        cbindgen_private::slint_property_evaluate_binding(self->t2_binding,
                                                                          &sub_value);
                        self->map_from(v, sub_value);
                    },
                    new BindingMapper { t2_binding, self->map_to, self->map_from },
                    [](void *user_data) { delete reinterpret_cast<BindingMapper *>(user_data); },
                    [](void *user_data, const void *value) {
                        auto self = reinterpret_cast<BindingMapper *>(user_data);
                        T2 sub_value = self->map_to(*reinterpret_cast<const T *>(value));
                        return cbindgen_private::slint_property_intercept_set_binding(
                                self->t2_binding, &sub_value);
                    },
                    nullptr);
            return true;
        };

        cbindgen_private::slint_property_set_binding_with_kind(
                &prop1->inner, TwoWayBinding::call_fn, new TwoWayBinding { common_property },
                TwoWayBinding::del_fn, TwoWayBinding::intercept_fn,
                TwoWayBinding::intercept_binding_fn, cpp_two_way_binding_kind);

        cbindgen_private::slint_property_set_binding(
                &prop2->inner, call_fn, new TwoWayBindingWithMap { common_property, map1, map2 },
                del_fn, intercept_fn, intercept_binding_fn);
    }

    /// Bind `prop` two-way to a value stored in a model row. `getter` reads
    /// the current row value (returning std::nullopt when the source is no
    /// longer alive, which keeps the previous value); `setter` writes a new
    /// value back into the row.
    template<typename Getter, typename Setter>
    static void link_two_way_to_model_data(const Property<T> *prop, Getter getter, Setter setter)
    {
        struct ModelTwoWayBinding
        {
            Getter getter;
            Setter setter;
        };
        cbindgen_private::slint_property_set_binding(
                &prop->inner,
                [](void *user_data, void *value) {
                    auto self = reinterpret_cast<ModelTwoWayBinding *>(user_data);
                    if (auto v = self->getter())
                        *reinterpret_cast<T *>(value) = *std::move(v);
                },
                new ModelTwoWayBinding { std::move(getter), std::move(setter) },
                [](void *user_data) { delete reinterpret_cast<ModelTwoWayBinding *>(user_data); },
                [](void *user_data, const void *value) {
                    auto self = reinterpret_cast<ModelTwoWayBinding *>(user_data);
                    self->setter(*reinterpret_cast<const T *>(value));
                    return true;
                },
                [](void *, void *) -> bool {
                    // A new binding replaces the model binding (this happens
                    // when a driver projection is installed on a two-way
                    // class' common property that is bound to a model row;
                    // same behavior as the Rust TwoWayBindingModel).
                    return false;
                });
    }

    /// The binding wrapper installed on a struct property that has two-way
    /// bindings onto its *fields*. Mirrors the Rust `StructMemberBindings`
    /// (see internal/core/properties/two_way_binding.rs): each mapped field
    /// is synchronized with a shared narrow common property of the field's
    /// type; the struct's own value-producing binding lives in the
    /// `DriverSlot` and both produces the unmapped fields and drives the
    /// commons via projections.
    struct StructMemberBindings
    {
        struct DriverSlot
        {
            /// The struct property's own binding (an owned Rust
            /// `BindingHolder`), if any.
            void *holder = nullptr;
            /// Seed value for evaluating the binding outside of the struct
            /// property's own storage.
            T cache {};
            /// Number of nested `evaluate_holder` frames; while non-zero,
            /// holders are parked in `pending_drops` instead of deleted (a
            /// projection evaluates the binding without the struct
            /// property's lock, so a write-back could otherwise free the
            /// binding while it is executing).
            int evaluation_depth = 0;
            std::vector<void *> pending_drops;

            void dispose(void *binding)
            {
                if (evaluation_depth > 0) {
                    pending_drops.push_back(binding);
                } else {
                    cbindgen_private::slint_property_delete_binding(binding);
                }
            }
            void clear()
            {
                if (auto *binding = std::exchange(holder, static_cast<void *>(nullptr))) {
                    dispose(binding);
                }
            }
            /// Evaluate the current holder into `value`, keeping it alive
            /// until the evaluation returns even if it is cleared or
            /// replaced while executing. Returns false when the slot is
            /// empty.
            bool evaluate_holder(T *value)
            {
                auto *binding = holder;
                if (!binding) {
                    return false;
                }
                evaluation_depth++;
                cbindgen_private::slint_property_evaluate_binding(binding, value);
                if (--evaluation_depth == 0) {
                    while (!pending_drops.empty()) {
                        auto *pending = pending_drops.back();
                        pending_drops.pop_back();
                        cbindgen_private::slint_property_delete_binding(pending);
                    }
                }
                return true;
            }
            ~DriverSlot()
            {
                clear();
                while (!pending_drops.empty()) {
                    auto *pending = pending_drops.back();
                    pending_drops.pop_back();
                    cbindgen_private::slint_property_delete_binding(pending);
                }
            }
        };

        std::shared_ptr<DriverSlot> slot = std::make_shared<DriverSlot>();

        struct Mapping
        {
            /// Field path within the struct (e.g. "field" or "outer.inner"),
            /// used to find and replace the mapping on re-links.
            std::string key;
            /// `value.<key> = common.get()`
            std::function<void(T &)> apply_from_common;
            /// `common.set(get_field(value))`
            std::function<void(const T &)> push_to_common;
            /// Installs a driver projection for this mapping's field onto
            /// the common.
            std::function<void(const std::shared_ptr<DriverSlot> &)> install_projection;
            /// The narrow common property (a shared_ptr<Property<T2>>),
            /// type-erased for the field-keyed reuse lookup.
            std::shared_ptr<void> common;
        };
        std::vector<Mapping> mappings;
    };

    /// Make sure the property wears a `StructMemberBindings` wrapper, moving
    /// a pre-existing binding into the wrapper's driver slot.
    static StructMemberBindings *ensure_struct_member_bindings(const Property<T> *prop)
    {
        if (void *user_data = cbindgen_private::slint_property_binding_kind_user_data(
                    &prop->inner, cpp_struct_member_bindings_kind)) {
            return reinterpret_cast<StructMemberBindings *>(user_data);
        }
        auto *wrapper = new StructMemberBindings();
        wrapper->slot->cache = prop->value;
        if (void *old = cbindgen_private::slint_property_detach_binding(&prop->inner)) {
            wrapper->slot->holder = old;
        }
        cbindgen_private::slint_property_set_binding_with_kind(
                &prop->inner,
                [](void *user_data, void *value) {
                    auto *self = reinterpret_cast<StructMemberBindings *>(user_data);
                    T &v = *reinterpret_cast<T *>(value);
                    if (self->slot->evaluate_holder(&v)) {
                        self->slot->cache = v;
                    }
                    for (auto &mapping : self->mappings) {
                        mapping.apply_from_common(v);
                    }
                },
                wrapper,
                [](void *user_data) {
                    delete reinterpret_cast<StructMemberBindings *>(user_data);
                },
                [](void *user_data, const void *value) -> bool {
                    // Setting a value drops the binding (as on an unwrapped
                    // property) and pushes the mapped fields into their
                    // classes; the unmapped fields are stored by set().
                    auto *self = reinterpret_cast<StructMemberBindings *>(user_data);
                    self->slot->clear();
                    const T &v = *reinterpret_cast<const T *>(value);
                    for (auto &mapping : self->mappings) {
                        mapping.push_to_common(v);
                    }
                    return true;
                },
                [](void *user_data, void *new_binding) -> bool {
                    // A new binding becomes the driver of the mapped fields'
                    // classes: install the projections (which also marks the
                    // commons' dependents dirty).
                    auto *self = reinterpret_cast<StructMemberBindings *>(user_data);
                    if (auto *old = std::exchange(self->slot->holder, new_binding)) {
                        self->slot->dispose(old);
                    }
                    for (auto &mapping : self->mappings) {
                        mapping.install_projection(self->slot);
                    }
                    return true;
                },
                cpp_struct_member_bindings_kind);
        return wrapper;
    }

    /// The (type-erased) narrow common property the given field of this
    /// struct property is synchronized with, if any.
    static std::shared_ptr<void> struct_member_common(const Property<T> *prop,
                                                      std::string_view field_key)
    {
        if (void *user_data = cbindgen_private::slint_property_binding_kind_user_data(
                    &prop->inner, cpp_struct_member_bindings_kind)) {
            auto *wrapper = reinterpret_cast<StructMemberBindings *>(user_data);
            for (auto &mapping : wrapper->mappings) {
                if (mapping.key == field_key) {
                    return mapping.common;
                }
            }
        }
        return nullptr;
    }

    /// Add (or replace, keyed by `field_key`) a mapping synchronizing
    /// `field_key` of this struct property with `common`.
    template<typename T2, typename GetField, typename SetField>
    static void add_struct_member_mapping(const Property<T> *prop, std::string_view field_key,
                                          std::shared_ptr<Property<T2>> common, GetField get_field,
                                          SetField set_field)
    {
        auto *wrapper = ensure_struct_member_bindings(prop);
        typename StructMemberBindings::Mapping mapping {
            std::string(field_key),
            [common, set_field](T &value) { set_field(value, common->get()); },
            [common, get_field](const T &value) { common->set(get_field(value)); },
            [common, get_field](const std::shared_ptr<typename StructMemberBindings::DriverSlot>
                                        &slot) {
                // The driver projection: evaluates the struct's binding and
                // extracts the mapped field, so the binding drives the class
                // (last installed binding on the common wins).
                struct Projection
                {
                    std::shared_ptr<typename StructMemberBindings::DriverSlot> slot;
                    GetField get_field;
                };
                cbindgen_private::slint_property_set_binding(
                        &common->inner,
                        [](void *user_data, void *value) {
                            auto *self = reinterpret_cast<Projection *>(user_data);
                            T struct_value = self->slot->cache;
                            if (!self->slot->evaluate_holder(&struct_value)) {
                                // the driver was dropped: keep the common's
                                // current value
                                return;
                            }
                            *reinterpret_cast<T2 *>(value) = self->get_field(struct_value);
                            self->slot->cache = std::move(struct_value);
                        },
                        new Projection { slot, get_field },
                        [](void *user_data) { delete reinterpret_cast<Projection *>(user_data); },
                        nullptr, nullptr);
            },
            common,
        };
        // A pre-existing binding must also drive this field's class.
        if (wrapper->slot->holder) {
            mapping.install_projection(wrapper->slot);
        }
        bool replaced = false;
        for (auto &existing : wrapper->mappings) {
            if (existing.key == mapping.key) {
                existing = std::move(mapping);
                replaced = true;
                break;
            }
        }
        if (!replaced) {
            wrapper->mappings.push_back(std::move(mapping));
        }
        // The wrapper must re-evaluate to pick up the new mapping's common
        // (and register the dependency on it).
        cbindgen_private::slint_property_mark_binding_and_dependencies_dirty(&prop->inner);
    }

    /// Link `field_key` of the struct property `struct_prop` two-way with
    /// the property `member_prop`, such that they always have the same
    /// value. This is the runtime counterpart of `member <=> strct.field`;
    /// `struct_prop` is the right-hand side and its current field value
    /// wins. Mirrors the Rust `Property::link_two_way_to_member`.
    template<typename T2, typename GetField, typename SetField>
    static void link_two_way_to_member(const Property<T> *struct_prop,
                                       const Property<T2> *member_prop, std::string_view field_key,
                                       GetField get_field, SetField set_field)
    {
        std::shared_ptr<Property<T2>> common;
        bool member_was_linked = false;
        if (void *user_data = cbindgen_private::slint_property_binding_kind_user_data(
                    &member_prop->inner, cpp_two_way_binding_kind)) {
            common = reinterpret_cast<typename Property<T2>::TwoWayBinding *>(user_data)
                             ->common_property;
            member_was_linked = true;
        }
        if (auto struct_common_erased = struct_member_common(struct_prop, field_key)) {
            auto struct_common = std::static_pointer_cast<Property<T2>>(struct_common_erased);
            if (common && common != struct_common) {
                // both sides are already in (distinct) classes: unify them
                Property<T2>::link_two_way(common.get(), struct_common.get());
            }
            common = struct_common;
        }
        if (!common) {
            // seed a new class with the struct's genuine field value (the
            // right-hand side of `<=>` wins)
            common = std::make_shared<Property<T2>>(get_field(struct_prop->get()));
        } else if ((reinterpret_cast<uintptr_t>(common->inner._0) & 0b10) == 0) {
            // push the struct's field value into a reused class, unless the
            // class is driven by a binding (the binding stays authoritative)
            common->set(get_field(struct_prop->get()));
        }

        add_struct_member_mapping(struct_prop, field_key, common, get_field, set_field);

        if (!member_was_linked) {
            // a pre-existing regular binding on the member is dropped by
            // set_binding (the struct's value wins; bindings install after
            // the links in generated code)
            cbindgen_private::slint_property_set_binding_with_kind(
                    &member_prop->inner, Property<T2>::TwoWayBinding::call_fn,
                    new typename Property<T2>::TwoWayBinding { common },
                    Property<T2>::TwoWayBinding::del_fn, Property<T2>::TwoWayBinding::intercept_fn,
                    Property<T2>::TwoWayBinding::intercept_binding_fn, cpp_two_way_binding_kind);
        }
    }

    /// Link `field_key_a` of the struct property `prop_a` two-way with
    /// `field_key_b` of the struct property `prop_b` (both fields have the
    /// same type `T2`). This is the runtime counterpart of a whole-struct
    /// `<=>` that the compiler decomposed into per-field links; `prop_b` is
    /// the right-hand side and its current field value wins.
    template<typename T2, typename TB, typename GetFieldA, typename SetFieldA, typename GetFieldB,
             typename SetFieldB>
    static void link_two_way_members(const Property<T> *prop_a, std::string_view field_key_a,
                                     GetFieldA get_field_a, SetFieldA set_field_a,
                                     const Property<TB> *prop_b, std::string_view field_key_b,
                                     GetFieldB get_field_b, SetFieldB set_field_b)
    {
        auto common_a = std::static_pointer_cast<Property<T2>>(
                Property<T>::struct_member_common(prop_a, field_key_a));
        auto common_b = std::static_pointer_cast<Property<T2>>(
                Property<TB>::struct_member_common(prop_b, field_key_b));
        std::shared_ptr<Property<T2>> common;
        if (common_a && common_b) {
            if (common_a != common_b) {
                Property<T2>::link_two_way(common_a.get(), common_b.get());
            }
            common = common_b;
        } else if (common_a) {
            common = common_a;
        } else if (common_b) {
            common = common_b;
        }
        if (!common) {
            common = std::make_shared<Property<T2>>(get_field_b(prop_b->get()));
        } else if ((reinterpret_cast<uintptr_t>(common->inner._0) & 0b10) == 0) {
            common->set(get_field_b(prop_b->get()));
        }
        // prop_b's mapping is installed last, so a binding on prop_b wins
        // the driver election over one on prop_a.
        Property<T>::add_struct_member_mapping(prop_a, field_key_a, common, get_field_a,
                                               set_field_a);
        Property<TB>::add_struct_member_mapping(prop_b, field_key_b, common, get_field_b,
                                                set_field_b);
    }

    /// Link `field_key` of the struct property `struct_prop` two-way with a
    /// value stored in a model row. This is the runtime counterpart of a
    /// whole-row `strct <=> model-data` that the compiler decomposed into
    /// per-field links; the model is authoritative (the row binding is
    /// installed on the class' common property and drives it).
    template<typename T2, typename GetField, typename SetField, typename Getter, typename Setter>
    static void link_two_way_member_to_model_data(const Property<T> *struct_prop,
                                                  std::string_view field_key, GetField get_field,
                                                  SetField set_field, Getter getter, Setter setter)
    {
        auto common = std::static_pointer_cast<Property<T2>>(
                struct_member_common(struct_prop, field_key));
        if (!common) {
            common = std::make_shared<Property<T2>>(get_field(struct_prop->get()));
        }
        add_struct_member_mapping(struct_prop, field_key, common, get_field, set_field);
        Property<T2>::link_two_way_to_model_data(common.get(), std::move(getter),
                                                 std::move(setter));
    }

    /// Internal (private) constructor used by link_two_way
    explicit Property(cbindgen_private::PropertyHandleOpaque inner, T value)
        : inner(inner), value(std::move(value))
    {
    }

    const T &get_internal() const { return value; }

    void set_constant() const { cbindgen_private::slint_property_set_constant(&inner); }

private:
    cbindgen_private::PropertyHandleOpaque inner;
    mutable T value {};
    template<typename F>
    friend void set_state_binding(const Property<StateInfo> &property, F binding);

    template<typename T2>
    friend struct Property;

    struct TwoWayBinding
    {
        std::shared_ptr<Property<T>> common_property;

        static void del_fn(void *user_data)
        {
            delete reinterpret_cast<TwoWayBinding *>(user_data);
        };
        static void call_fn(void *user_data, void *value)
        {
            *reinterpret_cast<T *>(value) =
                    reinterpret_cast<TwoWayBinding *>(user_data)->common_property->get();
        };
        static bool intercept_fn(void *user_data, const void *value)
        {
            reinterpret_cast<TwoWayBinding *>(user_data)->common_property->set(
                    *reinterpret_cast<const T *>(value));
            return true;
        };
        static bool intercept_binding_fn(void *user_data, void *value)
        {
            cbindgen_private::slint_property_set_binding_internal(
                    &reinterpret_cast<TwoWayBinding *>(user_data)->common_property->inner, value);
            return true;
        };
    };
};

template<>
inline void Property<int32_t>::set_animated_value(
        const int32_t &new_value, const cbindgen_private::PropertyAnimation &animation_data) const
{
    cbindgen_private::slint_property_set_animated_value_int(&inner, value, new_value,
                                                            &animation_data);
}

template<>
inline void
Property<float>::set_animated_value(const float &new_value,
                                    const cbindgen_private::PropertyAnimation &animation_data) const
{
    cbindgen_private::slint_property_set_animated_value_float(&inner, value, new_value,
                                                              &animation_data);
}

template<>
inline void
Property<Color>::set_animated_value(const Color &new_value,
                                    const cbindgen_private::PropertyAnimation &animation_data) const
{
    cbindgen_private::slint_property_set_animated_value_color(&inner, value, new_value,
                                                              &animation_data);
}

template<typename F>
void set_state_binding(const Property<StateInfo> &property, F binding)
{
    cbindgen_private::slint_property_set_state_binding(
            &property.inner,
            [](void *user_data) -> int32_t { return (*reinterpret_cast<F *>(user_data))(); },
            new F(binding), [](void *user_data) { delete reinterpret_cast<F *>(user_data); });
}

/// PropertyTracker allows keeping track of when properties change and lazily evaluate code
/// if necessary.
/// Once constructed, you can call evaluate() with a functor that will be invoked. Any
/// Property<T> types that have their value read from within the invoked functor or any code that's
/// reached from there are added to internal book-keeping. When after returning from evaluate(),
/// any of these accessed properties change their value, the property tracker's is_dirt() function
/// will return true.
///
/// PropertyTracker instances nest, so if during the evaluation of one tracker, another tracker's
/// evaluate() function gets called and properties from within that evaluation change their value
/// later, both tracker instances will report true for is_dirty(). If you would like to disable the
/// nesting, use the evaluate_as_dependency_root() function instead.
struct PropertyTracker
{
    /// Constructs a new property tracker instance.
    PropertyTracker() { cbindgen_private::slint_property_tracker_init(&inner); }
    /// Destroys the property tracker.
    ~PropertyTracker() { cbindgen_private::slint_property_tracker_drop(&inner); }
    /// The copy constructor is intentionally deleted, property trackers cannot be copied.
    PropertyTracker(const PropertyTracker &) = delete;
    /// The assignment operator is intentionally deleted, property trackers cannot be copied.
    PropertyTracker &operator=(const PropertyTracker &) = delete;

    /// Returns true if any properties accessed during the last evaluate() call have changed their
    /// value since then.
    bool is_dirty() const { return cbindgen_private::slint_property_tracker_is_dirty(&inner); }

    /// Invokes the provided functor \a f and tracks accessed to any properties during that
    /// invocation.
    template<typename F>
    auto evaluate(const F &f) const -> std::enable_if_t<std::is_same_v<decltype(f()), void>>
    {
        cbindgen_private::slint_property_tracker_evaluate(
                &inner, [](void *f) { (*reinterpret_cast<const F *>(f))(); }, const_cast<F *>(&f));
    }

    /// Invokes the provided functor \a f and tracks accessed to any properties during that
    /// invocation. Use this overload if your functor returns a value, as evaluate() will pass it on
    /// and return it.
    template<typename F>
    auto evaluate(const F &f) const
            -> std::enable_if_t<!std::is_same_v<decltype(f()), void>, decltype(f())>
    {
        decltype(f()) result;
        this->evaluate([&] { result = f(); });
        return result;
    }

    /// Invokes the provided functor \a f and tracks accessed to any properties during that
    /// invocation.
    ///
    /// This starts a new dependency chain and if called during the evaluation of another
    /// property tracker, the outer tracker will not be notified if any accessed properties change.
    template<typename F>
    auto evaluate_as_dependency_root(const F &f) const
            -> std::enable_if_t<std::is_same_v<decltype(f()), void>>
    {
        cbindgen_private::slint_property_tracker_evaluate_as_dependency_root(
                &inner, [](void *f) { (*reinterpret_cast<const F *>(f))(); }, const_cast<F *>(&f));
    }

    /// Invokes the provided functor \a f and tracks accessed to any properties during that
    /// invocation. Use this overload if your functor returns a value, as evaluate() will pass it on
    /// and return it.
    ///
    /// This starts a new dependency chain and if called during the evaluation of another
    /// property tracker, the outer tracker will not be notified if any accessed properties change.
    template<typename F>
    auto evaluate_as_dependency_root(const F &f) const
            -> std::enable_if_t<!std::is_same_v<decltype(f()), void>, decltype(f())>
    {
        decltype(f()) result;
        this->evaluate_as_dependency_root([&] { result = f(); });
        return result;
    }

private:
    cbindgen_private::PropertyTrackerOpaque inner;
};

struct ChangeTracker
{
    ChangeTracker() { cbindgen_private::slint_change_tracker_construct(&inner); }
    ~ChangeTracker() { cbindgen_private::slint_change_tracker_drop(&inner); }
    ChangeTracker(const ChangeTracker &) = delete;
    ChangeTracker &operator=(const ChangeTracker &) = delete;

    template<typename Data, typename FnEval, typename FnNotify>
    void init(Data data, FnEval fn_eval, FnNotify fn_notify)
    {
        using Value = std::invoke_result_t<FnEval, Data>;
        struct Inner
        {
            Data data;
            FnEval fn_eval;
            FnNotify fn_notify;
            Value value;
        };
        auto data_ptr =
                new Inner { std::move(data), std::move(fn_eval), std::move(fn_notify), Value() };
        cbindgen_private::slint_change_tracker_init(
                &inner, data_ptr, [](void *d) { delete reinterpret_cast<Inner *>(d); },
                [](void *d) {
                    auto inner = reinterpret_cast<Inner *>(d);
                    auto v = inner->fn_eval(inner->data);
                    bool r = v != inner->value;
                    inner->value = v;
                    return r;
                },
                [](void *d) {
                    auto inner = reinterpret_cast<Inner *>(d);
                    inner->fn_notify(inner->data, inner->value);
                });
    }

private:
    cbindgen_private::ChangeTracker inner;
};

} // namespace slint::private_api
