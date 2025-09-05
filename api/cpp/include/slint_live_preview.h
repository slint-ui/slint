// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint.h"

#ifndef SLINT_FEATURE_LIVE_PREVIEW
#    error SLINT_FEATURE_LIVE_PREVIEW must be activated
#else

#    include "slint-interpreter.h"

/// Internal API to support the live-preview generated code
namespace slint::private_api::live_preview {

template<typename T>
    requires(std::convertible_to<T, slint::interpreter::Value>)
slint::interpreter::Value into_slint_value(const T &val)
{
    return val;
}

template<typename T>
    requires requires(T val) { val.into_slint_value(); }
slint::interpreter::Value into_slint_value(const T &val)
{
    return val.into_slint_value();
}

inline slint::interpreter::Value into_slint_value(const slint::interpreter::Value &val)
{
    return val;
}

template<typename T>
    requires std::is_same_v<T, void>
inline void from_slint_value(const slint::interpreter::Value &, const T *)
{
}
inline bool from_slint_value(const slint::interpreter::Value &val, const bool *)
{
    return val.to_bool().value();
}
inline slint::SharedString from_slint_value(const slint::interpreter::Value &val,
                                            const slint::SharedString *)
{
    return val.to_string().value();
}
inline int from_slint_value(const slint::interpreter::Value &val, const int *)
{
    return val.to_number().value();
}
inline float from_slint_value(const slint::interpreter::Value &val, const float *)
{
    return val.to_number().value();
}
inline slint::Color from_slint_value(const slint::interpreter::Value &val, const slint::Color *)
{
    return val.to_brush().value().color();
}
inline interpreter::Value into_slint_value(const slint::Color &val)
{
    return slint::Brush(val);
}
inline slint::Brush from_slint_value(const slint::interpreter::Value &val, const slint::Brush *)
{
    return val.to_brush().value();
}
inline slint::Image from_slint_value(const slint::interpreter::Value &val, const slint::Image *)
{
    return val.to_image().value();
}
/// duration
inline long int from_slint_value(const slint::interpreter::Value &val, const long int *)
{
    return val.to_number().value();
}
inline interpreter::Value into_slint_value(const long int &val)
{
    return double(val);
}

template<typename ModelData>
std::shared_ptr<slint::Model<ModelData>>
from_slint_value(const slint::interpreter::Value &,
                 const std::shared_ptr<slint::Model<ModelData>> *);

template<typename ModelData>
slint::interpreter::Value into_slint_value(const std::shared_ptr<slint::Model<ModelData>> &val);

inline slint::interpreter::Value into_slint_value(const slint::StandardListViewItem &val)
{
    slint::interpreter::Struct s;
    s.set_field("text", val.text);
    return s;
}

inline slint::StandardListViewItem from_slint_value(const slint::interpreter::Value &val,
                                                    const slint::StandardListViewItem *)
{
    auto s = val.to_struct().value();
    return slint::StandardListViewItem { .text = s.get_field("text").value().to_string().value() };
}

inline slint::interpreter::Value into_slint_value(const slint::LogicalPosition &val)
{
    slint::interpreter::Struct s;
    s.set_field("x", val.x);
    s.set_field("y", val.y);
    return s;
}

inline slint::LogicalPosition from_slint_value(const slint::interpreter::Value &val,
                                               const slint::LogicalPosition *)
{
    auto s = val.to_struct().value();
    return slint::LogicalPosition({ float(s.get_field("x").value().to_number().value()),
                                    float(s.get_field("y").value().to_number().value()) });
}

template<typename T>
T from_slint_value(const slint::interpreter::Value &v)
{
    return from_slint_value(v, static_cast<const T *>(nullptr));
}

class LiveReloadingComponent
{
    const cbindgen_private::LiveReloadingComponentInner *inner;

public:
    /// Libraries is an array of string that have in the form `lib=...`
    LiveReloadingComponent(std::string_view file_name, std::string_view component_name,
                           const slint::SharedVector<slint::SharedString> &include_paths,
                           const slint::SharedVector<slint::SharedString> &libraries,
                           std::string_view style)
    {
        assert_main_thread();
        inner = cbindgen_private::slint_live_preview_new(
                string_to_slice(file_name), string_to_slice(component_name), &include_paths,
                &libraries, string_to_slice(style));
    }

