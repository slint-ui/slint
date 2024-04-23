// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

#include "slint.h"
#include "slint_testing_internal.h"
#include <optional>
#include <string_view>

#ifdef SLINT_FEATURE_TESTING

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
    cbindgen_private::ItemWeak inner;

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
                reinterpret_cast<SharedVector<cbindgen_private::ItemWeak> *>(&result));
        return result;
    }

    /// Returns true if the underlying element still exists; false otherwise.
    bool is_valid() const { return private_api::upgrade_item_weak(inner).has_value(); }

    /// Returns the accessible-label of that element, if any.
    std::optional<SharedString> accessible_label() const
    {
        if (auto item = private_api::upgrade_item_weak(inner)) {
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
        if (auto item = private_api::upgrade_item_weak(inner)) {
            SharedString result;
            if (item->item_tree.vtable()->accessible_string_property(
                        item->item_tree.borrow(), item->index,
                        cbindgen_private::AccessibleStringProperty::Value, &result)) {
                return result;
            }
        }
        return std::nullopt;
    }

    /// Sets the accessible-value of that element.
    ///
    /// Setting the value will invoke the `accessible-action-set-value` callback.
    void set_accessible_value(SharedString value) const
    {
        if (auto item = private_api::upgrade_item_weak(inner)) {
            union SetValueHelper {
                cbindgen_private::AccessibilityAction action;
                SetValueHelper(SharedString value)
                // : action { .set_value = { cbindgen_private::AccessibilityAction::Tag::SetValue,
                //                           std::move(value) } }
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

    /// Invokes the default accessibility action of that element
    /// (`accessible-action-default`).
    void invoke_default_action() const
    {
        if (auto item = private_api::upgrade_item_weak(inner)) {
            union DefaultActionHelper {
                cbindgen_private::AccessibilityAction action;
                DefaultActionHelper()
                //: action { .tag = cbindgen_private::AccessibilityAction::Tag::Default }
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
        if (auto item = private_api::upgrade_item_weak(inner)) {
            auto rect =
                    item->item_tree.vtable()->item_geometry(item->item_tree.borrow(), item->index);
            return LogicalSize({ rect.width, rect.height });
        }
        return LogicalSize({ 0, 0 });
    }

    /// Returns the absolute position of this element
    LogicalPosition absolute_position() const
    {
        if (auto item = private_api::upgrade_item_weak(inner)) {
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

#endif // SLINT_FEATURE_TESTING
