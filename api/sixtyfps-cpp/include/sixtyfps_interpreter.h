/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include "sixtyfps.h"

#include "sixtyfps_interpreter_internal.h"

#include <optional>

namespace sixtyfps::interpreter {

class Value;

struct Struct
{
public:
    Struct() { cbindgen_private::sixtyfps_interpreter_struct_new(&inner); }

    Struct(const Struct &other)
    {
        cbindgen_private::sixtyfps_interpreter_struct_clone(&other.inner, &inner);
    }
    Struct(Struct &&other)
    {
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_struct_new(&other.inner);
    }
    Struct &operator=(const Struct &other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_struct_destructor(&inner);
        sixtyfps_interpreter_struct_clone(&other.inner, &inner);
        return *this;
    }
    Struct &operator=(Struct &&other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_struct_destructor(&inner);
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_struct_new(&other.inner);
        return *this;
    }
    ~Struct() { cbindgen_private::sixtyfps_interpreter_struct_destructor(&inner); }

    inline Struct(std::initializer_list<std::pair<std::string_view, Value>> args);

    template<typename InputIterator,
             typename std::enable_if_t<
                     std::is_convertible<decltype(std::get<0>(*std::declval<InputIterator>())),
                                         std::string_view>::value
                     && std::is_convertible<decltype(std::get<1>(*std::declval<InputIterator>())),
                                            Value>::value

                     > * = nullptr>
    Struct(InputIterator it, InputIterator end) : Struct()
    {
        for (; it != end; ++it) {
            auto [key, value] = *it;
            set_field(key, value);
        }
    }

    // FIXME: this probably miss a lot of iterator api
    struct iterator
    {
        using value_type = std::pair<std::string_view, const Value &>;

    private:
        cbindgen_private::StructIteratorOpaque inner;
        const Value *v = nullptr;
        std::string_view k;
        friend Struct;
        explicit iterator(cbindgen_private::StructIteratorOpaque inner) : inner(inner) { next(); }
        // construct a end iterator
        iterator() = default;
        void next()
        {
            auto next = cbindgen_private::sixtyfps_interpreter_struct_iterator_next(&inner);
            v = reinterpret_cast<const Value *>(next.v);
            k = std::string_view(reinterpret_cast<char *>(next.k.ptr), next.k.len);
            if (!v) {
                cbindgen_private::sixtyfps_interpreter_struct_iterator_destructor(&inner);
            }
        }

    public:
        ~iterator()
        {
            if (v) {
                cbindgen_private::sixtyfps_interpreter_struct_iterator_destructor(&inner);
            }
        }
        // FIXME i believe iterator are supposed to be copy constructible
        iterator(const iterator &) = delete;
        iterator &operator=(const iterator &) = delete;
        iterator(iterator &&) = default;
        iterator &operator=(iterator &&) = default;
        iterator &operator++()
        {
            if (v)
                next();
            return *this;
        }
        value_type operator*() const { return { k, *v }; }
        friend bool operator==(const iterator &a, const iterator &b) { return a.v == b.v; }
        friend bool operator!=(const iterator &a, const iterator &b) { return a.v != b.v; }
    };

    iterator begin() const
    {
        return iterator(cbindgen_private::sixtyfps_interpreter_struct_make_iter(&inner));
    }
    iterator end() const { return iterator(); }

    inline std::optional<Value> get_field(std::string_view name) const;
    inline void set_field(std::string_view name, const Value &value);

    // internal
    Struct(const sixtyfps::cbindgen_private::StructOpaque &other)
    {
        cbindgen_private::sixtyfps_interpreter_struct_clone(&other, &inner);
    }

private:
    using StructOpaque = sixtyfps::cbindgen_private::StructOpaque;
    StructOpaque inner;
    friend class Value;
};
class Value
{
public:
    Value() { cbindgen_private::sixtyfps_interpreter_value_new(&inner); }

