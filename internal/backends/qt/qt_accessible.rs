// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore descendents qobject qwidget

use crate::accessible_generated::*;
use crate::qt_window::QtWindow;

use i_slint_core::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
};
use i_slint_core::item_tree::{ItemRc, ItemWeak};
use i_slint_core::properties::{PropertyDirtyHandler, PropertyTracker};
use i_slint_core::window::WindowInner;
use i_slint_core::SharedVector;

use cpp::*;
use pin_project::pin_project;
use qttypes::QString;

use core::ffi::c_void;
use std::pin::Pin;

// KEEP IN SYNC WITH CONSTANTS IN C++
const NAME: u32 = QAccessible_Text_Name;
const DESCRIPTION: u32 = QAccessible_Text_Description;
const VALUE: u32 = QAccessible_Text_Value;
const CHECKED: u32 = QAccessible_Text_UserText;
const VALUE_MINIMUM: u32 = CHECKED + 1;
const VALUE_MAXIMUM: u32 = VALUE_MINIMUM + 1;
const VALUE_STEP: u32 = VALUE_MAXIMUM + 1;
const CHECKABLE: u32 = VALUE_STEP + 1;
const EXPANDABLE: u32 = CHECKABLE + 1;
const EXPANDED: u32 = EXPANDABLE + 1;
const READ_ONLY: u32 = EXPANDED + 1;

pub struct AccessibleItemPropertiesTracker {
    obj: *mut c_void,
}

impl PropertyDirtyHandler for AccessibleItemPropertiesTracker {
    fn notify(self: Pin<&Self>) {
        let obj = self.obj;
        cpp!(unsafe [obj as "QObject*"] {
            QTimer::singleShot(0, [obj = QPointer(obj)]() {
                if (!obj)
                    return;

                auto accessible_item = static_cast<Slint_accessible_item*>(QAccessible::queryAccessibleInterface(obj));
                auto data = accessible_item->data();
                rust!(AccessibleItemPropertiesTracker_rearm [data: Pin<&SlintAccessibleItemData> as "void*"] {
                    data.arm_state_tracker();
                });

                QAccessible::State s = {};
                s.checked = true; // Mark checked as changed!
                auto event = QAccessibleStateChangeEvent(obj, s);
                QAccessible::updateAccessibility(&event);
            });
        });
    }
}

pub struct ValuePropertyTracker {
    obj: *mut c_void,
}

impl PropertyDirtyHandler for ValuePropertyTracker {
    fn notify(self: Pin<&Self>) {
        let obj = self.obj;
        cpp!(unsafe [obj as "QObject*"] {
            QTimer::singleShot(0, [ obj = QPointer(obj)]() {
                if (!obj)
                    return;

                auto accessible_item = static_cast<Slint_accessible_item*>(QAccessible::queryAccessibleInterface(obj));
                auto data = accessible_item->data();
                rust!(ValuePropertyTracker_rearm [data: Pin<&SlintAccessibleItemData> as "void*"] {
                    data.arm_value_tracker();
                });

                auto event = QAccessibleValueChangeEvent(obj, accessible_item->currentValue());
                QAccessible::updateAccessibility(&event);
            });
        });
    }
}

pub struct LabelPropertyTracker {
    obj: *mut c_void,
}

impl PropertyDirtyHandler for LabelPropertyTracker {
    fn notify(self: Pin<&Self>) {
        let obj = self.obj;
        cpp!(unsafe [obj as "QObject*"] {
            QTimer::singleShot(0, [obj = QPointer(obj)]() {
                if (!obj)
                    return;

                auto accessible_item = static_cast<Slint_accessible_item*>(QAccessible::queryAccessibleInterface(obj));
                auto data = accessible_item->data();
                rust!(LabelPropertyTracker_rearm [data: Pin<&SlintAccessibleItemData> as "void*"] {
                    data.arm_label_tracker();
                });

                auto event = QAccessibleEvent(obj, QAccessible::NameChanged);
                QAccessible::updateAccessibility(&event);
            });
        });
    }
}

