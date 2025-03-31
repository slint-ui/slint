// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::ops::ControlFlow;
use i_slint_core::accessibility::{AccessibilityAction, AccessibleStringProperty};
use i_slint_core::api::{ComponentHandle, LogicalPosition};
use i_slint_core::item_tree::{ItemTreeRc, ItemWeak, ParentItemTraversalMode};
use i_slint_core::items::{ItemRc, Opacity};
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
    /// Returns the root of the element tree.
    fn root_element(&self) -> ElementHandle {
        let item_rc = ItemRc::new(self.item_tree(), 0);
        ElementHandle { item: item_rc.downgrade(), element_index: 0 }
    }
}

impl<T: ComponentHandle> ElementRoot for T {
    fn item_tree(&self) -> ItemTreeRc {
        WindowInner::from_pub(self.window()).component()
    }
}

impl<T: ComponentHandle> Sealed for T {}

enum SingleElementMatch {
    MatchById { id: String, root_base: Option<String> },
    MatchByTypeName(String),
    MatchByTypeNameOrBase(String),
    MatchByAccessibleRole(crate::AccessibleRole),
    MatchByPredicate(Box<dyn Fn(&ElementHandle) -> bool>),
}

impl SingleElementMatch {
    fn matches(&self, element: &ElementHandle) -> bool {
        match self {
            SingleElementMatch::MatchById { id, root_base } => {
                if element.id().is_some_and(|candidate_id| candidate_id == id) {
                    return true;
                }
                root_base.as_ref().is_some_and(|root_base| {
                    element
                        .type_name()
                        .is_some_and(|type_name_candidate| type_name_candidate == root_base)
                        || element
                            .bases()
                            .is_some_and(|mut bases| bases.any(|base| base == root_base))
                })
            }
            SingleElementMatch::MatchByTypeName(type_name) => element
                .type_name()
                .is_some_and(|candidate_type_name| candidate_type_name == type_name),
            SingleElementMatch::MatchByTypeNameOrBase(type_name) => {
                element
                    .type_name()
                    .is_some_and(|candidate_type_name| candidate_type_name == type_name)
                    || element.bases().is_some_and(|mut bases| bases.any(|base| base == type_name))
            }
            SingleElementMatch::MatchByAccessibleRole(role) => {
                element.accessible_role() == Some(*role)
            }
            SingleElementMatch::MatchByPredicate(predicate) => (predicate)(element),
        }
    }
}

enum ElementQueryInstruction {
    MatchDescendants,
    MatchSingleElement(SingleElementMatch),
}

impl ElementQueryInstruction {
    fn match_recursively(
        query_stack: &[Self],
        element: ElementHandle,
        control_flow_after_first_match: ControlFlow<()>,
        active_popups: &[(ItemRc, ItemTreeRc)],
    ) -> (ControlFlow<()>, Vec<ElementHandle>) {
        let Some((query, tail)) = query_stack.split_first() else {
            return (control_flow_after_first_match, vec![element]);
        };

        match query {
            ElementQueryInstruction::MatchDescendants => {
                let mut results = vec![];
                match element.visit_descendants_impl(
                    &mut |child| {
                        let (next_control_flow, sub_results) = Self::match_recursively(
                            tail,
                            child,
                            control_flow_after_first_match,
                            active_popups,
                        );
                        results.extend(sub_results);
                        next_control_flow
                    },
                    active_popups,
                ) {
                    Some(_) => (ControlFlow::Break(()), results),
                    None => (ControlFlow::Continue(()), results),
                }
            }
            ElementQueryInstruction::MatchSingleElement(criteria) => {
                let mut results = vec![];
                let control_flow = if criteria.matches(&element) {
                    let (next_control_flow, sub_results) = Self::match_recursively(
                        tail,
                        element,
                        control_flow_after_first_match,
                        active_popups,
                    );
                    results.extend(sub_results);
                    next_control_flow
                } else {
                    ControlFlow::Continue(())
                };
                (control_flow, results)
            }
        }
    }
}

