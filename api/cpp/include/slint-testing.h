// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include "slint.h"
#include "slint_testing_internal.h"
#include <cstdint>
#include <optional>
#include <string_view>
#include <type_traits>

#ifdef SLINT_FEATURE_TESTING
#    ifdef SLINT_FEATURE_EXPERIMENTAL

/// Use the functions and classes in this namespace for in-process UI testing.
///
/// This module is still experimental - its API is subject to changes and not stabilized yet. To
/// use the module, you must enable the `SLINT_FEATURE_EXPERIMENTAL=ON` and `SLINT_FEATURE_TESTING`
/// CMake options.
namespace slint::testing {

using slint::cbindgen_private::AccessibleRole;

/// Init the testing backend.
/// Should be called before any other Slint function that can access the platform.
/// Then future windows will not appear on the screen anymore
inline void init()
{
    cbindgen_private::slint_testing_init_backend();
}

/// A handle to an element for querying accessible properties, intended for testing purposes.
class ElementHandle
{
    cbindgen_private::ElementHandle inner;

    explicit ElementHandle(const cbindgen_private::ElementHandle *inner) : inner(*inner) { }

public:
    /// Visits visible elements within a component and calls the visitor for each of them.
    ///
    /// The visitor must be a callable object that accepts an `ElementHandle` and returns either
    /// `void`, or a type that can be converted to `bool`.
    /// - If the visitor returns `void`, the visitation continues until all elements have been
    ///   visited.
    /// - If the visitor returns a type that can be converted to `bool`, the visitation continues as
    ///   long as the conversion result is false; otherwise, it stops, returning that value.
    ///   If the visitor never returns something that converts to true, then the function returns a
    ///   default constructed value;
    ///
    /// ```cpp
    /// auto element = ElementHandle::visit_elements(component, [&](const ElementHandle& eh)
    ///          -> std::optional<ElementHandle> {
    ///      return eh.id() == "Foo::bar" ? std::make_optional(eh) : std::nullopt;
    /// });
    /// ```
    template<typename T, std::invocable<ElementHandle> Visitor,
             typename R = std::invoke_result_t<Visitor, ElementHandle>>
        requires((std::is_constructible_v<bool, R> && std::is_default_constructible_v<R>)
                 || std::is_void_v<R>)
    static auto visit_elements(const ComponentHandle<T> &component, Visitor visitor)
            -> std::invoke_result_t<Visitor, ElementHandle>
    {
        // using R = std::invoke_result_t<Visitor, ElementHandle>;
        auto vrc = component.into_dyn();
        if constexpr (std::is_void_v<R>) {
            cbindgen_private::slint_testing_element_visit_elements(
                    &vrc, &visitor,
                    [](void *visitor, const cbindgen_private::ElementHandle *element) {
                        (*reinterpret_cast<Visitor *>(visitor))(ElementHandle(element));
                        return false;
                    });
            return;
        } else {
            struct VisitorAndResult
            {
                Visitor &visitor;
                R result;
            } visitor_and_result { visitor, R {} };
            cbindgen_private::slint_testing_element_visit_elements(
                    &vrc, &visitor_and_result,
                    [](void *user_data, const cbindgen_private::ElementHandle *element) {
                        auto visitor_and_result = reinterpret_cast<VisitorAndResult *>(user_data);
                        if (auto r = visitor_and_result->visitor(ElementHandle(element))) {
                            visitor_and_result->result = std::move(r);
                            return true;
                        };
                        return false;
                    });
            return visitor_and_result.result;
        }
    }

    /// Find all elements matching the given accessible label.
    template<typename T>
    static SharedVector<ElementHandle> find_by_accessible_label(const ComponentHandle<T> &component,
                                                                std::string_view label)
    {
        cbindgen_private::Slice<uint8_t> label_view {
            const_cast<unsigned char *>(reinterpret_cast<const unsigned char *>(label.data())),
            label.size()
        };
        auto vrc = component.into_dyn();
        SharedVector<ElementHandle> result;
        cbindgen_private::slint_testing_element_find_by_accessible_label(
                &vrc, &label_view,
                reinterpret_cast<SharedVector<cbindgen_private::ElementHandle> *>(&result));
        return result;
    }

