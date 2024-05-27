// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include "slint.h"
#include "slint_testing_internal.h"
#include <optional>
#include <string_view>

#ifdef SLINT_FEATURE_TESTING
#    ifdef SLINT_FEATURE_EXPERIMENTAL

namespace slint::testing {
/// Init the testing backend.
/// Should be called before any other Slint function that can access the platform.
/// Then future windows will not appear on the screen anymore
inline void init()
{
    cbindgen_private::slint_testing_init_backend();
}

/// A Handle to an element to query accessible property for testing purposes.
///
/// Use find_by_accessible_label() to obtain all elements matching the given accessible label.
class ElementHandle
{
    cbindgen_private::ElementHandle inner;

public:
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

    /// Returns the accessible-label of that element, if any.
    std::optional<SharedString> accessible_label() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::Label, &result)) {
                return result;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-value of that element, if any.
    std::optional<SharedString> accessible_value() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::Value, &result)) {
                return result;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-description of that element, if any.
    std::optional<SharedString> accessible_description() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::Description, &result)) {
                return result;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-value-maximum of that element, if any.
    std::optional<float> accessible_value_maximum() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::ValueMaximum, &result)) {
                float value = 0.0;
                if (cbindgen_private::slint_string_to_float(&result, &value)) {
                    return value;
                }
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-value-minimum of that element, if any.
    std::optional<float> accessible_value_minimum() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::ValueMinimum, &result)) {
                float value = 0.0;
                if (cbindgen_private::slint_string_to_float(&result, &value)) {
                    return value;
                }
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-value-step of that element, if any.
    std::optional<float> accessible_value_step() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::ValueStep, &result)) {
                float value = 0.0;
                if (cbindgen_private::slint_string_to_float(&result, &value)) {
                    return value;
                }
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-checked of that element, if any.
    std::optional<bool> accessible_checked() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::Checked, &result)) {
                if (result == "true")
                    return true;
                else if (result == "false")
                    return false;
            }
        }
        return std::nullopt;
    }

    /// Returns the accessible-checkable of that element, if any.
    std::optional<bool> accessible_checkable() const
    {
        if (auto item = private_api::upgrade_item_weak(inner.item)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::Checkable, &result)) {
                if (result == "true")
                    return true;
                else if (result == "false")
                    return false;
            }
        }
        return std::nullopt;
    }

    /// Sets the accessible-value of that element.
    ///
    /// Setting the value will invoke the `accessible-action-set-value` callback.
    void set_accessible_value(SharedString value) const
    {
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
};

}

#    endif // SLINT_FEATURE_EXPERIMENTAL
#endif // SLINT_FEATURE_TESTING
