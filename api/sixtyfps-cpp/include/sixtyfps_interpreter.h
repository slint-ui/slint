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

#define SIXTYFPS_QT_INTEGRATION // In the future, should be defined by cmake only if this is enabled
#ifdef SIXTYFPS_QT_INTEGRATION
class QWidget;
#endif

namespace sixtyfps::cbindgen_private {
//  This has to stay opaque, but VRc don't compile if it is just forward declared
struct ErasedComponentBox : vtable::Dyn
{
    ~ErasedComponentBox() = delete;
    ErasedComponentBox() = delete;
    ErasedComponentBox(ErasedComponentBox &) = delete;
};
}

namespace sixtyfps::interpreter {

class Value;

/// This type represents a runtime instance of structure in `.60`.
///
/// This can either be an instance of a name structure introduced
/// with the `struct` keyword in the .60 file, or an annonymous struct
/// writen with the `{ key: value, }`  notation.
///
/// It can be constructed with the range constructor or initializer lst,
/// and converted into or from a Value with the Value constructor and
/// Value::to_struct().
struct Struct
{
public:
    /// Constructs a new empty struct. You can add fields with set_field() and
    /// read them with get_field().
    Struct() { cbindgen_private::sixtyfps_interpreter_struct_new(&inner); }

    /// Creates a new Struct as a copy from \a other. All fields are copied as well.
    Struct(const Struct &other)
    {
        cbindgen_private::sixtyfps_interpreter_struct_clone(&other.inner, &inner);
    }
    /// Creates a new Struct by moving all fields from \a other into this struct.
    Struct(Struct &&other)
    {
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_struct_new(&other.inner);
    }
    /// Assigns all the fields of \a other to this struct.
    Struct &operator=(const Struct &other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_struct_destructor(&inner);
        sixtyfps_interpreter_struct_clone(&other.inner, &inner);
        return *this;
    }
    /// Moves all the fields of \a other to this struct.
    Struct &operator=(Struct &&other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_struct_destructor(&inner);
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_struct_new(&other.inner);
        return *this;
    }
    /// Destroys this struct.
    ~Struct() { cbindgen_private::sixtyfps_interpreter_struct_destructor(&inner); }

    /// Creates a new struct with the fields of the std::initializer_list given by args.
    inline Struct(std::initializer_list<std::pair<std::string_view, Value>> args);

    /// Creates a new struct with the fields produced by the iterator \a it. \a it is
    /// advanced until it equals \a end.
    template<typename InputIterator
// Doxygen doesn't understand this template wizardry
#if !defined(DOXYGEN)
             ,
             typename std::enable_if_t<
                     std::is_convertible<decltype(std::get<0>(*std::declval<InputIterator>())),
                                         std::string_view>::value
                     && std::is_convertible<decltype(std::get<1>(*std::declval<InputIterator>())),
                                            Value>::value

                     > * = nullptr
#endif
             >
    Struct(InputIterator it, InputIterator end) : Struct()
    {
        for (; it != end; ++it) {
            auto [key, value] = *it;
            set_field(key, value);
        }
    }

    // FIXME: this probably miss a lot of iterator api
    /// The Struct::iterator class implements the typical C++ iterator protocol and conveniently
    /// provides access to the field names and values of a Struct. It is created by calling either
    /// Struct::begin() or Struct::end().
    ///
    /// Make sure to compare the iterator to the iterator returned by Struct::end() before
    /// de-referencing it. The value returned when de-referencing is a std::pair that holds a
    /// std::string_view of the field name as well as a const reference of the value. Both
    /// references become invalid when the iterator or the Struct is changed, so make sure to make
    /// copies if you want to retain the name or value.
    ///
    /// If you're using C++ 17, you can use the convenience destructuring syntax to extract the name
    /// and value in one go:
    ///
    /// ```
    /// Struct stru = ...;
    /// auto it = stru.begin();
    /// ...
    /// ++it; // advance iterator to the next field
    /// ...
    /// // Check iterator before dereferencing it
    /// if (it != stru.end()) {
    ///     // Extract a view of the name and a const reference to the value in one go.
    ///     auto [field_name, field_value] = *it;
    /// }
    /// ```
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
        /// Destroys this field iterator.
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

    /// Returns an iterator over the fields of the struct.
    iterator begin() const
    {
        return iterator(cbindgen_private::sixtyfps_interpreter_struct_make_iter(&inner));
    }
    /// Returns an iterator that when compared with an iterator returned by begin() can be
    /// used to detect when all fields have been visited.
    iterator end() const { return iterator(); }

    /// Returns the value of the field with the given \a name; Returns an std::optional without
    /// value if the field does not exist.
    inline std::optional<Value> get_field(std::string_view name) const;
    /// Sets the value of the field with the given \a name to the specified \a value. If the field
    /// does not exist yet, it is created; otherwise the existing field is updated to hold the new
    /// value.
    inline void set_field(std::string_view name, const Value &value);