/// Use ElementQuery to form a query into the tree of UI elements and then locate one or multiple
/// matching elements.
///
/// ElementQuery uses the builder pattern to concatenate criteria, such as searching for descendants,
/// or matching elements only with a certain id.
///
/// Construct an instance of this by calling [`ElementQuery::from_root`] or [`ElementHandle::query_descendants`]. Apply additional criterial on the returned `ElementQuery`
/// and fetch results by either calling [`Self::find_first()`] to collect just the first match or
/// [`Self::find_all()`] to collect all matches for the query.
pub struct ElementQuery {
    root: ElementHandle,
    query_stack: Vec<ElementQueryInstruction>,
}

impl ElementQuery {
    /// Creates a new element query starting at the root of the tree and matching all descendants.
    pub fn from_root(component: &impl ElementRoot) -> Self {
        component.root_element().query_descendants()
    }

    /// Applies any subsequent matches to all descendants of the results of the query up to this point.
    pub fn match_descendants(mut self) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchDescendants);
        self
    }

    /// Include only elements in the results where [`ElementHandle::id()`] is equal to the provided `id`.
    pub fn match_id(mut self, id: impl Into<String>) -> Self {
        let id = id.into().replace('_', "-");
        let mut id_split = id.split("::");
        let type_name = id_split.next().map(ToString::to_string);
        let local_id = id_split.next();
        let root_base = if local_id == Some("root") { type_name } else { None };

        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchById { id, root_base },
        ));
        self
    }

    /// Include only elements in the results where [`ElementHandle::type_name()`] is equal to the provided `type_name`.
    pub fn match_type_name(mut self, type_name: impl Into<String>) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchByTypeName(type_name.into()),
        ));
        self
    }

    /// Include only elements in the results where [`ElementHandle::type_name()`] or [`ElementHandle::bases()`] is contains to the provided `type_name`.
    pub fn match_inherits(mut self, type_name: impl Into<String>) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchByTypeNameOrBase(type_name.into()),
        ));
        self
    }

    /// Include only elements in the results where [`ElementHandle::accessible_role()`] is equal to the provided `role`.
    pub fn match_accessible_role(mut self, role: crate::AccessibleRole) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchByAccessibleRole(role),
        ));
        self
    }

    pub fn match_predicate(mut self, predicate: impl Fn(&ElementHandle) -> bool + 'static) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchByPredicate(Box::new(predicate)),
        ));
        self
    }

    /// Runs the query and returns the first result; returns None if no element matches the selected
    /// criteria.
    pub fn find_first(&self) -> Option<ElementHandle> {
        ElementQueryInstruction::match_recursively(
            &self.query_stack,
            self.root.clone(),
            ControlFlow::Break(()),
            &self.root.active_popups(),
        )
        .1
        .into_iter()
        .next()
    }

    /// Runs the query and returns a vector of all matching elements.
    pub fn find_all(&self) -> Vec<ElementHandle> {
        ElementQueryInstruction::match_recursively(
            &self.query_stack,
            self.root.clone(),
            ControlFlow::Continue(()),
            &self.root.active_popups(),
        )
        .1
    }
}

