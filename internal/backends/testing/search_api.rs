// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::ops::ControlFlow;
use i_slint_core::accessibility::{AccessibilityAction, AccessibleStringProperty};
use i_slint_core::api::ComponentHandle;
use i_slint_core::item_tree::{ItemTreeRc, ItemVisitorResult, ItemWeak, TraversalOrder};
use i_slint_core::items::ItemRc;
use i_slint_core::window::WindowInner;
use i_slint_core::SharedString;

fn warn_missing_debug_info() {
    i_slint_core::debug_log!("The use of the ElementHandle API requires the presence of debug info in Slint compiler generated code. Set the `SLINT_EMIT_DEBUG_INFO=1` environment variable at application build time")
}

mod internal {
    /// Used as base of another trait so it cannot be re-implemented
    pub trait Sealed {}
}

pub(crate) use internal::Sealed;

/// Trait for type that can be searched for element. This is implemented for everything that implements [`ComponentHandle`]
pub trait ElementRoot: Sealed {
    #[doc(hidden)]
    fn item_tree(&self) -> ItemTreeRc;
}

impl<T: ComponentHandle> ElementRoot for T {
    fn item_tree(&self) -> ItemTreeRc {
        WindowInner::from_pub(self.window()).component()
    }
}

impl<T: ComponentHandle> Sealed for T {}

/// `ElementHandle`` wraps an existing element in a Slint UI. An ElementHandle does not keep
/// the corresponding element in the UI alive. Use [`Self::is_valid()`] to verify that
/// it is still alive.
///
/// Obtain instances of `ElementHandle` by querying your application through
/// [`Self::find_by_accessible_label()`].
#[derive(Clone)]
#[repr(C)]
pub struct ElementHandle {
    item: ItemWeak,
    element_index: usize, // When multiple elements get optimized into a single ItemRc, this index separates.
}

impl ElementHandle {
    fn collect_elements(item: ItemRc) -> impl Iterator<Item = ElementHandle> {
        (0..item.element_count().unwrap_or_else(|| {
            warn_missing_debug_info();
            0
        }))
            .map(move |element_index| ElementHandle { item: item.downgrade(), element_index })
    }

    /// Visit elements of a component and call the visitor to each of them, until the visitor returns [`ControlFlow::Break`].
    /// When the visitor breaks, the function returns the value. If it doesn't break, the function returns None.
    pub fn visit_elements<R>(
        component: &impl ElementRoot,
        mut visitor: impl FnMut(ElementHandle) -> ControlFlow<R>,
    ) -> Option<R> {
        let mut result = None;
        let item_tree = component.item_tree();
        i_slint_core::item_tree::visit_items(
            &item_tree,
            TraversalOrder::BackToFront,
            |parent_tree, _, index, _| {
                let item_rc = ItemRc::new(parent_tree.clone(), index);
                let elements = ElementHandle::collect_elements(item_rc);
                for e in elements {
                    match visitor(e) {
                        ControlFlow::Continue(_) => (),
                        ControlFlow::Break(x) => {
                            result = Some(x);
                            return ItemVisitorResult::Abort;
                        }
                    }
                }
                ItemVisitorResult::Continue(())
            },
            (),
        );
        result
    }

    /// This function searches through the entire tree of elements of `component`, looks for
    /// elements that have a `accessible-label` property with the provided value `label`,
    /// and returns an iterator over the found elements.
    pub fn find_by_accessible_label(
        component: &impl ElementRoot,
        label: &str,
    ) -> impl Iterator<Item = Self> {
        let mut result = Vec::new();
        Self::visit_elements::<()>(component, |elem| {
            if elem.accessible_label().is_some_and(|x| x == label) {
                result.push(elem);
            }
            ControlFlow::Continue(())
        });
        result.into_iter()
    }