    /// \private
    Struct(const sixtyfps::cbindgen_private::StructOpaque &other)
    {
        cbindgen_private::sixtyfps_interpreter_struct_clone(&other, &inner);
    }

private:
    using StructOpaque = sixtyfps::cbindgen_private::StructOpaque;
    StructOpaque inner;
    friend class Value;
};

/// This is a dynamically typed value used in the SixtyFPS interpreter.
/// It can hold a value of different types, and you should use the
/// different overloaded constructors and the to_xxx() functions to access the
//// value within.
///
/// It is also possible to query the type the value holds by calling the Value::type()
/// function.
///
/// ```
/// Value v(42.0); // Creates a value that holds a double with the value 42.
///
/// Value some_value = ...;
/// // Check if the value has a string
/// if (std::optional<sixtyfps::SharedString> string_value = some_value.to_string())
///     do_something(*string_value);  // Extract the string by de-referencing
/// ```
class Value
{
public:
    /// Constructs a new value of type Value::Type::Void.
    Value() { cbindgen_private::sixtyfps_interpreter_value_new(&inner); }

    /// Constructs a new value by copying \a other.
    Value(const Value &other) { sixtyfps_interpreter_value_clone(&other.inner, &inner); }
    /// Constructs a new value by moving \a other to this.
    Value(Value &&other)
    {
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&other.inner);
    }
    /// Assigns the value \a other to this.
    Value &operator=(const Value &other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_value_destructor(&inner);
        sixtyfps_interpreter_value_clone(&other.inner, &inner);
        return *this;
    }
    /// Moves the value \a other to this.
    Value &operator=(Value &&other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_value_destructor(&inner);
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&other.inner);
        return *this;
    }
    /// Destroys the value.
    ~Value() { cbindgen_private::sixtyfps_interpreter_value_destructor(&inner); }

    /// \private
    using Type = cbindgen_private::ValueType;

    // optional<int> to_int() const;
    // optional<float> to_float() const;
    /// Returns a std::optional that contains a double if the type of this Value is
    /// Type::Double, otherwise an empty optional is returned.
    std::optional<double> to_number() const
    {
        if (auto *number = cbindgen_private::sixtyfps_interpreter_value_to_number(&inner)) {
            return *number;
        } else {
            return {};
        }
    }

    /// Returns a std::optional that contains a string if the type of this Value is
    /// Type::String, otherwise an empty optional is returned.
    std::optional<sixtyfps::SharedString> to_string() const
    {
        if (auto *str = cbindgen_private::sixtyfps_interpreter_value_to_string(&inner)) {
            return *str;
        } else {
            return {};
        }
    }

    /// Returns a std::optional that contains a bool if the type of this Value is
    /// Type::Bool, otherwise an empty optional is returned.
    std::optional<bool> to_bool() const
    {
        if (auto *b = cbindgen_private::sixtyfps_interpreter_value_to_bool(&inner)) {
            return *b;
        } else {
            return {};
        }
    }

    /// Returns a std::optional that contains a vector of values if the type of this Value is
    /// Type::Array, otherwise an empty optional is returned.
    inline std::optional<sixtyfps::SharedVector<Value>> to_array() const;

    /// Returns a std::optional that contains a model of values if the type of this Value is
    /// Type::Model, otherwise an empty optional is returned.
    std::optional<std::shared_ptr<sixtyfps::Model<Value>>> to_model() const;

    /// Returns a std::optional that contains a brush if the type of this Value is
    /// Type::Brush, otherwise an empty optional is returned.
    std::optional<sixtyfps::Brush> to_brush() const
    {
        if (auto *brush = cbindgen_private::sixtyfps_interpreter_value_to_brush(&inner)) {
            return *brush;
        } else {
            return {};
        }
    }

    /// Returns a std::optional that contains a Struct if the type of this Value is
    /// Type::Struct, otherwise an empty optional is returned.
    std::optional<Struct> to_struct() const
    {
        if (auto *opaque_struct = cbindgen_private::sixtyfps_interpreter_value_to_struct(&inner)) {
            return Struct(*opaque_struct);
        } else {
            return {};
        }
    }

    // template<typename T> std::optional<T> get() const;

    /// Constructs a new Value that holds the double \a value.
    Value(double value) { cbindgen_private::sixtyfps_interpreter_value_new_double(value, &inner); }
    /// Constructs a new Value that holds the string \a str.
    Value(const SharedString &str)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_string(&str, &inner);
    }
    /// Constructs a new Value that holds the boolean \a b.
    Value(bool b) { cbindgen_private::sixtyfps_interpreter_value_new_bool(b, &inner); }
    /// Constructs a new Value that holds the value vector \a v.
    inline Value(const SharedVector<Value> &v);
    /// Constructs a new Value that holds the value model \a m.
    Value(const std::shared_ptr<sixtyfps::Model<Value>> &m);
    /// Constructs a new Value that holds the brush \a b.
    Value(const sixtyfps::Brush &brush)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_brush(&brush, &inner);
    }
    /// Constructs a new Value that holds the Struct \a struc.
    Value(const Struct &struc)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_struct(&struc.inner, &inner);
    }

    /// Returns the type the variant holds.
    Type type() const { return cbindgen_private::sixtyfps_interpreter_value_type(&inner); }

    /// Returns true if \a and \b hold values of the same type and the underlying vales are equal.
    friend bool operator==(const Value &a, const Value &b)
    {
        return cbindgen_private::sixtyfps_interpreter_value_eq(&a.inner, &b.inner);
    }
    /// Returns true if \a and \b hold values of the same type and the underlying vales are not
    /// equal.
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