pub struct DescriptionPropertyTracker {
    obj: *mut c_void,
}

impl PropertyDirtyHandler for DescriptionPropertyTracker {
    fn notify(self: Pin<&Self>) {
        let obj = self.obj;
        cpp!(unsafe [obj as "QObject*"] {
            QTimer::singleShot(0, [obj = QPointer(obj)]() {
                if (!obj)
                    return;

                auto accessible_item = static_cast<Slint_accessible_item*>(QAccessible::queryAccessibleInterface(obj));
                auto data = accessible_item->data();
                rust!(DescriptionPropertyTracker_rearm [data: Pin<&SlintAccessibleItemData> as "void*"] {
                    data.arm_description_tracker();
                });

                auto event = QAccessibleEvent(obj, QAccessible::DescriptionChanged);
                QAccessible::updateAccessibility(&event);
            });
        });
    }
}

pub struct FocusDelegationPropertyTracker {
    obj: *mut c_void,
}

impl PropertyDirtyHandler for FocusDelegationPropertyTracker {
    fn notify(self: Pin<&Self>) {
        let obj = self.obj;
        cpp!(unsafe [obj as "QObject*"] {
            QTimer::singleShot(0, [obj = QPointer(obj)]() {
                if (!obj)
                    return;

                auto accessible_item = static_cast<Slint_accessible_item*>(QAccessible::queryAccessibleInterface(obj));
                auto data = accessible_item->data();
                rust!(FocusDelegationPropertyTracker_rearm [data: Pin<&SlintAccessibleItemData> as "void*"] {
                    data.arm_focus_delegation_tracker();
                });

                accessible_item->delegateFocus();
            });
        });
    }
}

#[pin_project]
pub struct SlintAccessibleItemData {
    #[pin]
    state_tracker: PropertyTracker<AccessibleItemPropertiesTracker>,
    #[pin]
    value_tracker: PropertyTracker<ValuePropertyTracker>,
    #[pin]
    label_tracker: PropertyTracker<LabelPropertyTracker>,
    #[pin]
    description_tracker: PropertyTracker<DescriptionPropertyTracker>,
    #[pin]
    focus_delegation_tracker: PropertyTracker<FocusDelegationPropertyTracker>,
    item: ItemWeak,
}

impl SlintAccessibleItemData {
    fn new_pin_box(obj: *mut c_void, item: &ItemWeak) -> Pin<Box<Self>> {
        let state_tracker =
            PropertyTracker::new_with_dirty_handler(AccessibleItemPropertiesTracker { obj });
        let value_tracker = PropertyTracker::new_with_dirty_handler(ValuePropertyTracker { obj });
        let label_tracker = PropertyTracker::new_with_dirty_handler(LabelPropertyTracker { obj });
        let description_tracker =
            PropertyTracker::new_with_dirty_handler(DescriptionPropertyTracker { obj });
        let focus_delegation_tracker =
            PropertyTracker::new_with_dirty_handler(FocusDelegationPropertyTracker { obj });

        let result = Box::pin(Self {
            state_tracker,
            value_tracker,
            label_tracker,
            description_tracker,
            focus_delegation_tracker,
            item: item.clone(),
        });

        result.as_ref().arm_state_tracker();
        result.as_ref().arm_value_tracker();
        result.as_ref().arm_label_tracker();
        result.as_ref().arm_description_tracker();
        result.as_ref().arm_focus_delegation_tracker();

        result
    }