    /// This function searches through the entire tree of elements of this window and looks for
    /// elements by their id. The id is a qualified string consisting of the name of the component
    /// and the assigned name within the component. In the following examples, the first Button
    /// has the id "MyView::submit-button" and the second button "App::close":
    ///
    /// ```slint,no-preview
    /// component MyView {
    ///    submit-button := Button {}
    /// }
    /// export component App {
    ///     VerticalLayout {
    ///         close := Button {}
    ///     }
    /// }
    /// ```
    pub fn find_by_element_id(
        component: &impl ElementRoot,
        id: &str,
    ) -> impl Iterator<Item = Self> {
        let mut id_split = id.split("::");
        let type_name = id_split.next();
        let local_id = id_split.next();
        let root_base = if local_id == Some("root") { type_name } else { None };

        let mut result = Vec::new();
        Self::visit_elements::<()>(component, |elem| {
            if elem.id().unwrap() == id {
                result.push(elem);
            } else if let Some(root_base) = root_base {
                if elem.type_name().unwrap() == root_base
                    || elem.bases().unwrap().any(|base| base == root_base)
                {
                    result.push(elem);
                }
            }
            ControlFlow::Continue(())
        });
        result.into_iter()
    }

    /// This function searches through the entire tree of elements of `component`, looks for
    /// elements with given type name.
    pub fn find_by_element_type_name(
        component: &impl ElementRoot,
        type_name: &str,
    ) -> impl Iterator<Item = Self> {
        let mut result = Vec::new();
        Self::visit_elements::<()>(component, |elem| {
            if elem.type_name().unwrap() == type_name
                || elem.bases().unwrap().any(|tn| tn == type_name)
            {
                result.push(elem);
            }
            ControlFlow::Continue(())
        });
        result.into_iter()
    }

    /// Returns true if the element still exists in the in UI and is valid to access; false otherwise.
    pub fn is_valid(&self) -> bool {
        self.item.upgrade().is_some()
    }

    /// Returns the element's qualified id. Returns None if the element is not valid anymore or the
    /// element does not have an id.
    /// A qualified id consists of the name of the surrounding component as well as the provided local
    /// name, separate by a double colon.
    ///
    /// ```rust
    /// # i_slint_backend_testing::init_no_event_loop();
    /// slint::slint!{
    ///
    /// component PushButton {
    ///     /* .. */
    /// }
    ///
    /// export component App {
    ///    mybutton := PushButton { } // known as `App::mybutton`
    ///    PushButton { } // no id
    /// }
    ///
    /// }
    ///
    /// let app = App::new().unwrap();
    /// let button = i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "App::mybutton")
    ///              .next().unwrap();
    /// assert_eq!(button.id().unwrap(), "App::mybutton");
    /// ```
    pub fn id(&self) -> Option<SharedString> {
        self.item.upgrade().and_then(|item| {
            item.element_type_names_and_ids(self.element_index)
                .unwrap_or_else(|| {
                    warn_missing_debug_info();
                    Default::default()
                })
                .into_iter()
                .next()
                .map(|(_, id)| id)
        })
    }

    /// Returns the element's type name; None if the element is not valid anymore.
    ///
    /// ```rust
    /// # i_slint_backend_testing::init_no_event_loop();
    /// slint::slint!{
    ///
    /// component PushButton {
    ///     /* .. */
    /// }
    ///
    /// export component App {
    ///    mybutton := PushButton { }
    /// }
    ///
    /// }
    ///
    /// let app = App::new().unwrap();
    /// let button = i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "App::mybutton")
    ///              .next().unwrap();
    /// assert_eq!(button.type_name().unwrap(), "PushButton");
    /// ```
    pub fn type_name(&self) -> Option<SharedString> {
        self.item.upgrade().and_then(|item| {
            item.element_type_names_and_ids(self.element_index)
                .unwrap_or_else(|| {
                    warn_missing_debug_info();
                    Default::default()
                })
                .into_iter()
                .next()
                .map(|(type_name, _)| type_name)
        })
    }