class ComponentInstance : vtable::Dyn
{
    ComponentInstance() = delete;
    ComponentInstance(ComponentInstance &) = delete;
    ComponentInstance &operator=(ComponentInstance &) = delete;
    friend class ComponentDefinition;

    // ComponentHandle<ComponentInstance>  is in fact a VRc<ComponentVTable, ErasedComponentBox>
    const cbindgen_private::ErasedComponentBox *inner() const
    {
        return reinterpret_cast<const cbindgen_private::ErasedComponentBox *>(this);
    }

public:
    void show() const
    {
        cbindgen_private::sixtyfps_interpreter_component_instance_show(inner(), true);
    }
    void hide() const
    {
        cbindgen_private::sixtyfps_interpreter_component_instance_show(inner(), false);
    }
    void run() const
    {
        show();
        cbindgen_private::sixtyfps_run_event_loop();
        hide();
    }
#ifdef SIXTYFPS_QT_INTEGRATION
    /// Return a QWidget for this instance.
    /// This function is only available if the qt graphical backend was compiled in, and
    /// it may return nullptr if the Qt backend is not used at runtime.
    QWidget *qwidget() const
    {
        cbindgen_private::ComponentWindowOpaque win;
        cbindgen_private::sixtyfps_interpreter_component_instance_window(inner(), &win);
        return reinterpret_cast<QWidget *>(cbindgen_private::sixtyfps_qt_get_widget(
                reinterpret_cast<cbindgen_private::ComponentWindow *>(&win)));
    }
#endif

    bool set_property(std::string_view name, const Value &value) const
    {
        using namespace cbindgen_private;
        return sixtyfps_interpreter_component_instance_set_property(
                inner(), sixtyfps::private_api::string_to_slice(name), &value.inner);
    }
    std::optional<Value> get_property(std::string_view name) const
    {
        using namespace cbindgen_private;
        ValueOpaque out;
        if (sixtyfps_interpreter_component_instance_get_property(
                    inner(), sixtyfps::private_api::string_to_slice(name), &out)) {
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
                    inner(), sixtyfps::private_api::string_to_slice(name), args_view, &out)) {
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
            Value r = (*reinterpret_cast<F *>(data))(args_view);
            new (ret) Value(std::move(r));
        };
        return cbindgen_private::sixtyfps_interpreter_component_instance_set_callback(
                inner(), sixtyfps::private_api::string_to_slice(name), actual_cb,
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }
};

/// ComponentDefinition is a representation of a compiled component from .60 markup.
///
/// It can be constructed from a .60 file using the ComponentCompiler::build_from_path() or
/// ComponentCompiler::build_from_source() functions. And then it can be instantiated with the
/// create() function.
///
/// The ComponentDefinition acts as a factory to create new instances. When you've finished
/// creating the instances it is safe to destroy the ComponentDefinition.
class ComponentDefinition
{
    friend class ComponentCompiler;

    using ComponentDefinitionOpaque = sixtyfps::cbindgen_private::ComponentDefinitionOpaque;
    ComponentDefinitionOpaque inner;

    ComponentDefinition() = delete;
    // Internal constructor that takes ownership of the component definition
    explicit ComponentDefinition(ComponentDefinitionOpaque &inner) : inner(inner) { }

public:
    /// Constructs a new ComponentDefinition as a copy of \a other.
    ComponentDefinition(const ComponentDefinition &other)
    {
        sixtyfps_interpreter_component_definition_clone(&other.inner, &inner);
    }
    /// Assigns \a other to this ComponentDefinition.
    ComponentDefinition &operator=(const ComponentDefinition &other)
    {
        using namespace sixtyfps::cbindgen_private;

        if (this == &other)
            return *this;

        sixtyfps_interpreter_component_definition_destructor(&inner);
        sixtyfps_interpreter_component_definition_clone(&other.inner, &inner);

        return *this;
    }
    /// Destroys this ComponentDefinition.
    ~ComponentDefinition() { sixtyfps_interpreter_component_definition_destructor(&inner); }
    /// Creates a new instance of the component and returns a shared handle to it.
    ComponentHandle<ComponentInstance> create() const
    {
        union CI {
            cbindgen_private::ComponentInstance i;
            ~CI() { i.~ComponentInstance(); }
            CI() { }
        } u;
        cbindgen_private::sixtyfps_interpreter_component_instance_create(&inner, &u.i);
        return *reinterpret_cast<ComponentHandle<ComponentInstance> *>(&u.i);
    }
};