    LiveReloadingComponent(const LiveReloadingComponent &other) : inner(other.inner)
    {
        assert_main_thread();
        cbindgen_private::slint_live_preview_clone(other.inner);
    }
    LiveReloadingComponent &operator=(const LiveReloadingComponent &other)
    {
        assert_main_thread();
        if (this == &other)
            return *this;
        cbindgen_private::slint_live_preview_drop(inner);
        inner = other.inner;
        cbindgen_private::slint_live_preview_clone(inner);
        return *this;
    }
    ~LiveReloadingComponent()
    {
        assert_main_thread();
        cbindgen_private::slint_live_preview_drop(inner);
    }

    void set_property(std::string_view name, const interpreter::Value &value) const
    {
        assert_main_thread();
        return cbindgen_private::slint_live_preview_set_property(inner, string_to_slice(name),
                                                                 value.inner);
    }

    interpreter::Value get_property(std::string_view name) const
    {
        assert_main_thread();
        auto val = slint::interpreter::Value(
                cbindgen_private::slint_live_preview_get_property(inner, string_to_slice(name)));
        return val;
    }

    template<typename... Args>
    interpreter::Value invoke(std::string_view name, Args &...args) const
    {
        assert_main_thread();
        std::array<interpreter::Value, sizeof...(Args)> args_values { into_slint_value(args)... };
        cbindgen_private::Slice<cbindgen_private::Value *> args_slice {
            reinterpret_cast<cbindgen_private::Value **>(args_values.data()), args_values.size()
        };
        interpreter::Value val(cbindgen_private::slint_live_preview_invoke(
                inner, string_to_slice(name), args_slice));
        return val;
    }

    template<std::invocable<std::span<const interpreter::Value>> F>
        requires(std::is_convertible_v<std::invoke_result_t<F, std::span<const interpreter::Value>>,
                                       interpreter::Value>)
    void set_callback(std::string_view name, F &&callback) const
    {
        assert_main_thread();
        auto actual_cb =
                [](void *data,
                   cbindgen_private::Slice<cbindgen_private::Box<cbindgen_private::Value>> arg) {
                    std::span<const interpreter::Value> args_view {
                        reinterpret_cast<const interpreter::Value *>(arg.ptr), arg.len
                    };
                    interpreter::Value r = (*reinterpret_cast<F *>(data))(args_view);
                    auto inner = r.inner;
                    r.inner = cbindgen_private::slint_interpreter_value_new();
                    return inner;
                };
        return cbindgen_private::slint_live_preview_set_callback(
                inner, slint::private_api::string_to_slice(name), actual_cb,
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }

    slint::Window &window() const
    {
        const cbindgen_private::WindowAdapterRcOpaque *win_ptr = nullptr;
        cbindgen_private::slint_live_preview_window(inner, &win_ptr);
        return const_cast<slint::Window &>(*reinterpret_cast<const slint::Window *>(win_ptr));
    }

    // Helper function that abuse the friend on Value
    static slint::interpreter::Value value_from_enum(std::string_view name, std::string_view value)
    {
        return slint::interpreter::Value(cbindgen_private::slint_interpreter_value_new_enum(
                string_to_slice(name), string_to_slice(value)));
    }
    static slint::SharedString get_enum_value(const slint::interpreter::Value &value)
    {
        slint::SharedString result;
        slint::cbindgen_private::slint_interpreter_value_enum_to_string(value.inner, &result);
        return result;
    }
};

class LiveReloadModelWrapperBase : public private_api::ModelChangeListener
{
    cbindgen_private::ModelNotifyOpaque notify;
    // This means that the rust code has ownership of "this" until the drop function is called
    std::shared_ptr<ModelChangeListener> self = nullptr;

    void row_added(size_t index, size_t count) override
    {
        cbindgen_private::slint_interpreter_model_notify_row_added(&notify, index, count);
    }
    void row_changed(size_t index) override
    {
        cbindgen_private::slint_interpreter_model_notify_row_changed(&notify, index);
    }
    void row_removed(size_t index, size_t count) override
    {
        cbindgen_private::slint_interpreter_model_notify_row_removed(&notify, index, count);
    }
    void reset() override { cbindgen_private::slint_interpreter_model_notify_reset(&notify); }