    /// Returns the element's base types as an iterator; None if the element is not valid anymore.
    ///
    /// ```rust
    /// # i_slint_backend_testing::init_no_event_loop();
    /// slint::slint!{
    ///
    /// component ButtonBase {
    ///     /* .. */
    /// }
    ///
    /// component PushButton inherits ButtonBase {
    ///     /* .. */
    /// }
    ///
    /// export component App {
    ///    mybutton := PushButton { }
    /// }
    ///
    /// }
    ///
    /// let app = App::new().unwrap();
    /// let button = i_slint_backend_testing::ElementHandle::find_by_element_id(&app, "App::mybutton")
    ///              .next().unwrap();
    /// assert_eq!(button.type_name().unwrap(), "PushButton");
    /// assert_eq!(button.bases().unwrap().collect::<Vec<_>>(),
    ///           ["ButtonBase"]);
    /// ```
    pub fn bases(&self) -> Option<impl Iterator<Item = SharedString>> {
        self.item.upgrade().map(|item| {
            item.element_type_names_and_ids(self.element_index)
                .unwrap_or_else(|| {
                    warn_missing_debug_info();
                    Default::default()
                })
                .into_iter()
                .skip(1)
                .filter_map(
                    |(type_name, _)| {
                        if !type_name.is_empty() {
                            Some(type_name)
                        } else {
                            None
                        }
                    },
                )
        })
    }

    /// Invokes the default accessible action on the element. For example a `MyButton` element might declare
    /// an accessible default action that simulates a click, as in the following example:
    ///
    /// ```slint,no-preview
    /// component MyButton {
    ///     // ...
    ///     callback clicked();
    ///     in property <string> text;
    ///
    ///     TouchArea {
    ///         clicked => { root.clicked() }
    ///     }
    ///     accessible-role: button;
    ///     accessible-label: self.text;
    ///     accessible-action-default => { self.clicked(); }
    /// }
    /// ```
    pub fn invoke_accessible_default_action(&self) {
        if self.element_index != 0 {
            return;
        }
        if let Some(item) = self.item.upgrade() {
            item.accessible_action(&AccessibilityAction::Default)
        }
    }

    /// Returns the value of the element's `accessible-value` property, if present.
    pub fn accessible_value(&self) -> Option<SharedString> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Value))
    }

    /// Sets the value of the element's `accessible-value` property. Note that you can only set this
    /// property if it is declared in your Slint code.
    pub fn set_accessible_value(&self, value: impl Into<SharedString>) {
        if self.element_index != 0 {
            return;
        }
        if let Some(item) = self.item.upgrade() {
            item.accessible_action(&AccessibilityAction::SetValue(value.into()))
        }
    }

    /// Returns the value of the element's `accessible-value-maximum` property, if present.
    pub fn accessible_value_maximum(&self) -> Option<f32> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            item.accessible_string_property(AccessibleStringProperty::ValueMaximum)
                .and_then(|item| item.parse().ok())
        })
    }

    /// Returns the value of the element's `accessible-value-minimum` property, if present.
    pub fn accessible_value_minimum(&self) -> Option<f32> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            item.accessible_string_property(AccessibleStringProperty::ValueMinimum)
                .and_then(|item| item.parse().ok())
        })
    }

    /// Returns the value of the element's `accessible-value-step` property, if present.
    pub fn accessible_value_step(&self) -> Option<f32> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            item.accessible_string_property(AccessibleStringProperty::ValueStep)
                .and_then(|item| item.parse().ok())
        })
    }

    /// Returns the value of the `accessible-label` property, if present.
    pub fn accessible_label(&self) -> Option<SharedString> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Label))
    }

    /// Returns the value of the `accessible-description` property, if present
    pub fn accessible_description(&self) -> Option<SharedString> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Description))
    }

    /// Returns the value of the `accessible-checked` property, if present
    pub fn accessible_checked(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Checked))
            .and_then(|item| item.parse().ok())
    }

    /// Returns the value of the `accessible-checkable` property, if present
    pub fn accessible_checkable(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Checkable))
            .and_then(|item| item.parse().ok())
    }

    /// Returns the size of the element in logical pixels. This corresponds to the value of the `width` and
    /// `height` properties in Slint code. Returns a zero size if the element is not valid.
    pub fn size(&self) -> i_slint_core::api::LogicalSize {
        self.item
            .upgrade()
            .map(|item| {
                let g = item.geometry();
                i_slint_core::lengths::logical_size_to_api(g.size)
            })
            .unwrap_or_default()
    }

    /// Returns the position of the element within the entire window. This corresponds to the value of the
    /// `absolute-position` property in Slint code. Returns a zero position if the element is not valid.
    pub fn absolute_position(&self) -> i_slint_core::api::LogicalPosition {
        self.item
            .upgrade()
            .map(|item| {
                let g = item.geometry();
                let p = item.map_to_window(g.origin);
                i_slint_core::lengths::logical_position_to_api(p)
            })
            .unwrap_or_default()
    }

    /// Invokes the element's `accessible-action-increment` callback, if declared. On widgets such as spinboxes, this
    /// typically increments the value.
    pub fn invoke_accessible_increment_action(&self) {
        if self.element_index != 0 {
            return;
        }
        if let Some(item) = self.item.upgrade() {
            item.accessible_action(&AccessibilityAction::Increment)
        }
    }

    /// Invokes the element's `accessible-action-decrement` callback, if declared. On widgets such as spinboxes, this
    /// typically decrements the value.
    pub fn invoke_accessible_decrement_action(&self) {
        if self.element_index != 0 {
            return;
        }
        if let Some(item) = self.item.upgrade() {
            item.accessible_action(&AccessibilityAction::Decrement)
        }
    }
}