/// `ElementHandle` wraps an existing element in a Slint UI. An ElementHandle does not keep
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

    /// Visit all descendants of this element and call the visitor to each of them, until the visitor returns [`ControlFlow::Break`].
    /// When the visitor breaks, the function returns the value. If it doesn't break, the function returns None.
    pub fn visit_descendants<R>(
        &self,
        mut visitor: impl FnMut(ElementHandle) -> ControlFlow<R>,
    ) -> Option<R> {
        self.visit_descendants_impl(&mut |e| visitor(e), &self.active_popups())
    }

    /// Visit all descendants of this element and call the visitor to each of them, until the visitor returns [`ControlFlow::Break`].
    /// When the visitor breaks, the function returns the value. If it doesn't break, the function returns None.
    fn visit_descendants_impl<R>(
        &self,
        visitor: &mut dyn FnMut(ElementHandle) -> ControlFlow<R>,
        active_popups: &[(ItemRc, ItemTreeRc)],
    ) -> Option<R> {
        let self_item = self.item.upgrade()?;

        let visit_attached_popups =
            |item_rc: &ItemRc, visitor: &mut dyn FnMut(ElementHandle) -> ControlFlow<R>| {
                for (popup_elem, popup_item_tree) in active_popups {
                    if popup_elem == item_rc {
                        if let Some(result) = (ElementHandle {
                            item: ItemRc::new(popup_item_tree.clone(), 0).downgrade(),
                            element_index: 0,
                        })
                        .visit_descendants_impl(visitor, active_popups)
                        {
                            return Some(result);
                        }
                    }
                }
                None
            };

        visit_attached_popups(&self_item, visitor);

        self_item.visit_descendants(move |item_rc| {
            if !item_rc.is_visible() {
                return ControlFlow::Continue(());
            }

            if let Some(result) = visit_attached_popups(item_rc, visitor) {
                return ControlFlow::Break(result);
            }

            let elements = ElementHandle::collect_elements(item_rc.clone());
            for e in elements {
                let result = visitor(e);
                if matches!(result, ControlFlow::Break(..)) {
                    return result;
                }
            }
            ControlFlow::Continue(())
        })
    }

    /// Creates a new [`ElementQuery`] to match any descendants of this element.
    pub fn query_descendants(&self) -> ElementQuery {
        ElementQuery {
            root: self.clone(),
            query_stack: vec![ElementQueryInstruction::MatchDescendants],
        }
    }

    /// This function searches through the entire tree of elements of `component`, looks for
    /// elements that have a `accessible-label` property with the provided value `label`,
    /// and returns an iterator over the found elements.
    pub fn find_by_accessible_label(
        component: &impl ElementRoot,
        label: &str,
    ) -> impl Iterator<Item = Self> {
        let label = label.to_string();
        let results = component
            .root_element()
            .query_descendants()
            .match_predicate(move |elem| {
                elem.accessible_label().is_some_and(|candidate_label| candidate_label == label)
            })
            .find_all();
        results.into_iter()
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
        let results = component.root_element().query_descendants().match_id(id).find_all();
        results.into_iter()
    }

    /// This function searches through the entire tree of elements of `component`, looks for
    /// elements with given type name.
    pub fn find_by_element_type_name(
        component: &impl ElementRoot,
        type_name: &str,
    ) -> impl Iterator<Item = Self> {
        let results =
            component.root_element().query_descendants().match_inherits(type_name).find_all();
        results.into_iter()
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

    /// Returns the value of the element's `accessible-role` property, if present. Use this property to
    /// locate elements by their type/role, i.e. buttons, checkboxes, etc.
    pub fn accessible_role(&self) -> Option<crate::AccessibleRole> {
        self.item.upgrade().map(|item| item.accessible_role())
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

    /// Returns the value of the element's `accessible-placeholder-text` property, if present.
    pub fn accessible_placeholder_text(&self) -> Option<SharedString> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            item.accessible_string_property(AccessibleStringProperty::PlaceholderText)
        })
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

    /// Returns the value of the `accessible-enabled` property, if present
    pub fn accessible_enabled(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Enabled))
            .and_then(|item| item.parse().ok())
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

    /// Returns the value of the `accessible-item-selected` property, if present
    pub fn accessible_item_selected(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| {
                item.accessible_string_property(AccessibleStringProperty::ItemSelected)
            })
            .and_then(|item| item.parse().ok())
    }

    /// Returns the value of the `accessible-item-selectable` property, if present
    pub fn accessible_item_selectable(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| {
                item.accessible_string_property(AccessibleStringProperty::ItemSelectable)
            })
            .and_then(|item| item.parse().ok())
    }

    /// Returns the value of the element's `accessible-item-index` property, if present.
    pub fn accessible_item_index(&self) -> Option<usize> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            item.accessible_string_property(AccessibleStringProperty::ItemIndex)
                .and_then(|s| s.parse().ok())
        })
    }

    /// Returns the value of the element's `accessible-item-count` property, if present.
    pub fn accessible_item_count(&self) -> Option<usize> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            item.accessible_string_property(AccessibleStringProperty::ItemCount)
                .and_then(|s| s.parse().ok())
        })
    }

    /// Returns the value of the `accessible-expanded` property, if present
    pub fn accessible_expanded(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Expanded))
            .and_then(|item| item.parse().ok())
    }

    /// Returns the value of the `accessible-expandable` property, if present
    pub fn accessible_expandable(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Expandable))
            .and_then(|item| item.parse().ok())
    }

    /// Returns the value of the `accessible-read-only` property, if present
    pub fn accessible_read_only(&self) -> Option<bool> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::ReadOnly))
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

    /// Returns the opacity that is applied when rendering this element. This is the product of
    /// the opacity property multiplied with any opacity specified by parent elements. Returns zero
    /// if the element is not valid.
    pub fn computed_opacity(&self) -> f32 {
        self.item
            .upgrade()
            .map(|mut item| {
                let mut opacity = 1.0;
                while let Some(parent) = item.parent_item(ParentItemTraversalMode::StopAtPopups) {
                    if let Some(opacity_item) =
                        i_slint_core::items::ItemRef::downcast_pin::<Opacity>(item.borrow())
                    {
                        opacity *= opacity_item.opacity();
                    }
                    item = parent.clone();
                }
                opacity
            })
            .unwrap_or(0.0)
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

    /// Invokes the element's `accessible-action-expand` callback, if declared. On widgets such as combo boxes, this
    /// typically discloses the list of available choices.
    pub fn invoke_accessible_expand_action(&self) {
        if self.element_index != 0 {
            return;
        }
        if let Some(item) = self.item.upgrade() {
            item.accessible_action(&AccessibilityAction::Expand)
        }
    }

    /// Simulates a single click (or touch tap) on the element at its center point with the
    /// specified button.
    pub async fn single_click(&self, button: i_slint_core::platform::PointerEventButton) {
        let Some(item) = self.item.upgrade() else { return };
        let Some(window_adapter) = item.window_adapter() else { return };
        let window = window_adapter.window();

        let item_pos = self.absolute_position();
        let item_size = self.size();
        let position = LogicalPosition::new(
            item_pos.x + item_size.width / 2.,
            item_pos.y + item_size.height / 2.,
        );

        window.dispatch_event(i_slint_core::platform::WindowEvent::PointerMoved { position });
        window.dispatch_event(i_slint_core::platform::WindowEvent::PointerPressed {
            position,
            button,
        });

        wait_for(std::time::Duration::from_millis(50)).await;

        window_adapter.window().dispatch_event(
            i_slint_core::platform::WindowEvent::PointerReleased { position, button },
        );
    }

    /// Simulates a double click (or touch tap) on the element at its center point.
    pub async fn double_click(&self, button: i_slint_core::platform::PointerEventButton) {
        let Ok(click_interval) = i_slint_core::with_global_context(
            || Err(i_slint_core::platform::PlatformError::NoPlatform),
            |ctx| ctx.platform().click_interval(),
        ) else {
            return;
        };
        let Some(duration_recognized_as_double_click) =
            click_interval.checked_sub(std::time::Duration::from_millis(10))
        else {
            return;
        };

        let Some(single_click_duration) = duration_recognized_as_double_click.checked_div(2) else {
            return;
        };

        let Some(item) = self.item.upgrade() else { return };
        let Some(window_adapter) = item.window_adapter() else { return };
        let window = window_adapter.window();

        let item_pos = self.absolute_position();
        let item_size = self.size();
        let position = LogicalPosition::new(
            item_pos.x + item_size.width / 2.,
            item_pos.y + item_size.height / 2.,
        );

        window.dispatch_event(i_slint_core::platform::WindowEvent::PointerMoved { position });
        window.dispatch_event(i_slint_core::platform::WindowEvent::PointerPressed {
            position,
            button,
        });

        wait_for(single_click_duration).await;

        window.dispatch_event(i_slint_core::platform::WindowEvent::PointerReleased {
            position,
            button,
        });
        window.dispatch_event(i_slint_core::platform::WindowEvent::PointerPressed {
            position,
            button,
        });

        wait_for(single_click_duration).await;

        window_adapter.window().dispatch_event(
            i_slint_core::platform::WindowEvent::PointerReleased { position, button },
        );
    }

    fn active_popups(&self) -> Vec<(ItemRc, ItemTreeRc)> {
        self.item
            .upgrade()
            .and_then(|item| item.window_adapter())
            .map(|window_adapter| {
                let window = WindowInner::from_pub(window_adapter.window());
                window
                    .active_popups()
                    .iter()
                    .filter_map(|popup| {
                        Some((popup.parent_item.upgrade()?, popup.component.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

async fn wait_for(duration: std::time::Duration) {
    enum AsyncTimerState {
        Starting,
        Waiting(std::task::Waker),
        Done,
    }

    let state = std::rc::Rc::new(std::cell::RefCell::new(AsyncTimerState::Starting));

    std::future::poll_fn(move |context| {
        let mut current_state = state.borrow_mut();
        match *current_state {
            AsyncTimerState::Starting => {
                *current_state = AsyncTimerState::Waiting(context.waker().clone());
                let state_clone = state.clone();
                i_slint_core::timers::Timer::single_shot(duration, move || {
                    let mut current_state = state_clone.borrow_mut();
                    match *current_state {
                        AsyncTimerState::Starting => unreachable!(),
                        AsyncTimerState::Waiting(ref waker) => {
                            waker.wake_by_ref();
                            *current_state = AsyncTimerState::Done;
                        }
                        AsyncTimerState::Done => {}
                    }
                });

                std::task::Poll::Pending
            }
            AsyncTimerState::Waiting(ref existing_waker) => {
                let new_waker = context.waker();
                if !existing_waker.will_wake(new_waker) {
                    *current_state = AsyncTimerState::Waiting(new_waker.clone());
                }
                std::task::Poll::Pending
            }
            AsyncTimerState::Done => std::task::Poll::Ready(()),
        }
    })
    .await
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
            if condition: dynamic-elem := Rectangle {
                accessible-role: text;
            }
            visible-element := Rectangle {
                visible: !condition;
                inner-element := Text { text: "hello"; }
            }
        }
    }

    let app = App::new().unwrap();
    let mut it = ElementHandle::find_by_element_id(&app, "App::dynamic-elem");
    assert!(it.next().is_none());

    assert_eq!(ElementHandle::find_by_element_id(&app, "App::visible-element").count(), 1);
    assert_eq!(ElementHandle::find_by_element_id(&app, "App::inner-element").count(), 1);

    app.set_condition(true);

    it = ElementHandle::find_by_element_id(&app, "App::dynamic-elem");
    let elem = it.next().unwrap();
    assert!(it.next().is_none());

    assert_eq!(elem.id().unwrap(), "App::dynamic-elem");
    assert_eq!(elem.type_name().unwrap(), "Rectangle");
    assert_eq!(elem.bases().unwrap().count(), 0);
    assert_eq!(elem.accessible_role().unwrap(), crate::AccessibleRole::Text);

    assert_eq!(ElementHandle::find_by_element_id(&app, "App::visible-element").count(), 0);
    assert_eq!(ElementHandle::find_by_element_id(&app, "App::inner-element").count(), 0);

    app.set_condition(false);

    // traverse the item tree before testing elem.is_valid()
    assert!(ElementHandle::find_by_element_id(&app, "App::dynamic-elem").next().is_none());
    assert!(!elem.is_valid());

    assert_eq!(ElementHandle::find_by_element_id(&app, "App::visible-element").count(), 1);
    assert_eq!(ElementHandle::find_by_element_id(&app, "App::inner-element").count(), 1);
}

#[test]
fn test_matches() {
    crate::init_no_event_loop();

    slint::slint! {
        component Base inherits Rectangle {}

        export component App inherits Window {
            in property <bool> condition: false;
            if condition: dynamic-elem := Base {
                accessible-role: text;
            }
            visible-element := Rectangle {
                visible: !condition;
                inner-element := Text { text: "hello"; }
            }
        }
    }

    let app = App::new().unwrap();

    let root = app.root_element();

    assert_eq!(root.query_descendants().match_inherits("Rectangle").find_all().len(), 1);
    assert_eq!(root.query_descendants().match_inherits("Base").find_all().len(), 0);
    assert!(root.query_descendants().match_id("App::dynamic-elem").find_first().is_none());

    assert_eq!(root.query_descendants().match_id("App::visible-element").find_all().len(), 1);
    assert_eq!(root.query_descendants().match_id("App::inner-element").find_all().len(), 1);

    assert_eq!(
        root.query_descendants()
            .match_id("App::visible-element")
            .match_descendants()
            .match_accessible_role(crate::AccessibleRole::Text)
            .find_first()
            .and_then(|elem| elem.accessible_label())
            .unwrap_or_default(),
        "hello"
    );

    app.set_condition(true);

    assert!(root
        .query_descendants()
        .match_id("App::visible-element")
        .match_descendants()
        .match_accessible_role(crate::AccessibleRole::Text)
        .find_first()
        .is_none());

    let elems = root.query_descendants().match_id("App::dynamic-elem").find_all();
    assert_eq!(elems.len(), 1);
    let elem = &elems[0];

    assert_eq!(elem.id().unwrap(), "App::dynamic-elem");
    assert_eq!(elem.type_name().unwrap(), "Base");
    assert_eq!(elem.bases().unwrap().count(), 1);
    assert_eq!(elem.accessible_role().unwrap(), crate::AccessibleRole::Text);

    assert_eq!(root.query_descendants().match_inherits("Base").find_all().len(), 1);
}

#[test]
fn test_normalize_id() {
    crate::init_no_event_loop();

    slint::slint! {
        export component App inherits Window {
            the_element := Text {
                text: "Found me";
            }
        }
    }

    let app = App::new().unwrap();

    let root = app.root_element();

    assert_eq!(root.query_descendants().match_id("App::the-element").find_all().len(), 1);
    assert_eq!(root.query_descendants().match_id("App::the_element").find_all().len(), 1);
}

#[test]
fn test_opacity() {
    crate::init_no_event_loop();

    slint::slint! {
        export component App inherits Window {
            Rectangle {
                opacity: 0.5;
                translucent-label := Text {
                    opacity: 0.2;
                }
            }
            definitely-there := Text {}
        }
    }

    let app = App::new().unwrap();

    let root = app.root_element();

    use i_slint_core::graphics::euclid::approxeq::ApproxEq;

    assert!(root
        .query_descendants()
        .match_id("App::translucent-label")
        .find_first()
        .unwrap()
        .computed_opacity()
        .approx_eq(&0.1));
    assert!(root
        .query_descendants()
        .match_id("App::definitely-there")
        .find_first()
        .unwrap()
        .computed_opacity()
        .approx_eq(&1.0));
}

#[test]
fn test_popups() {
    crate::init_no_event_loop();

    slint::slint! {
        export component App inherits Window {
            popup := PopupWindow {
                close-policy: close-on-click-outside;
                Rectangle {
                    ok-label := Text {
                        accessible-role: text;
                        accessible-value: self.text;
                        text: "Ok";
                    }
                    ta := TouchArea {
                        clicked => {
                            another-popup.show();
                        }
                        accessible-role: button;
                        accessible-action-default => {
                            another-popup.show();
                        }
                    }
                    another-popup := PopupWindow {
                        inner-rect := Rectangle {
                            nested-label := Text {
                                accessible-role: text;
                                accessible-value: self.text;
                                text: "Nested";
                            }
                        }
                    }
                }
            }
            Rectangle {
            }
            first-button := TouchArea {
                clicked => {
                    popup.show();
                }
                accessible-role: button;
                accessible-action-default => {
                    popup.show();
                }
            }
        }
    }

    let app = App::new().unwrap();

    let root = app.root_element();

    assert!(root
        .query_descendants()
        .match_accessible_role(crate::AccessibleRole::Text)
        .find_all()
        .into_iter()
        .filter_map(|elem| elem.accessible_label())
        .collect::<Vec<_>>()
        .is_empty());

    root.query_descendants()
        .match_id("App::first-button")
        .find_first()
        .unwrap()
        .invoke_accessible_default_action();

    assert_eq!(
        root.query_descendants()
            .match_accessible_role(crate::AccessibleRole::Text)
            .find_all()
            .into_iter()
            .filter_map(|elem| elem.accessible_label())
            .collect::<Vec<_>>(),
        ["Ok"]
    );

    root.query_descendants()
        .match_id("App::ta")
        .find_first()
        .unwrap()
        .invoke_accessible_default_action();

    assert_eq!(
        root.query_descendants()
            .match_accessible_role(crate::AccessibleRole::Text)
            .find_all()
            .into_iter()
            .filter_map(|elem| elem.accessible_label())
            .collect::<Vec<_>>(),
        ["Nested", "Ok"]
    );
}