using Diagnostic = sixtyfps::cbindgen_private::CDiagnostic;
using DiagnosticLevel = sixtyfps::cbindgen_private::CDiagnosticLevel;

/// ComponentCompiler is the entry point to the SixtyFPS interpreter that can be used
/// to load .60 files or compile them on-the-fly from a string.
class ComponentCompiler
{
    cbindgen_private::ComponentCompilerOpaque inner;

    ComponentCompiler(ComponentCompiler &) = delete;
    ComponentCompiler &operator=(ComponentCompiler &) = delete;

public:
    /// Constructs a new ComponentCompiler instance.
    ComponentCompiler() { cbindgen_private::sixtyfps_interpreter_component_compiler_new(&inner); }

    /// Destroys this ComponentCompiler.
    ~ComponentCompiler()
    {
        cbindgen_private::sixtyfps_interpreter_component_compiler_destructor(&inner);
    }

    /// Sets the include paths used for looking up `.60` imports to the specified vector of paths.
    void set_include_paths(const sixtyfps::SharedVector<sixtyfps::SharedString> &paths)
    {
        cbindgen_private::sixtyfps_interpreter_component_compiler_set_include_paths(&inner, &paths);
    }

    /// Sets the style to be used for widgets.
    void set_style(std::string_view style)
    {
        cbindgen_private::sixtyfps_interpreter_component_compiler_set_style(
                &inner, sixtyfps::private_api::string_to_slice(style));
    }

    /// Returns the widget style the compiler is currently using when compiling .60 files.
    sixtyfps::SharedString style() const
    {
        sixtyfps::SharedString s;
        cbindgen_private::sixtyfps_interpreter_component_compiler_get_style(&inner, &s);
        return s;
    }

    /// Returns the include paths the component compiler is currently configured with.
    sixtyfps::SharedVector<sixtyfps::SharedString> include_paths() const
    {
        sixtyfps::SharedVector<sixtyfps::SharedString> paths;
        cbindgen_private::sixtyfps_interpreter_component_compiler_get_include_paths(&inner, &paths);
        return paths;
    }

    /// Returns the diagnostics that were produced in the last call to build_from_path() or
    /// build_from_source().
    sixtyfps::SharedVector<Diagnostic> diagnostics() const
    {
        sixtyfps::SharedVector<Diagnostic> result;
        cbindgen_private::sixtyfps_interpreter_component_compiler_get_diagnostics(&inner, &result);
        return result;
    }

    /// Compile a .60 file into a ComponentDefinition
    ///
    /// Returns the compiled `ComponentDefinition` if there were no errors.
    ///
    /// Any diagnostics produced during the compilation, such as warnigns or errors, are collected
    /// in this ComponentCompiler and can be retrieved after the call using the diagnostics()
    /// function.
    ///
    /// Diagnostics from previous calls are cleared when calling this function.
    std::optional<ComponentDefinition> build_from_source(std::string_view source_code,
                                                         std::string_view path)
    {
        cbindgen_private::ComponentDefinitionOpaque result;
        if (cbindgen_private::sixtyfps_interpreter_component_compiler_build_from_source(
                    &inner, sixtyfps::private_api::string_to_slice(source_code),
                    sixtyfps::private_api::string_to_slice(path), &result)) {

            return ComponentDefinition(result);
        } else {
            return {};
        }
    }

    /// Compile some .60 code into a ComponentDefinition
    ///
    /// The `path` argument will be used for diagnostics and to compute relative
    /// paths while importing.
    ///
    /// Any diagnostics produced during the compilation, such as warnings or errors, are collected
    /// in this ComponentCompiler and can be retrieved after the call using the
    /// Self::diagnostics() function.
    ///
    /// Diagnostics from previous calls are cleared when calling this function.
    std::optional<ComponentDefinition> build_from_path(std::string_view path)
    {
        cbindgen_private::ComponentDefinitionOpaque result;
        if (cbindgen_private::sixtyfps_interpreter_component_compiler_build_from_path(
                    &inner, sixtyfps::private_api::string_to_slice(path), &result)) {

            return ComponentDefinition(result);
        } else {
            return {};
        }
    }
};

}