#[test]
fn test_optimized() {
    crate::init_no_event_loop();

    slint::slint! {
        export component App inherits Window {
            first := Rectangle {
                second := Rectangle {
                    third := Rectangle {}
                }
            }
        }
    }

    let app = App::new().unwrap();
    let mut it = ElementHandle::find_by_element_id(&app, "App::first");
    let first = it.next().unwrap();
    assert!(it.next().is_none());

    assert_eq!(first.type_name().unwrap(), "Rectangle");
    assert_eq!(first.id().unwrap(), "App::first");
    assert_eq!(first.bases().unwrap().count(), 0);

    it = ElementHandle::find_by_element_id(&app, "App::second");
    let second = it.next().unwrap();
    assert!(it.next().is_none());

    assert_eq!(second.type_name().unwrap(), "Rectangle");
    assert_eq!(second.id().unwrap(), "App::second");
    assert_eq!(second.bases().unwrap().count(), 0);

    it = ElementHandle::find_by_element_id(&app, "App::third");
    let third = it.next().unwrap();
    assert!(it.next().is_none());

    assert_eq!(third.type_name().unwrap(), "Rectangle");
    assert_eq!(third.id().unwrap(), "App::third");
    assert_eq!(third.bases().unwrap().count(), 0);
}

#[test]
fn test_conditional() {
    crate::init_no_event_loop();

    slint::slint! {
        export component App inherits Window {
            in property <bool> condition: false;
            if condition: dynamic-elem := Rectangle {}
        }
    }

    let app = App::new().unwrap();
    let mut it = ElementHandle::find_by_element_id(&app, "App::dynamic-elem");
    assert!(it.next().is_none());

    app.set_condition(true);

    it = ElementHandle::find_by_element_id(&app, "App::dynamic-elem");
    let elem = it.next().unwrap();
    assert!(it.next().is_none());

    assert_eq!(elem.id().unwrap(), "App::dynamic-elem");
    assert_eq!(elem.type_name().unwrap(), "Rectangle");
    assert_eq!(elem.bases().unwrap().count(), 0);

    app.set_condition(false);

    // traverse the item tree before testing elem.is_valid()
    assert!(ElementHandle::find_by_element_id(&app, "App::dynamic-elem").next().is_none());
    assert!(!elem.is_valid());
}
