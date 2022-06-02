// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cspell:ignore descendents qobject

use cpp::*;

use i_slint_core::item_tree::{ItemRc, ItemWeak};
use i_slint_core::SharedVector;

use pin_project::pin_project;
use qttypes::QString;

use alloc::boxed::Box;
use core::ffi::c_void;
use std::pin::Pin;

pub struct HasFocusPropertyTracker {
    accessible_item: *mut c_void,
}

impl i_slint_core::properties::PropertyChangeHandler for HasFocusPropertyTracker {
    fn notify(&self) {
        let accessible_item = self.accessible_item;
        let data = cpp!(unsafe [accessible_item as "Slint_accessible_item*"] -> Pin<&SlintAccessibleItemData> as "void*" {
            auto obj = accessible_item->object();
            auto event = QAccessibleEvent(obj, QAccessible::Focus);

            QAccessible::updateAccessibility(&event);

            return accessible_item->data();
        });
        data.arm_focus_tracker();
    }
}

pub struct AccessibleItemPropertiesTracker {
    accessible_item: *mut c_void,
}

impl i_slint_core::properties::PropertyChangeHandler for AccessibleItemPropertiesTracker {
    fn notify(&self) {
        let accessible_item = self.accessible_item;
        let data = cpp!(unsafe [accessible_item as "Slint_accessible_item*"] -> Pin<&SlintAccessibleItemData> as "void*"{
            auto obj = accessible_item->object();

            auto event = QAccessibleStateChangeEvent(obj, accessible_item->state());
            QAccessible::updateAccessibility(&event);

            return accessible_item->data();
        });
        data.arm_state_tracker();
    }
}

#[pin_project]
pub struct SlintAccessibleItemData {
    #[pin]
    state_tracker: i_slint_core::properties::PropertyTracker<AccessibleItemPropertiesTracker>,
    #[pin]
    focus_tracker: i_slint_core::properties::PropertyTracker<HasFocusPropertyTracker>,
    item: ItemWeak,
}

impl SlintAccessibleItemData {
    fn new(accessible_item: *mut c_void, item: &ItemWeak) -> Pin<Box<Self>> {
        let state_tracker = i_slint_core::properties::PropertyTracker::new_with_change_handler(
            AccessibleItemPropertiesTracker { accessible_item },
        );
        let focus_tracker = i_slint_core::properties::PropertyTracker::new_with_change_handler(
            HasFocusPropertyTracker { accessible_item },
        );

        let result = Box::pin(Self { state_tracker, focus_tracker, item: item.clone() });

        result.as_ref().arm_state_tracker();
        result.as_ref().arm_focus_tracker();

        result
    }

    fn arm_state_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.state_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(
                    i_slint_core::accessibility::AccessibleStringProperty::Label,
                );
                item_rc.accessible_string_property(
                    i_slint_core::accessibility::AccessibleStringProperty::Description,
                );
            }
        });
    }

    fn arm_focus_tracker(self: Pin<&Self>) {
        let item = self.item.clone();
        let p = self.project_ref();
        p.focus_tracker.evaluate_as_dependency_root(move || {
            if let Some(item_rc) = item.upgrade() {
                item_rc.accessible_string_property(
                    i_slint_core::accessibility::AccessibleStringProperty::HasFocus,
                );
            }
        });
    }
}