    /// Find all elements matching the given element_id.
    template<typename T>
    static SharedVector<ElementHandle> find_by_element_id(const ComponentHandle<T> &component,
                                                          std::string_view element_id)
    {
        cbindgen_private::Slice<uint8_t> element_id_view {
            const_cast<unsigned char *>(reinterpret_cast<const unsigned char *>(element_id.data())),
            element_id.size()
        };
        auto vrc = component.into_dyn();
        SharedVector<ElementHandle> result;
        cbindgen_private::slint_testing_element_find_by_element_id(
                &vrc, &element_id_view,
                reinterpret_cast<SharedVector<cbindgen_private::ElementHandle> *>(&result));
        return result;
    }

    /// Find all elements matching the given type name.
    template<typename T>
    static SharedVector<ElementHandle>
    find_by_element_type_name(const ComponentHandle<T> &component, std::string_view type_name)
    {
        cbindgen_private::Slice<uint8_t> element_type_name_view {
            const_cast<unsigned char *>(reinterpret_cast<const unsigned char *>(type_name.data())),
            type_name.size()
        };
        auto vrc = component.into_dyn();
        SharedVector<ElementHandle> result;
        cbindgen_private::slint_testing_element_find_by_element_type_name(
                &vrc, &element_type_name_view,
                reinterpret_cast<SharedVector<cbindgen_private::ElementHandle> *>(&result));
        return result;
    }

    /// Returns true if the underlying element still exists; false otherwise.
    bool is_valid() const { return private_api::upgrade_item_weak(inner.item).has_value(); }

    /// Returns the element's qualified id. Returns None if the element is not valid anymore or the
    /// element does not have an id.
    /// A qualified id consists of the name of the surrounding component as well as the provided
    /// local name, separate by a double colon.
    ///
    /// ```slint,no-preview
    /// component PushButton {
    ///     /* .. */
    /// }
    ///
    /// export component App {
    ///    mybutton := PushButton { } // known as `App::mybutton`
    ///    PushButton { } // no id
    /// }
    /// ```
    std::optional<SharedString> id() const
    {
        SharedString id;
        if (cbindgen_private::slint_testing_element_id(&inner, &id)) {
            return id;
        } else {
            return std::nullopt;
        }
    }

    /// Returns the element's type name; std::nullopt if the element is not valid anymore.
    /// ```slint,no-preview
    /// component PushButton {
    ///     /* .. */
    /// }
    ///
    /// export component App {
    ///    mybutton := PushButton { } // type_name is "PushButton"
    /// }
    /// ```
    std::optional<SharedString> type_name() const
    {
        SharedString type_name;
        if (cbindgen_private::slint_testing_element_type_name(&inner, &type_name)) {
            return type_name;
        } else {
            return std::nullopt;
        }
    }

    /// Returns the element's base types as an iterator; None if the element is not valid anymore.
    ///
    /// ```slint,no-preview
    /// component ButtonBase {
    ///     /* .. */
    /// }
    ///
    /// component PushButton inherits ButtonBase {
    ///     /* .. */
    /// }
    ///
    /// export component App {
    ///    mybutton := PushButton { } // bases will be ["ButtonBase"]
    /// }
    /// ```
    std::optional<SharedVector<SharedString>> bases() const
    {
        SharedVector<SharedString> bases;
        if (cbindgen_private::slint_testing_element_bases(&inner, &bases)) {
            return bases;
        } else {
            return std::nullopt;
        }
    }

    /// Returns the value of the element's `accessible-role` property, if present. Use this property
    /// to locate elements by their type/role, i.e. buttons, checkboxes, etc.
    std::optional<slint::testing::AccessibleRole> accessible_role() const
    {
        if (inner.element_index != 0)
            return std::nullopt;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            return item->item_tree.vtable()->accessible_role(item->item_tree.borrow(), item->index);
        }
        return std::nullopt;
    }

    /// Returns the accessible-label of that element, if any.
    std::optional<SharedString> accessible_label() const
    {
        return get_accessible_string_property(cbindgen_private::AccessibleStringProperty::Label);
    }

    /// Returns the accessible-enabled of that element, if any.
    std::optional<bool> accessible_enabled() const
    {
        return get_accessible_bool_property(cbindgen_private::AccessibleStringProperty::Enabled);
    }

