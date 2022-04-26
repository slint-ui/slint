// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cspell:ignore descendents qobject

use cpp::*;

use i_slint_core::item_tree::{ItemRc, ItemWeak};
use qttypes::QString;

use std::pin::Pin;
use std::rc::Weak;

pub struct HasFocusPropertyTracker {
    accessible_item: *mut core::ffi::c_void,
}

impl i_slint_core::properties::PropertyChangeHandler for HasFocusPropertyTracker {
    fn notify(&self) {
        let accessible_item = self.accessible_item;
        cpp!(unsafe [accessible_item as "Slint_accessible_item*"] {
            QTimer::singleShot(0, [accessible_item]() {
                // Delete this once we have returned from here: This is owned by the old
                // QAccessibleInterface!
                auto obj = accessible_item->object();
                auto event = QAccessibleEvent(obj, QAccessible::Focus);
                QAccessible::updateAccessibility(&event);
            });

            accessible_item->armFocusTracker();
        });
    }
}

pub struct AccessibleItemPropertiesTracker {
    accessible_item: *mut core::ffi::c_void,
}

impl i_slint_core::properties::PropertyChangeHandler for AccessibleItemPropertiesTracker {
    fn notify(&self) {
        let accessible_item = self.accessible_item;
        cpp!(unsafe [accessible_item as "Slint_accessible_item*"] {
            QTimer::singleShot(0, [accessible_item]() {
                // Delete this once we have returned from here: This is owned by the old
                // QAccessibleInterface!
                auto obj = accessible_item->object();
                auto id = QAccessible::uniqueId(accessible_item);
                auto new_ai = new Slint_accessible_item(accessible_item->takeRustItem(), obj,
                    accessible_item->role(), accessible_item->parent());
                QAccessible::deleteAccessibleInterface(id);
                QAccessible::registerAccessibleInterface(new_ai);

                auto event = QAccessibleStateChangeEvent(obj, new_ai->state());
                QAccessible::updateAccessibility(&event);
            });
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
            rustDescendents = rust!(Descendents_ctor [root_item: *mut core::ffi::c_void as "void*"] ->
                    *mut core::ffi::c_void as "void*" {
                let mut descendents = i_slint_core::accessibility::accessible_descendents(
                        &*(root_item as *mut ItemRc));
                descendents.shrink_to_fit();
                Box::into_raw(Box::new(descendents)) as _
            });
        }

        size_t count() {
            return rust!(Descendents_count [rustDescendents: *mut core::ffi::c_void as "void*"] -> usize as "size_t" {
               let vector = Box::from_raw(rustDescendents as *mut Vec<ItemRc>);
               let result = vector.len();
               std::mem::forget(vector);

               result
            });
        }

        void* itemAt(size_t index) {
            return rust!(Descendents_itemAt [rustDescendents: *mut core::ffi::c_void as "void*",
                                             index: usize as "size_t"]
                    -> *mut core::ffi::c_void as "void*" {
                let mut vector = Box::from_raw(rustDescendents as *mut Vec<ItemRc>);
                let item_rc = vector[index].clone();
                let mut item_weak = Box::new(item_rc.downgrade());

                let result = core::ptr::addr_of_mut!(*item_weak);

                std::mem::forget(vector);
                std::mem::forget(item_weak);

                result as _
            });
        }

        QAccessible::Role roleAt(size_t index) {
            return rust!(Descendents_roleAt [rustDescendents: *mut core::ffi::c_void as "void*",
                                             index: usize as "size_t"]
                    -> u32 as "QAccessible::Role" {
                let vector = Box::from_raw(rustDescendents as *mut Vec<ItemRc>);
                let result = match vector[index].accessible_role() {
                    i_slint_core::items::AccessibleRole::none => 0x00, // QAccessible::NoRole
                    i_slint_core::items::AccessibleRole::text => 0x29, // QAccessible::StaticText
                    i_slint_core::items::AccessibleRole::button => 0x2b, // QAccessible::Button
                    i_slint_core::items::AccessibleRole::checkbox => 0x2c, // QAccessible::CheckBox
                };
                std::mem::forget(vector);

                result
            });
        }

        ~Descendents() {
            rust!(Descendents_dtor [rustDescendents: *mut core::ffi::c_void as "void*"] {
               Box::from_raw(rustDescendents as *mut Vec<ItemRc>);
            });
        }

    private:
        void *rustDescendents;
    };

    void *root_item_for_window(void *rustWindow) {
        return rust!(root_item_for_window_ [rustWindow: *const core::ffi::c_void as "void*"]
                -> *mut core::ffi::c_void as "void*" {
            let window = &*(rustWindow as *const i_slint_core::window::Window);

            let root_item = Box::new(ItemRc::new(window.component(), 0).downgrade());
            Box::into_raw(root_item) as _
        });
    }

    QString item_string_property(void *rustItem, QAccessible::Text what) {
        return rust!(item_string_property_
            [rustItem: *const core::ffi::c_void as "void*", what: u32 as "QAccessible::Text"]
                -> QString as "QString" {
            let item = rustItem as *const ItemWeak;
            assert!(!item.is_null());

            if let Some(item) = item.as_ref().and_then(|i| i.upgrade()) {
                let string = match what {
                    0 /* Name */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Label),
                    1 /* Description */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Description),
                    0xffff /* UserText */ => item.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Has_focus),
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
                auto focus = child(i)->focusChild();
                if (focus) return focus;
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
                [item: *mut ItemWeak as "void*"] -> qttypes::QRectF as "QRectF" {
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
            Slint_accessible(obj, role, parent), m_rustItem(item)
        {
            m_stateTracker = rust!(Slint_accessible_item_ctor [this: *mut core::ffi::c_void as "Slint_accessible_item*",
                    m_rustItem: &ItemWeak as "void*"] ->
                    *mut i_slint_core::properties::PropertyTracker<AccessibleItemPropertiesTracker> as "void*" {
                        let item = m_rustItem.clone();
                        let property_tracker =
                            i_slint_core::properties::PropertyTracker::new_with_change_handler(AccessibleItemPropertiesTracker {
                                accessible_item: this,
                            });
                        let property_tracker = Box::pin(property_tracker);
                        property_tracker.as_ref().evaluate_as_dependency_root(move || {
                            if let Some(item_rc) = item.upgrade() {
                                item_rc.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Label);
                                item_rc.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Description);
                            }
                        });
                        Box::into_raw(unsafe { core::pin::Pin::into_inner_unchecked(property_tracker) })
                    });
            m_focusTracker = rust!(Slint_accessible_item_ctor_2 [this: *mut core::ffi::c_void as "Slint_accessible_item*"] ->
                    *mut i_slint_core::properties::PropertyTracker<HasFocusPropertyTracker> as "void*" {
                        let property_tracker =
                            i_slint_core::properties::PropertyTracker::new_with_change_handler(HasFocusPropertyTracker {
                                accessible_item: this,
                            });
                        let property_tracker = Box::pin(property_tracker);
                        Box::into_raw(unsafe { core::pin::Pin::into_inner_unchecked(property_tracker) })
                    });
            armFocusTracker();
        }

        ~Slint_accessible_item() {
            rust!(Slint_accessible_item_dtor [
                m_rustItem: *mut ItemWeak as "void*",
                m_focusTracker: *mut i_slint_core::properties::PropertyTracker<HasFocusPropertyTracker> as "void*",
                m_stateTracker: *mut i_slint_core::properties::PropertyTracker<AccessibleItemPropertiesTracker> as "void*"] {
                if !m_rustItem.is_null() {
                    Box::from_raw(m_rustItem);
                };
                Box::from_raw(m_stateTracker);
                Box::from_raw(m_focusTracker);
            });
        }

        void armFocusTracker() {
            rust!(Slint_accessible_item_arm_focus_tracker [
                m_rustItem: &ItemWeak as "void*",
                m_focusTracker: &i_slint_core::properties::PropertyTracker<HasFocusPropertyTracker> as "void*"] {
                    let item = m_rustItem.clone();
                    let m_focusTracker = unsafe { Pin::new_unchecked(m_focusTracker) };
                    m_focusTracker.evaluate_as_dependency_root(move || {
                        if let Some(item_rc) = item.upgrade() {
                            item_rc.accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Has_focus);
                        }
                    });
                    unsafe { Pin::into_inner_unchecked(m_focusTracker) };
            });
        }

        void *rustItem() const override {
            return m_rustItem;
        }

        void *takeRustItem() {
            auto item = m_rustItem;
            m_rustItem = nullptr;
            return item;
        }

        QWindow *window() const override {
            return parent()->window();
        }

        // properties and state
        QString text(QAccessible::Text t) const override {
            return item_string_property(m_rustItem, t);
        }

        QAccessible::State state() const override {
            QAccessible::State state;
            state.active = 1;
            state.focusable = 1;
            state.focused = (item_string_property(m_rustItem, QAccessible::UserText) == "true") ? 1 : 0;
            return state; /* FIXME */
        }

    private:
        void *m_rustItem = nullptr; // A rust item to make accessible (Actually a ItemWeak!)
        void *m_stateTracker = nullptr; // (Actually a Pin<Box<PropertyTracker<AccessibleItemPropertiesTracker>>>)
        void *m_focusTracker = nullptr; // (Actually a Pin<Box<PropertyTracker<AccessibleItemPropertiesTracker>>>)
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
            rust!(Slint_accessible_window_dtor [m_rustWindow: *mut core::ffi::c_void as "void*"] {
                Weak::from_raw(m_rustWindow as _); // Consume the Weak wo hold in our void*!
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