    Value(const Value &other) { sixtyfps_interpreter_value_clone(&other.inner, &inner); }
    Value(Value &&other)
    {
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&other.inner);
    }
    Value &operator=(const Value &other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_value_destructor(&inner);
        sixtyfps_interpreter_value_clone(&other.inner, &inner);
        return *this;
    }
    Value &operator=(Value &&other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_value_destructor(&inner);
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&other.inner);
        return *this;
    }
    ~Value() { cbindgen_private::sixtyfps_interpreter_value_destructor(&inner); }

    using Type = cbindgen_private::ValueType;

    // optional<int> to_int() const;
    // optional<float> to_float() const;
    std::optional<double> to_number() const
    {
        if (auto *number = cbindgen_private::sixtyfps_interpreter_value_to_number(&inner)) {
            return *number;
        } else {
            return {};
        }
    }
    std::optional<sixtyfps::SharedString> to_string() const
    {
        if (auto *str = cbindgen_private::sixtyfps_interpreter_value_to_string(&inner)) {
            return *str;
        } else {
            return {};
        }
    }
    std::optional<bool> to_bool() const
    {
        if (auto *b = cbindgen_private::sixtyfps_interpreter_value_to_bool(&inner)) {
            return *b;
        } else {
            return {};
        }
    }
    inline std::optional<sixtyfps::SharedVector<Value>> to_array() const;
    std::optional<std::shared_ptr<sixtyfps::Model<Value>>> to_model() const;
    std::optional<sixtyfps::Brush> to_brush() const
    {
        if (auto *brush = cbindgen_private::sixtyfps_interpreter_value_to_brush(&inner)) {
            return *brush;
        } else {
            return {};
        }
    }
    std::optional<Struct> to_struct() const
    {
        if (auto *opaque_struct = cbindgen_private::sixtyfps_interpreter_value_to_struct(&inner)) {
            return Struct(*opaque_struct);
        } else {
            return {};
        }
    }

    // template<typename T> std::optional<T> get() const;
    Value(double value) { cbindgen_private::sixtyfps_interpreter_value_new_double(value, &inner); }
    Value(const SharedString &str)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_string(&str, &inner);
    }
    Value(bool b) { cbindgen_private::sixtyfps_interpreter_value_new_bool(b, &inner); }
    inline Value(const SharedVector<Value> &);
    Value(const std::shared_ptr<sixtyfps::Model<Value>> &);
    Value(const sixtyfps::Brush &brush)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_brush(&brush, &inner);
    }
    Value(const Struct &struc)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_struct(&struc.inner, &inner);
    }

    Type type() const { return cbindgen_private::sixtyfps_interpreter_value_type(&inner); }

    friend bool operator==(const Value &a, const Value &b)
    {
        return cbindgen_private::sixtyfps_interpreter_value_eq(&a.inner, &b.inner);
    }
    friend bool operator!=(const Value &a, const Value &b)
    {
        return !cbindgen_private::sixtyfps_interpreter_value_eq(&a.inner, &b.inner);
    }

private:
    using ValueOpaque = sixtyfps::cbindgen_private::ValueOpaque;
    ValueOpaque inner;
    friend struct Struct;
    friend class ComponentInstance;
    // Internal constructor that takes ownership of the value
    explicit Value(ValueOpaque &inner) : inner(inner) { }
};

inline Value::Value(const sixtyfps::SharedVector<Value> &array)
{
    cbindgen_private::sixtyfps_interpreter_value_new_array(
            &reinterpret_cast<const sixtyfps::SharedVector<ValueOpaque> &>(array), &inner);
}

inline std::optional<sixtyfps::SharedVector<Value>> Value::to_array() const
{
    if (auto *array = cbindgen_private::sixtyfps_interpreter_value_to_array(&inner)) {
        return *reinterpret_cast<const sixtyfps::SharedVector<Value> *>(array);
    } else {
        return {};
    }
}
inline Value::Value(const std::shared_ptr<sixtyfps::Model<Value>> &model)
{
    using cbindgen_private::ModelAdaptorVTable;
    using vtable::VRef;
    struct ModelWrapper : AbstractRepeaterView
    {
        std::shared_ptr<sixtyfps::Model<Value>> model;
        cbindgen_private::ModelNotifyOpaque notify;
        // This kind of mean that the rust code has ownership of "this" until the drop funciton is
        // called
        std::shared_ptr<AbstractRepeaterView> self;
        ~ModelWrapper() { cbindgen_private::sixtyfps_interpreter_model_notify_destructor(&notify); }

        void row_added(int index, int count) override
        {
            cbindgen_private::sixtyfps_interpreter_model_notify_row_added(&notify, index, count);
        }
        void row_changed(int index) override
        {
            cbindgen_private::sixtyfps_interpreter_model_notify_row_changed(&notify, index);
        }
        void row_removed(int index, int count) override
        {
            cbindgen_private::sixtyfps_interpreter_model_notify_row_removed(&notify, index, count);
        }
    };

    auto wrapper = std::make_shared<ModelWrapper>();
    wrapper->model = model;
    wrapper->self = wrapper;
    cbindgen_private::sixtyfps_interpreter_model_notify_new(&wrapper->notify);
    model->attach_peer(wrapper);

    auto row_count = [](VRef<ModelAdaptorVTable> self) -> uintptr_t {
        return reinterpret_cast<ModelWrapper *>(self.instance)->model->row_count();
    };
    auto row_data = [](VRef<ModelAdaptorVTable> self, uintptr_t row, ValueOpaque *out) {
        Value v = reinterpret_cast<ModelWrapper *>(self.instance)->model->row_data(row);
        *out = v.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&v.inner);
    };
    auto set_row_data = [](VRef<ModelAdaptorVTable> self, uintptr_t row, const ValueOpaque *value) {
        Value v = *reinterpret_cast<const Value *>(value);
        reinterpret_cast<ModelWrapper *>(self.instance)->model->set_row_data(row, v);
    };
    auto get_notify =
            [](VRef<ModelAdaptorVTable> self) -> const cbindgen_private::ModelNotifyOpaque * {
        return &reinterpret_cast<ModelWrapper *>(self.instance)->notify;
    };
    auto drop = [](vtable::VRefMut<ModelAdaptorVTable> self) {
        reinterpret_cast<ModelWrapper *>(self.instance)->self = nullptr;
    };

    static const ModelAdaptorVTable vt { row_count, row_data, set_row_data, get_notify, drop };
    vtable::VBox<ModelAdaptorVTable> wrap { &vt, wrapper.get() };
    cbindgen_private::sixtyfps_interpreter_value_new_model(wrap, &inner);
}