    /// Returns the accessible-value of that element, if any.
    std::optional<SharedString> accessible_value() const
    {
        return get_accessible_string_property(cbindgen_private::AccessibleStringProperty::Value);
    }

    /// Returns the accessible-placeholder-text of that element, if any.
    std::optional<SharedString> accessible_placeholder_text() const
    {
        return get_accessible_string_property(
                cbindgen_private::AccessibleStringProperty::PlaceholderText);
    }

    /// Returns the accessible-description of that element, if any.
    std::optional<SharedString> accessible_description() const
    {
        return get_accessible_string_property(
                cbindgen_private::AccessibleStringProperty::Description);
    }

    /// Returns the accessible-value-maximum of that element, if any.
    std::optional<float> accessible_value_maximum() const
    {
        if (auto result = get_accessible_string_property(
                    cbindgen_private::AccessibleStringProperty::ValueMaximum)) {
            float value = 0.0;
            if (cbindgen_private::slint_string_to_float(&*result, &value)) {
                return value;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-value-minimum of that element, if any.
    std::optional<float> accessible_value_minimum() const
    {
        if (auto result = get_accessible_string_property(
                    cbindgen_private::AccessibleStringProperty::ValueMinimum)) {
            float value = 0.0;
            if (cbindgen_private::slint_string_to_float(&*result, &value)) {
                return value;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-value-step of that element, if any.
    std::optional<float> accessible_value_step() const
    {
        if (auto result = get_accessible_string_property(
                    cbindgen_private::AccessibleStringProperty::ValueStep)) {
            float value = 0.0;
            if (cbindgen_private::slint_string_to_float(&*result, &value)) {
                return value;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-checked of that element, if any.
    std::optional<bool> accessible_checked() const
    {
        return get_accessible_bool_property(cbindgen_private::AccessibleStringProperty::Checked);
    }

    /// Returns the accessible-checkable of that element, if any.
    std::optional<bool> accessible_checkable() const
    {
        return get_accessible_bool_property(cbindgen_private::AccessibleStringProperty::Checkable);
    }

    /// Returns the accessible-item-selected of that element, if any.
    std::optional<bool> accessible_item_selected() const
    {
        return get_accessible_bool_property(
                cbindgen_private::AccessibleStringProperty::ItemSelected);
    }

    /// Returns the accessible-item-selectable of that element, if any.
    std::optional<bool> accessible_item_selectable() const
    {
        return get_accessible_bool_property(
                cbindgen_private::AccessibleStringProperty::ItemSelectable);
    }

    /// Returns the accessible-item-index of that element, if any.
    std::optional<size_t> accessible_item_index() const
    {
        if (auto result = get_accessible_string_property(
                    cbindgen_private::AccessibleStringProperty::ItemIndex)) {
            uintptr_t value = 0;
            if (cbindgen_private::slint_string_to_usize(&*result, &value)) {
                return value;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-item-count of that element, if any.
    std::optional<size_t> accessible_item_count() const
    {
        if (auto result = get_accessible_string_property(
                    cbindgen_private::AccessibleStringProperty::ItemCount)) {
            uintptr_t value = 0;
            if (cbindgen_private::slint_string_to_usize(&*result, &value)) {
                return value;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-expanded of that element, if any.
    std::optional<bool> accessible_expanded() const
    {
        return get_accessible_bool_property(cbindgen_private::AccessibleStringProperty::Expanded);
    }

    /// Returns the accessible-expandable of that element, if any.
    std::optional<bool> accessible_expandable() const
    {
        return get_accessible_bool_property(cbindgen_private::AccessibleStringProperty::Expandable);
    }

    /// Returns the accessible-read-only of that element, if any.
    std::optional<bool> accessible_read_only() const
    {
        return get_accessible_bool_property(cbindgen_private::AccessibleStringProperty::ReadOnly);
    }

    /// Invokes the expand accessibility action of that element
    /// (`accessible-action-expand`).
    void invoke_accessible_expand_action() const
    {
        if (inner.element_index != 0)
            return;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            union ExpandActionHelper {
                cbindgen_private::AccessibilityAction action;
                ExpandActionHelper()
                {
                    action.tag = cbindgen_private::AccessibilityAction::Tag::Expand;
                }
                ~ExpandActionHelper() { }

            } action;
            item->item_tree.vtable()->accessibility_action(item->item_tree.borrow(), item->index,
                                                           &action.action);
        }
    }

    /// Sets the accessible-value of that element.
    ///
    /// Setting the value will invoke the `accessible-action-set-value` callback.
    void set_accessible_value(SharedString value) const
    {
        if (inner.element_index != 0)
            return;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            union SetValueHelper {
                cbindgen_private::AccessibilityAction action;
                SetValueHelper(SharedString value)
                {
                    new (&action.set_value) cbindgen_private::AccessibilityAction::SetValue_Body {
                        cbindgen_private::AccessibilityAction::Tag::SetValue, std::move(value)
                    };
                }
                ~SetValueHelper() { action.set_value.~SetValue_Body(); }

            } action(std::move(value));
            item->item_tree.vtable()->accessibility_action(item->item_tree.borrow(), item->index,
                                                           &action.action);
        }
    }

    /// Invokes the increase accessibility action of that element
    /// (`accessible-action-increment`).
    void invoke_accessible_increment_action() const
    {
        if (inner.element_index != 0)
            return;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            union IncreaseActionHelper {
                cbindgen_private::AccessibilityAction action;
                IncreaseActionHelper()
                {
                    action.tag = cbindgen_private::AccessibilityAction::Tag::Increment;
                }
                ~IncreaseActionHelper() { }

            } action;
            item->item_tree.vtable()->accessibility_action(item->item_tree.borrow(), item->index,
                                                           &action.action);
        }
    }

    /// Invokes the decrease accessibility action of that element
    /// (`accessible-action-decrement`).
    void invoke_accessible_decrement_action() const
    {
        if (inner.element_index != 0)
            return;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            union DecreaseActionHelper {
                cbindgen_private::AccessibilityAction action;
                DecreaseActionHelper()
                {
                    action.tag = cbindgen_private::AccessibilityAction::Tag::Decrement;
                }
                ~DecreaseActionHelper() { }

            } action;
            item->item_tree.vtable()->accessibility_action(item->item_tree.borrow(), item->index,
                                                           &action.action);
        }
    }

    /// Invokes the default accessibility action of that element
    /// (`accessible-action-default`).
    void invoke_accessible_default_action() const
    {
        if (inner.element_index != 0)
            return;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            union DefaultActionHelper {
                cbindgen_private::AccessibilityAction action;
                DefaultActionHelper()
                {
                    action.tag = cbindgen_private::AccessibilityAction::Tag::Default;
                }
                ~DefaultActionHelper() { }

            } action;
            item->item_tree.vtable()->accessibility_action(item->item_tree.borrow(), item->index,
                                                           &action.action);
        }
    }

    /// Returns the size of this element
    LogicalSize size() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            auto rect =
                    item->item_tree.vtable()->item_geometry(item->item_tree.borrow(), item->index);
            return LogicalSize({ rect.width, rect.height });
        }
        return LogicalSize({ 0, 0 });
    }

    /// Returns the absolute position of this element
    LogicalPosition absolute_position() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            cbindgen_private::LogicalRect rect =
                    item->item_tree.vtable()->item_geometry(item->item_tree.borrow(), item->index);
            cbindgen_private::LogicalPoint abs =
                    slint::cbindgen_private::slint_item_absolute_position(&item->item_tree,
                                                                          item->index);
            return LogicalPosition({ abs.x + rect.x, abs.y + rect.y });
        }
        return LogicalPosition({ 0, 0 });
    }

private:
    std::optional<SharedString>
    get_accessible_string_property(cbindgen_private::AccessibleStringProperty what) const
    {
        if (inner.element_index != 0)
            return std::nullopt;
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(item->item_tree.borrow(),
                                                                     item->index, what, &result)) {
                return result;
            }
        }
        return std::nullopt;
    }

    std::optional<bool>
    get_accessible_bool_property(cbindgen_private::AccessibleStringProperty what) const
    {
        if (auto result = get_accessible_string_property(what)) {
            if (*result == "true")
                return true;
            else if (*result == "false")
                return false;
        }
        return std::nullopt;
    }
};
}

#    endif // SLINT_FEATURE_EXPERIMENTAL
#endif // SLINT_FEATURE_TESTING