cpp! {{
    #include <QtWidgets/QtWidgets>

    #include <memory>

    // ------------------------------------------------------------------------------
    // Helper:
    // ------------------------------------------------------------------------------

    class Descendents {
    public:
        Descendents(void *root_item) {
            rustDescendents = rust!(Descendents_ctor [root_item: *mut c_void as "void*"] ->
                    SharedVector<ItemRc> as "void*" {
                let mut descendents = i_slint_core::accessibility::accessible_descendents(
                        &*(root_item as *mut ItemRc));
                SharedVector::from_slice(&descendents)
            });
        }

        size_t count() {
            return rust!(Descendents_count [rustDescendents: SharedVector<ItemRc> as "void*"] -> usize as "size_t" {
               rustDescendents.len()
            });
        }

        void* itemAt(size_t index) {
            return rust!(Descendents_itemAt [rustDescendents: SharedVector<ItemRc> as "void*",
                                             index: usize as "size_t"]
                    -> *mut c_void as "void*" {
                let item_rc = rustDescendents[index].clone();
                let mut item_weak = Box::new(item_rc.downgrade());

                let result = core::ptr::addr_of_mut!(*item_weak);

                std::mem::forget(item_weak);

                result as _
            });
        }

        QAccessible::Role roleAt(size_t index) {
            return rust!(Descendents_roleAt [rustDescendents: SharedVector<ItemRc> as "void*",
                                             index: usize as "size_t"]
                    -> u32 as "QAccessible::Role" {
                match rustDescendents[index].accessible_role() {
                    i_slint_core::items::AccessibleRole::none => 0x00, // QAccessible::NoRole
                    i_slint_core::items::AccessibleRole::button => 0x2b, // QAccessible::Button
                    i_slint_core::items::AccessibleRole::checkbox => 0x2c, // QAccessible::CheckBox
                    i_slint_core::items::AccessibleRole::tab => 0x25, // QAccessible::TabPage
                    i_slint_core::items::AccessibleRole::text => 0x29, // QAccessible::StaticText
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
        return rust!(root_item_for_window_ [rustWindow: &i_slint_core::window::Window as "void*"]
                -> *mut c_void as "void*" {
            let root_item = Box::new(ItemRc::new(rustWindow.component(), 0).downgrade());
            Box::into_raw(root_item) as _
        });
    }

    QString item_string_property(void *data, uint32_t what) {
        return rust!(item_string_property_
            [data: &SlintAccessibleItemData as "void*", what: u32 as "uint32_t"]
                -> QString as "QString" {
            if let Some(item) = data.item.upgrade() {
                let string = match what {
                    0 /* Name */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Label),
                    1 /* Description */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Description),
                    0xffff /* UserText */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::HasFocus),
                    0x10000 /* UserText + 1 */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Checked),
                    _ => Default::default(),
                };
                QString::from(string.as_ref())
            } else {
                QString::default()
            }
        });
    }

    // ------------------------------------------------------------------------------
    // Slint_accessible:
    // ------------------------------------------------------------------------------

    // Base object for accessibility support
    class Slint_accessible : public QAccessibleInterface {
    public:
        Slint_accessible(QObject *obj, QAccessible::Role role, QAccessibleInterface *parent) :
            m_role(role), m_parent(parent), m_object(obj)
        { }

        ~Slint_accessible() {
            qDeleteAll(m_children);
        }

        virtual void *rustItem() const = 0;

        bool isValid() const override {
            return true;
        }

        QObject *object() const override {
            return m_object;
        }

        // relations
        QVector<QPair<QAccessibleInterface *, QAccessible::Relation>>
        relations(QAccessible::Relation match = QAccessible::AllRelations) const override {
            Q_UNUSED(match);
            return {}; /* FIXME */
        }

        // navigation, hierarchy
        QAccessibleInterface *parent() const override {
            return m_parent;
        }

        QAccessibleInterface *focusChild() const override {
            if (state().focused) {
                return const_cast<QAccessibleInterface *>(static_cast<const QAccessibleInterface *>(this));
            }
            for (int i = 0; i < childCount(); ++i)  {
                if (auto focus = child(i)->focusChild()) return focus;
            }
            return nullptr;
        }

        int childCount() const override;

        int indexOfChild(const QAccessibleInterface *child) const override {
            return m_children.indexOf(child->object()); // FIXME: Theoretically we can have several QAIs per QObject!
        }

        QAccessibleInterface *child(int index) const override {
            if (0 <= index && index < m_children.count())
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
                        let geometry = item_rc.borrow().as_ref().geometry();

                        qttypes::QRectF {
                            x: geometry.origin.x as _,
                            y: geometry.origin.y as _,
                            width: geometry.width() as _,
                            height: geometry.height() as _,
                        }
                    } else {
                        Default::default()
                    }
                });
            return QRect(static_cast<int>(r.left()), static_cast<int>(r.top()),
                         static_cast<int>(r.right()), static_cast<int>(r.bottom()));
        }

        QAccessibleInterface *childAt(int x, int y) const override {
            for (int i = 0; i < childCount(); ++i)  {
                auto c = child(i)->childAt(x, y);
                if (c) return c;
            }

            auto r = rect();
            if (r.contains(x, y)) {
                return const_cast<QAccessibleInterface *>(static_cast<const QAccessibleInterface *>(this));
            }
            return nullptr;
        }


        QColor foregroundColor() const override { return {}; /* FIXME */ }
        QColor backgroundColor() const override { return {}; /* FIXME */ }

        void virtual_hook(int id, void *data) override { Q_UNUSED(id); Q_UNUSED(data); /* FIXME */ }

    private:
        QAccessible::Role m_role = QAccessible::NoRole;
        QAccessibleInterface *m_parent = nullptr;
        QObject *m_object = nullptr; // A dummy QObject to use QAccessible // This is directly or indirectly owned by the Slint_accessible_window!
        mutable QList<QObject*> m_children;
    };

    // ------------------------------------------------------------------------------
    // Slint_accessible_item:
    // ------------------------------------------------------------------------------

    class Slint_accessible_item : public Slint_accessible {
    public:
        Slint_accessible_item(void *item, QObject *obj, QAccessible::Role role, QAccessibleInterface *parent) :
            Slint_accessible(obj, role, parent)
        {
            m_data = rust!(Slint_accessible_item_ctor [this: *mut c_void as "Slint_accessible_item*",
                    item: &ItemWeak as "void*"] ->
                    *mut SlintAccessibleItemData as "void*" {
                        let data = SlintAccessibleItemData::new(this, item);
                        unsafe { Box::into_raw(Pin::into_inner_unchecked(data)) }
            });
        }

        ~Slint_accessible_item() {
            rust!(Slint_accessible_item_dtor [m_data: *mut SlintAccessibleItemData as "void*"] {
                if !m_data.is_null() {
                    unsafe { Pin::new_unchecked(Box::from_raw(m_data)) };
                };
            });
        }

        void *rustItem() const override {
            return rust!(Slint_accessible_item_rustItem [m_data: Pin<&SlintAccessibleItemData> as "void*"] -> *const ItemWeak as "void*" {
                &m_data.item
            });
        }

        void *data() const {
            return m_data;
        }

        QWindow *window() const override {
            return parent()->window();
        }

        // properties and state
        QString text(QAccessible::Text t) const override {
            return item_string_property(m_data, t);
        }

        QAccessible::State state() const override {
            QAccessible::State state;
            state.active = 1;
            state.focusable = 1;
            state.focused = (item_string_property(m_data, QAccessible::UserText) == "true") ? 1 : 0;
            state.checked = (item_string_property(m_data, QAccessible::UserText + 1) == "true") ? 1 : 0;
            return state; /* FIXME */
        }

    private:
        mutable void *m_data = nullptr;
    };

    // ------------------------------------------------------------------------------
    // Slint_accessible_window:
    // ------------------------------------------------------------------------------

    class Slint_accessible_window : public Slint_accessible {
    public:
        Slint_accessible_window(QWidget *widget, void *rust_window) :
            Slint_accessible(widget, QAccessible::Window, QAccessible::queryAccessibleInterface(qApp)),
            m_rustWindow(rust_window)
        { }

        ~Slint_accessible_window()
        {
            rust!(Slint_accessible_window_dtor [m_rustWindow: *mut c_void as "void*"] {
                alloc::rc::Weak::from_raw(m_rustWindow as _); // Consume the Weak wo hold in our void*!
            });
        }

        void *rustItem() const override {
            return root_item_for_window(m_rustWindow);
        }

        QWindow *window() const override {
            return qobject_cast<QWidget *>(object())->windowHandle();
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
        void *m_rustWindow;
    };

    int Slint_accessible::childCount() const {
        if (m_children.isEmpty()) {
            auto descendents = Descendents(rustItem());
            for (size_t i = 0; i < descendents.count(); ++i) {
                auto object = new QObject();
                auto ai = new Slint_accessible_item(descendents.itemAt(i),
                    object, descendents.roleAt(i),
                    const_cast<Slint_accessible *>(this));
                QAccessible::registerAccessibleInterface(ai);
                m_children.append(object);
            }
        }
        return m_children.count();
    }
}}