    fn arm_state_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.state_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(AccessibleStringProperty::Checkable);
                item_rc.accessible_string_property(AccessibleStringProperty::Checked);
                item_rc.accessible_string_property(AccessibleStringProperty::Expandable);
                item_rc.accessible_string_property(AccessibleStringProperty::Expanded);
                item_rc.accessible_string_property(AccessibleStringProperty::ReadOnly);
            }
        });
    }

    fn arm_value_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.value_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(AccessibleStringProperty::Value);
                item_rc.accessible_string_property(AccessibleStringProperty::ValueMinimum);
                item_rc.accessible_string_property(AccessibleStringProperty::ValueMaximum);
                item_rc.accessible_string_property(AccessibleStringProperty::ValueStep);
            }
        });
    }

    fn arm_label_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.label_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(AccessibleStringProperty::Label);
            }
        });
    }

    fn arm_description_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.description_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(AccessibleStringProperty::Description);
            }
        });
    }

    fn arm_focus_delegation_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.focus_delegation_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(AccessibleStringProperty::DelegateFocus);
            }
        });
    }
}

cpp! {{
    #include <QtGui/QAccessible>
    #include <QtWidgets/QWidget>

    #include <memory>

    /// KEEP IN SYNC WITH CONSTANTS IN RUST!
    const uint32_t CHECKED { QAccessible::UserText };
    const uint32_t VALUE_MINIMUM { CHECKED + 1 };
    const uint32_t VALUE_MAXIMUM { VALUE_MINIMUM + 1 };
    const uint32_t VALUE_STEP { VALUE_MAXIMUM + 1 };
    const uint32_t CHECKABLE { VALUE_STEP + 1 };
    const uint32_t EXPANDABLE { CHECKABLE + 1 };
    const uint32_t EXPANDED { EXPANDABLE + 1 };
    const uint32_t READ_ONLY { EXPANDED + 1 };

    // ------------------------------------------------------------------------------
    // Helper:
    // ------------------------------------------------------------------------------

    class Descendents {
    public:
        Descendents(void *root_item) {
            rustDescendents = rust!(Descendents_ctor [root_item: *mut c_void as "void*"] ->
                    SharedVector<ItemRc> as "void*" {
                i_slint_core::accessibility::accessible_descendents(
                        &*(root_item as *mut ItemRc))
                .collect()
            });
        }

        size_t count() const {
            return rust!(Descendents_count [rustDescendents: SharedVector<ItemRc> as "void*"] -> usize as "size_t" {
               rustDescendents.len()
            });
        }

        void* itemAt(size_t index) {
            return rust!(Descendents_itemAt [rustDescendents: SharedVector<ItemRc> as "void*",
                                             index: usize as "size_t"]
                    -> *mut ItemWeak as "void*" {
                let item_rc = rustDescendents[index].clone();
                let mut item_weak = Box::new(item_rc.downgrade());

                Box::into_raw(item_weak)
            });
        }

        QAccessible::Role roleAt(size_t index) const {
            return rust!(Descendents_roleAt [rustDescendents: SharedVector<ItemRc> as "void*",
                                             index: usize as "size_t"]
                    -> u32 as "QAccessible::Role" {
                match rustDescendents[index].accessible_role() {
                    i_slint_core::items::AccessibleRole::None => QAccessible_Role_NoRole,
                    i_slint_core::items::AccessibleRole::Button => QAccessible_Role_Button,
                    i_slint_core::items::AccessibleRole::Checkbox => QAccessible_Role_CheckBox,
                    i_slint_core::items::AccessibleRole::Combobox => QAccessible_Role_ComboBox,
                    i_slint_core::items::AccessibleRole::List => QAccessible_Role_List,
                    i_slint_core::items::AccessibleRole::Slider => QAccessible_Role_Slider,
                    i_slint_core::items::AccessibleRole::Spinbox => QAccessible_Role_SpinBox,
                    i_slint_core::items::AccessibleRole::Tab => QAccessible_Role_PageTab,
                    i_slint_core::items::AccessibleRole::TabList => QAccessible_Role_PageTabList,
                    i_slint_core::items::AccessibleRole::Text => QAccessible_Role_StaticText,
                    i_slint_core::items::AccessibleRole::ProgressIndicator => QAccessible_Role_ProgressBar,
                    i_slint_core::items::AccessibleRole::Table => QAccessible_Role_Table,
                    i_slint_core::items::AccessibleRole::Tree => QAccessible_Role_Tree,
                    i_slint_core::items::AccessibleRole::TextInput => QAccessible_Role_EditableText,
                    i_slint_core::items::AccessibleRole::Switch => QAccessible_Role_CheckBox,
                    i_slint_core::items::AccessibleRole::ListItem => QAccessible_Role_ListItem,
                    i_slint_core::items::AccessibleRole::TabPanel => QAccessible_Role_Pane,
                    i_slint_core::items::AccessibleRole::Groupbox => QAccessible_Role_Grouping,
                    i_slint_core::items::AccessibleRole::Image => QAccessible_Role_Graphic,
                    _ => QAccessible_Role_NoRole,
                }
            });
        }

        ~Descendents() {
            auto descendentsPtr = &rustDescendents;
            rust!(Descendents_dtor [descendentsPtr: *mut SharedVector<ItemRc> as "void**"] {
                core::ptr::read(descendentsPtr);
            });
        }

    private:
        void *rustDescendents;
    };

    void *root_item_for_window(void *rustWindow) {
        return rust!(root_item_for_window_ [rustWindow: &crate::qt_window::QtWindow as "void*"]
                -> *mut c_void as "void*" {
            let root_item = Box::new(ItemRc::new(WindowInner::from_pub(&rustWindow.window).component(), 0).downgrade());
            Box::into_raw(root_item) as _
        });
    }

    QString item_string_property(void *data, uint32_t what) {
        return rust!(item_string_property_
            [data: &SlintAccessibleItemData as "void*", what: u32 as "uint32_t"]
                -> QString as "QString" {

            if let Some(item) = data.item.upgrade() {
                let string = match what {
                    NAME => item.accessible_string_property(AccessibleStringProperty::Label),
                    DESCRIPTION => item.accessible_string_property(AccessibleStringProperty::Description),
                    VALUE => item.accessible_string_property(AccessibleStringProperty::Value),
                    CHECKED => item.accessible_string_property(AccessibleStringProperty::Checked),
                    VALUE_MINIMUM => item.accessible_string_property(AccessibleStringProperty::ValueMinimum),
                    VALUE_MAXIMUM => item.accessible_string_property(AccessibleStringProperty::ValueMaximum),
                    VALUE_STEP => item.accessible_string_property(AccessibleStringProperty::ValueStep),
                    CHECKABLE => item.accessible_string_property(AccessibleStringProperty::Checkable),
                    EXPANDABLE => item.accessible_string_property(AccessibleStringProperty::Expandable),
                    EXPANDED => item.accessible_string_property(AccessibleStringProperty::Expanded),
                    READ_ONLY => item.accessible_string_property(AccessibleStringProperty::ReadOnly),
                    _ => None,
                };
                if let Some(string) = string {
                    return QString::from(string.as_ref())
                }
            };
            QString::default()
        });
    }

    // ------------------------------------------------------------------------------
    // Slint_accessible:
    // ------------------------------------------------------------------------------

    // Base object for accessibility support
    class Slint_accessible : public QAccessibleInterface {
    public:
        Slint_accessible(QAccessible::Role role, QAccessibleInterface *parent) :
             has_focus(false), has_focus_delegation(false), m_role(role), m_parent(parent)
        { }

        ~Slint_accessible() {
            qDeleteAll(m_children);
        }

        virtual void *rustItem() const = 0;

        // Returns the SlintWidget of the window... we have no other.
        virtual QWidget *qwidget() const = 0;

        QPoint mapToGlobal(const QPoint p) const {
            return qwidget()->mapToGlobal(p);
        }

        QPoint mapFromGlobal(const QPoint p) const {
            return qwidget()->mapFromGlobal(p);
        }

        void clearFocus() {
            has_focus = false;
            has_focus_delegation = false;

            for (int i = 0; i < rawChildCount(); ++i) {
                static_cast<Slint_accessible *>(child(i))->clearFocus();
            }
        }

        virtual void delegateFocus() const {
            sendFocusChangeEvent();
        }

        // Returns true if the item accepted the focus; false otherwise.
        bool focusItem(void *item) const {
            auto my_item = rustItem();
            if (rust!(Slint_accessible_findItem [item: &ItemWeak as "void *", my_item: &ItemWeak as "void*"] -> bool as "bool" {
                item == my_item
            })) {
                has_focus = true;

                delegateFocus();
                return true;
            }

            for (int i = 0; i < rawChildCount(); ++i) {
                if (static_cast<Slint_accessible *>(child(i))->focusItem(item)) {
                    return true;
                }
            }
            return false;
        }

        void sendFocusChangeEvent() const {
            auto event = QAccessibleEvent(object(), QAccessible::Focus);
            QAccessible::updateAccessibility(&event);
            has_focus_delegation = true;
        }

        bool isValid() const override {
            return true;
        }

        // navigation, hierarchy
        QAccessibleInterface *parent() const override {
            return m_parent;
        }

        QAccessibleInterface *focusChild() const override {
            if (has_focus_delegation) {
                return const_cast<QAccessibleInterface *>(static_cast<const QAccessibleInterface *>(this));
            }
            for (int i = 0; i < childCount(); ++i)  {
                if (auto focus = child(i)->focusChild()) return focus;
            }
            return nullptr;
        }

        int indexOfChild(const QAccessibleInterface *child) const override {
            return m_children.indexOf(child->object()); // FIXME: Theoretically we can have several QAIs per QObject!
        }

        // Will *not* trigger a build of the accessibility item tree!
        // Use this from the Slint side to make sure the accessibility
        // item tree is not generated needlessly.
        int rawChildCount() const {
            return m_children.count();
        }

        /// Will *not* trigger a build of the accessibility tree!
        QAccessibleInterface *rawChild(int index) const {
            if (0 <= index && index < rawChildCount())
                return QAccessible::queryAccessibleInterface(m_children[index]);
            return nullptr;
        }

        // May trigger a build of the accessibility item tree!
        // Use this from the Qt API side (which is triggered by the OS accessibility
        // layer to make sure accessibility information is up-to-date.
        int childCount() const override {
            return rawChildCount();
        }

        QAccessibleInterface *child(int index) const override {
            if (0 <= index && index < childCount())
                return QAccessible::queryAccessibleInterface(m_children[index]);
            return nullptr;
        }

        void setText(QAccessible::Text t, const QString &text) override {
            Q_UNUSED(t); Q_UNUSED(text);
        }

        QAccessible::Role role() const override {
            return m_role;
        }

        QRect rect() const override {
            auto item = rustItem();
            QRectF r = rust!(Slint_accessible_item_rect
                [item: *const ItemWeak as "void*"] -> qttypes::QRectF as "QRectF" {
                    if let Some(item_rc) = item.as_ref().unwrap().upgrade() {
                        let geometry = item_rc.geometry();

                        let mapped = item_rc.map_to_window(geometry.origin);

                        qttypes::QRectF {
                            x: mapped.x as _,
                            y: mapped.y as _,
                            width: geometry.width() as _,
                            height: geometry.height() as _,
                        }
                    } else {
                        Default::default()
                    }
                });
            auto topLeft = mapToGlobal(QPoint(static_cast<int>(r.left()), static_cast<int>(r.top())));
            auto bottomRight = mapToGlobal(QPoint(static_cast<int>(r.right()), static_cast<int>(r.bottom())));
            return QRect(topLeft, bottomRight);
        }

        QAccessibleInterface *childAt(int x, int y) const override {
            for (int i = 0; i < childCount(); ++i)  {
                auto c = child(i);
                auto r = c->rect();
                if (r.contains(x, y)) return c;
            }
            return nullptr;
        }

        void updateAccessibilityTree() const;

    protected:
        mutable bool has_focus;
        mutable bool has_focus_delegation;

    private:
        QAccessible::Role m_role = QAccessible::NoRole;
        QAccessibleInterface *m_parent = nullptr;
        mutable QList<QObject*> m_children;
    };

    // ------------------------------------------------------------------------------
    // Slint_accessible_item:
    // ------------------------------------------------------------------------------

    class Slint_accessible_item : public Slint_accessible, public QAccessibleValueInterface, public QAccessibleActionInterface {
    public:
        Slint_accessible_item(void *item, QObject *obj, QAccessible::Role role, QAccessibleInterface *parent) :
            Slint_accessible(role, parent), m_object(obj)
        {
            m_data = rust!(Slint_accessible_item_ctor [obj: *mut c_void as "QObject*",
                    item: &ItemWeak as "void*"] ->
                    *mut SlintAccessibleItemData as "void*" {
                        let data = SlintAccessibleItemData::new_pin_box(obj, item);
                        unsafe { Box::into_raw(Pin::into_inner_unchecked(data)) }
            });
        }

        QAccessibleActionInterface *actionInterface() { return this; }
        QAccessibleValueInterface *valueInterface() { return this; }


        ~Slint_accessible_item() {
            rust!(Slint_accessible_item_dtor [m_data: *mut SlintAccessibleItemData as "void*"] {
                unsafe { Pin::new_unchecked(Box::from_raw(m_data)) };
            });
        }

        void *rustItem() const override {
            return rust!(Slint_accessible_item_rustItem [m_data: Pin<&SlintAccessibleItemData> as "void*"] -> *const ItemWeak as "void*" {
                &m_data.item
            });
        }

        QObject *object() const override {
            return m_object;
        }

        QWidget *qwidget() const override {
            return dynamic_cast<Slint_accessible *>(parent())->qwidget();
        }

        void *data() const {
            return m_data;
        }

        QWindow *window() const override {
            return parent()->window();
        }

        void delegateFocus() const override {
            if (!has_focus) { return; }

            auto index = rust!(Slint_accessible_item_delegate_focus [m_data: Pin<&SlintAccessibleItemData> as "void*"] -> i32 as "int" {
                m_data.item.upgrade()
                    .and_then(|i| { i.accessible_string_property(AccessibleStringProperty::DelegateFocus) })
                    .and_then(|s| s.as_str().parse::<i32>().ok()).unwrap_or(-1)
            });

            if (index >= 0 && index < rawChildCount()) {
                static_cast<Slint_accessible_item*>(rawChild(index))->sendFocusChangeEvent();
            } else {
                sendFocusChangeEvent();
            }
        }

        // properties and state
        QString text(QAccessible::Text t) const override {
            return item_string_property(m_data, t);
        }

        QAccessible::State state() const override {
            auto checked = item_string_property(m_data, CHECKED);

            QAccessible::State state;
            state.active = 1;
            state.focusable = 1;
            state.focused = has_focus_delegation;
            state.checked = (checked == "true") ? 1 : 0;
            state.checkable = (item_string_property(m_data, CHECKABLE) == "true") ? 1 : 0;
            if (item_string_property(m_data, EXPANDABLE) == "true") {
                state.expandable = 1;
                if (item_string_property(m_data, EXPANDED) == "true") {
                    state.expanded = 1;
                } else {
                    state.collapsed = 1;
                }
            }
            state.readOnly = (item_string_property(m_data, READ_ONLY) == "true") ? 1 : 0;
            return state; /* FIXME */
        }

        void *interface_cast(QAccessible::InterfaceType t) override {
            if (t == QAccessible::ValueInterface && !item_string_property(m_data, QAccessible::Value).isEmpty()) {
                return static_cast<QAccessibleValueInterface*>(this);
            } else if (t == QAccessible::ActionInterface) {
                return static_cast<QAccessibleActionInterface*>(this);
            }
            return QAccessibleInterface::interface_cast(t);
        }

        // AccessibleValueInterface:
        QVariant currentValue() const override {
            return item_string_property(m_data, QAccessible::Value);
        }

        void setCurrentValue(const QVariant &value) override {
            QString value_string = value.toString();
            rust!(Slint_accessible_setCurrentValue [m_data: Pin<&SlintAccessibleItemData> as "void*", value_string: qttypes::QString as "QString"] {
                let Some(item) = m_data.item.upgrade() else {return};
                item.accessible_action(&AccessibilityAction::SetValue(i_slint_core::format!("{value_string}")));
            });
        }

        QVariant maximumValue() const override {
            return item_string_property(m_data, VALUE_MAXIMUM);
        }

        QVariant minimumValue() const override {
            return item_string_property(m_data, VALUE_MINIMUM);
        }

        QVariant minimumStepSize() const override {
            return item_string_property(m_data, VALUE_STEP);
        }

        QStringList actionNames() const override {
            int supported = rust!(Slint_accessible_item_supported [m_data: Pin<&SlintAccessibleItemData> as "void*"] -> SupportedAccessibilityAction as "uint" {
                m_data.item.upgrade().map(|i| i.supported_accessibility_actions()).unwrap_or_default()
            });
            QStringList actions;
            if (supported & rust!(Slint_accessible_item_an1 [] -> SupportedAccessibilityAction as "uint" { SupportedAccessibilityAction::Default }))
                actions << QAccessibleActionInterface::pressAction();
            if (supported & rust!(Slint_accessible_item_an2 [] -> SupportedAccessibilityAction as "uint" { SupportedAccessibilityAction::Increment }))
                actions << QAccessibleActionInterface::increaseAction();
            if (supported & rust!(Slint_accessible_item_an3 [] -> SupportedAccessibilityAction as "uint" { SupportedAccessibilityAction::Decrement }))
                actions << QAccessibleActionInterface::decreaseAction();
            if (supported & rust!(Slint_accessible_item_an4 [] -> SupportedAccessibilityAction as "uint" { SupportedAccessibilityAction::Expand }))
                actions << QAccessibleActionInterface::pressAction();
            return actions;
        }

        void doAction(const QString &actionName) override {
            if (actionName == QAccessibleActionInterface::pressAction()) {
                rust!(Slint_accessible_item_do_action1 [m_data: Pin<&SlintAccessibleItemData> as "void*"] {
                    let Some(item) = m_data.item.upgrade() else {return};
                    let supported_actions = item.supported_accessibility_actions();
                    if supported_actions.contains(SupportedAccessibilityAction::Expand) {
                        item.accessible_action(&AccessibilityAction::Expand);
                    } else {
                        item.accessible_action(&AccessibilityAction::Default);
                    }
                });
            } else if (actionName == QAccessibleActionInterface::increaseAction()) {
                rust!(Slint_accessible_item_do_action2 [m_data: Pin<&SlintAccessibleItemData> as "void*"] {
                    let Some(item) = m_data.item.upgrade() else {return};
                    item.accessible_action(&AccessibilityAction::Increment);
                });
            } else if (actionName == QAccessibleActionInterface::decreaseAction()) {
                rust!(Slint_accessible_item_do_action3 [m_data: Pin<&SlintAccessibleItemData> as "void*"] {
                    let Some(item) = m_data.item.upgrade() else {return};
                    item.accessible_action(&AccessibilityAction::Decrement);
                });
            }
        }

        QStringList keyBindingsForAction(const QString &) const override {
            return QStringList();
        }

    private:
        QObject *m_object = nullptr;
        mutable void *m_data = nullptr;
    };

    // ------------------------------------------------------------------------------
    // Slint_accessible_window:
    // ------------------------------------------------------------------------------

    class Slint_accessible_window : public Slint_accessible {
    public:
        Slint_accessible_window(QWidget *widget, void *rust_window) :
            Slint_accessible(QAccessible::Window, QAccessible::queryAccessibleInterface(qApp)),
            m_widget(widget),
            m_rustWindow(rust_window)
        { }

        ~Slint_accessible_window()
        {
            rust!(Slint_accessible_window_dtor [m_rustWindow: *mut c_void as "void*"] {
                alloc::rc::Weak::from_raw(m_rustWindow as *const QtWindow); // Consume the Weak<QtWindow> we hold in our void*!
            });
        }

        bool isUsed() const {
            return is_used;
        }

        void *rustItem() const override {
            return root_item_for_window(m_rustWindow);
        }

        QObject *object() const override {
            return m_widget;
        }

        QWidget *qwidget() const override {
            return m_widget;
        }

        QWindow *window() const override {
            return qobject_cast<QWidget *>(object())->windowHandle();
        }

        int childCount() const override {
            if (!is_used) { updateAccessibilityTree(); }
            is_used = true;
            return Slint_accessible::childCount();
        }

        // properties and state
        QString text(QAccessible::Text t) const override {
            switch (t) {
                case QAccessible::Name: return qobject_cast<QWidget*>(object())->windowTitle();
                default: return QString();
            }
        }

        QAccessible::State state() const override {
            QAccessible::State state;
            state.active = 1;
            state.focusable = 1;
            return state;
        }

    private:
        QWidget *m_widget;
        void *m_rustWindow; // *const QtWindow
        mutable bool is_used = false;
    };

    QList<QObject *> deleteStaleItems(QList<QObject *> &&current_children) {
        // Delete no longer valid objects:
        current_children.erase(std::remove_if(current_children.begin(), current_children.end(), [](QObject *o) {
            auto ai = dynamic_cast<Slint_accessible_item *>(QAccessible::queryAccessibleInterface(o));
            Q_ASSERT(ai);
            auto data = ai->data();

            if (rust!(Slint_delete_stale_items
                    [data: Pin<&SlintAccessibleItemData> as "void*"] -> bool as "bool" {
                data.item.upgrade().is_none()
            })) {
                o->deleteLater();
                return true;
            } else {
                return false;
            }
        }), current_children.end());

        return std::move(current_children);
    }

    int indexOfItem(const QList<QObject *> &existing, void *item) {
        for (int i = 0; i < existing.count(); ++i) {
            auto data = dynamic_cast<Slint_accessible_item *>(QAccessible::queryAccessibleInterface(existing[i]));
            if (rust!(Slint_indexOfItems [data: Pin<&SlintAccessibleItemData> as "void*", item: &ItemWeak as "void*"] -> bool as "bool" {
                data.item == *item
            })) {
                return i;
            }
        }
        return -1;
    }

    QList<QObject *> updateItems(QList<QObject *> &&current_children,
                                 Descendents &descendents,
                                 Slint_accessible *parent) {
        QList<QObject *> children = {};
        children.reserve(descendents.count());

        for (size_t i = 0; i < descendents.count(); ++i) {
            auto item = descendents.itemAt(i);
            auto index = indexOfItem(current_children, item);
            QObject *object = nullptr;
            Slint_accessible_item *ai = nullptr;

            if (index == -1) {
                // Create new item:
                object = new QObject();
                auto role = descendents.roleAt(i);
                ai = new Slint_accessible_item(item, object, role, parent);

                QAccessible::registerAccessibleInterface(ai);
            } else {
                // Reuse existing item:
                object = current_children[index];
                ai = dynamic_cast<Slint_accessible_item *>(QAccessible::queryAccessibleInterface(object));

                current_children.removeAt(index);
            }

            Q_ASSERT(ai);
            Q_ASSERT(object);

            ai->updateAccessibilityTree();

            children.append(object);
        }

        return children;
    }

    void Slint_accessible::updateAccessibilityTree() const {
        QList<QObject *> valid_objects = deleteStaleItems(std::move(m_children));
        auto descendents = Descendents(rustItem());

        m_children = updateItems(std::move(valid_objects), descendents,
                                 const_cast<Slint_accessible *>(this));
    }
}}