inline Struct::Struct(std::initializer_list<std::pair<std::string_view, Value>> args)
    : Struct(args.begin(), args.end())
{
}

inline std::optional<Value> Struct::get_field(std::string_view name) const
{
    cbindgen_private::Slice<uint8_t> name_view {
        const_cast<unsigned char *>(reinterpret_cast<const unsigned char *>(name.data())),
        name.size()
    };
    if (auto *value = cbindgen_private::sixtyfps_interpreter_struct_get_field(&inner, name_view)) {
        return *reinterpret_cast<const Value *>(value);
    } else {
        return {};
    }
}
inline void Struct::set_field(std::string_view name, const Value &value)
{
    cbindgen_private::Slice<uint8_t> name_view {
        const_cast<unsigned char *>(reinterpret_cast<const unsigned char *>(name.data())),
        name.size()
    };
    cbindgen_private::sixtyfps_interpreter_struct_set_field(&inner, name_view, &value.inner);
}

class ComponentInstance
{
    cbindgen_private::ComponentInstance inner;
    ComponentInstance() = delete;
    ComponentInstance(ComponentInstance &) = delete;
    ComponentInstance &operator=(ComponentInstance &) = delete;

public:
    void show() const
    {
        cbindgen_private::sixtyfps_interpreter_component_instance_show(&inner, true);
    }
    void hide() const
    {
        cbindgen_private::sixtyfps_interpreter_component_instance_show(&inner, false);
    }
    void run() const
    {
        show();
        cbindgen_private::sixtyfps_run_event_loop();
        hide();
    }

    bool set_property(std::string_view name, const Value &value) const
    {
        using namespace cbindgen_private;
        return sixtyfps_interpreter_component_instance_set_property(
                &inner, Slice<uint8_t>::from_string(name), &value.inner);
    }
    std::optional<Value> get_property(std::string_view name) const
    {
        using namespace cbindgen_private;
        ValueOpaque out;
        if (sixtyfps_interpreter_component_instance_get_property(
                    &inner, Slice<uint8_t>::from_string(name), &out)) {
            return Value(out);
        } else {
            return {};
        }
    }
    // FIXME! Slice in public API?  Should be std::span (c++20) or we need to improve the Slice API
    std::optional<Value> invoke_callback(std::string_view name, Slice<Value> args) const
    {
        using namespace cbindgen_private;
        Slice<ValueOpaque> args_view { reinterpret_cast<ValueOpaque *>(args.ptr), args.len };
        ValueOpaque out;
        if (sixtyfps_interpreter_component_instance_invoke_callback(
                    &inner, Slice<uint8_t>::from_string(name), args_view, &out)) {
            return Value(out);
        } else {
            return {};
        }
    }

    template<typename F>
    bool set_callback(std::string_view name, F callback) const
    {
        using cbindgen_private::ValueOpaque;
        auto actual_cb = [](void *data, Slice<ValueOpaque> arg, ValueOpaque *ret) {
            Slice<Value> args_view { reinterpret_cast<Value *>(arg.ptr), arg.len };
            Value r = (*reinterpret_cast<F *>(data))(arg);
            new (ret) Value(std::move(r));
        };
        return cbindgen_private::sixtyfps_interpreter_component_instance_set_callback(
                &inner, Slice<uint8_t>::from_string(name), actual_cb, new F(std::move(callback)),
                [](void *data) { delete reinterpret_cast<F *>(data); });
    }
};

class ComponentCompiler
{
    cbindgen_private::ComponentCompilerOpaque inner;

    ComponentCompiler(ComponentCompiler &) = delete;
    ComponentCompiler &operator=(ComponentCompiler &) = delete;

public:
    ComponentCompiler() { cbindgen_private::sixtyfps_interpreter_component_compiler_new(&inner); }

    ~ComponentCompiler()
    {
        cbindgen_private::sixtyfps_interpreter_component_compiler_destructor(&inner);
    }

    void set_include_paths(const sixtyfps::SharedVector<sixtyfps::SharedString> &paths)
    {
        cbindgen_private::sixtyfps_interpreter_component_compiler_set_include_paths(&inner, &paths);
    }

    sixtyfps::SharedVector<sixtyfps::SharedString> include_paths() const
    {
        sixtyfps::SharedVector<sixtyfps::SharedString> paths;
        cbindgen_private::sixtyfps_interpreter_component_compiler_get_include_paths(&inner, &paths);
        return paths;
    }
};

}