    static const ModelAdaptorVTable *vtable()
    {
        auto row_count = [](VRef<ModelAdaptorVTable> self) -> uintptr_t {
            return reinterpret_cast<LiveReloadModelWrapperBase *>(self.instance)->row_count();
        };
        auto row_data = [](VRef<ModelAdaptorVTable> self,
                           uintptr_t row) -> slint::cbindgen_private::Value * {
            std::optional<interpreter::Value> v =
                    reinterpret_cast<LiveReloadModelWrapperBase *>(self.instance)
                            ->row_data(int(row));
            if (v.has_value()) {
                slint::cbindgen_private::Value *rval = v->inner;
                v->inner = cbindgen_private::slint_interpreter_value_new();
                return rval;
            } else {
                return nullptr;
            }
        };
        auto set_row_data = [](VRef<ModelAdaptorVTable> self, uintptr_t row,
                               slint::cbindgen_private::Value *value) {
            interpreter::Value v(std::move(value));
            reinterpret_cast<LiveReloadModelWrapperBase *>(self.instance)->set_row_data(row, v);
        };
        auto get_notify =
                [](VRef<ModelAdaptorVTable> self) -> const cbindgen_private::ModelNotifyOpaque * {
            return &reinterpret_cast<LiveReloadModelWrapperBase *>(self.instance)->notify;
        };
        auto drop = [](vtable::VRefMut<ModelAdaptorVTable> self) {
            reinterpret_cast<LiveReloadModelWrapperBase *>(self.instance)->self = nullptr;
        };

        static const ModelAdaptorVTable vt { row_count, row_data, set_row_data, get_notify, drop };
        return &vt;
    }

protected:
    LiveReloadModelWrapperBase() { cbindgen_private::slint_interpreter_model_notify_new(&notify); }
    virtual ~LiveReloadModelWrapperBase()
    {
        cbindgen_private::slint_interpreter_model_notify_destructor(&notify);
    }

    virtual int row_count() const = 0;
    virtual std::optional<slint::interpreter::Value> row_data(int i) const = 0;
    virtual void set_row_data(int i, const slint::interpreter::Value &value) = 0;

    static interpreter::Value wrap(std::shared_ptr<LiveReloadModelWrapperBase> wrapper)
    {
        wrapper->self = wrapper;
        return interpreter::Value(cbindgen_private::slint_interpreter_value_new_model(
                reinterpret_cast<uint8_t *>(wrapper.get()), vtable()));
    }

public:
    // get the model wrapper from a value (or nullptr if the value don't contain a model)
    static const LiveReloadModelWrapperBase *get(const slint::interpreter::Value &value)
    {
        if (auto model =
                    cbindgen_private::slint_interpreter_value_to_model(value.inner, vtable())) {
            return reinterpret_cast<const LiveReloadModelWrapperBase *>(model);
        } else {
            return nullptr;
        }
    }
};

template<typename ModelData>
class LiveReloadModelWrapper : public LiveReloadModelWrapperBase
{
public:
    LiveReloadModelWrapper(std::shared_ptr<slint::Model<ModelData>> model) : model(std::move(model))
    {
    }

    std::shared_ptr<slint::Model<ModelData>> model = nullptr;

    int row_count() const override { return model->row_count(); }

    std::optional<slint::interpreter::Value> row_data(int i) const override
    {
        if (auto v = model->row_data(i))
            return into_slint_value(*v);
        else
            return {};
    }

    void set_row_data(int i, const slint::interpreter::Value &value) override
    {
        model->set_row_data(i, from_slint_value<ModelData>(value));
    }

    static slint::interpreter::Value wrap(std::shared_ptr<slint::Model<ModelData>> model)
    {
        auto self = std::make_shared<LiveReloadModelWrapper<ModelData>>(model);
        auto peer = std::weak_ptr<LiveReloadModelWrapperBase>(self);
        model->attach_peer(peer);
        return LiveReloadModelWrapperBase::wrap(self);
    }
};

template<typename ModelData>
slint::interpreter::Value into_slint_value(const std::shared_ptr<slint::Model<ModelData>> &val)
{
    if (!val) {
        return {};
    }
    return LiveReloadModelWrapper<ModelData>::wrap(val);
}

template<typename ModelData>
std::shared_ptr<slint::Model<ModelData>>
from_slint_value(const slint::interpreter::Value &value,
                 const std::shared_ptr<slint::Model<ModelData>> *)
{
    if (const LiveReloadModelWrapperBase *base = LiveReloadModelWrapperBase::get(value)) {
        if (auto wrapper = dynamic_cast<const LiveReloadModelWrapper<ModelData> *>(base)) {
            return wrapper->model;
        }
    }
    return {};
}

} // namespace slint::private_api::live_preview

#endif // SLINT_FEATURE_LIVE_PREVIEW
