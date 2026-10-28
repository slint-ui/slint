// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::ops::ControlFlow;
use i_slint_core::accessibility::{AccessibilityAction, AccessibleStringProperty};
use i_slint_core::api::{ComponentHandle, LogicalPosition};
use i_slint_core::item_tree::{ItemTreeRc, ItemWeak, ParentItemTraversalMode};
use i_slint_core::items::{Flickable, ItemRc, Opacity, PointerEventButton, ScrollBehavior};
use i_slint_core::lengths::LogicalLength;
use i_slint_core::platform::WindowEvent;
use i_slint_core::window::WindowInner;
use i_slint_core::{Coord, SharedString};
use std::rc::Rc;
use std::time::Duration;

/// Delay in milliseconds between interpolated PointerMoved events during drag simulation
/// (~one frame at 60 fps).
const DRAG_STEP_DELAY_MS: u64 = 16;

/// Distance in logical pixels between interpolated PointerMoved events during drag
/// simulation. Chosen to be below the 8 px `DISTANCE_THRESHOLD` (see `flickable.rs`)
/// so that every intermediate position is reported to the element.
const DRAG_STEP_SIZE: f32 = 5.0;

/// Synthesizes a drag from `start` to `end` against `window`: an initial
/// `PointerMoved`+`PointerPressed`, interpolated `PointerMoved`s sized below the 8 px drag
/// threshold (with `mock_elapsed_time` between each), and a final `PointerReleased`.
///
/// Exposed at module scope so that callers that don't have an `ElementHandle` (e.g. tests
/// that drive raw window coordinates) can drive the same gesture as
/// [`ElementHandle::mock_drag`].
pub(crate) fn mock_drag_window(
    window: &i_slint_core::api::Window,
    start: LogicalPosition,
    end: LogicalPosition,
    button: PointerEventButton,
) {
    window.dispatch_event(WindowEvent::PointerMoved { position: start });
    window.dispatch_event(WindowEvent::PointerPressed { position: start, button });

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance > f32::EPSILON {
        let steps = ((distance / DRAG_STEP_SIZE).ceil() as usize).max(2);

        for i in 1..steps {
            let t = i as f32 / steps as f32;
            let pos = LogicalPosition::new(start.x + dx * t, start.y + dy * t);
            crate::testing_backend::mock_elapsed_time(DRAG_STEP_DELAY_MS);
            window.dispatch_event(WindowEvent::PointerMoved { position: pos });
        }

        crate::testing_backend::mock_elapsed_time(DRAG_STEP_DELAY_MS);
        window.dispatch_event(WindowEvent::PointerMoved { position: end });
    }

    window.dispatch_event(WindowEvent::PointerReleased { position: end, button });
}

/// Shown when the application was built without element debug info.
pub(crate) const MISSING_DEBUG_INFO_MESSAGE: &str = "The use of the ElementHandle API requires the presence of debug info in Slint compiler generated code. Set the `SLINT_EMIT_DEBUG_INFO=1` environment variable at application build time or use `compile_with_config` and `with_debug_info` with `slint_build`'s `CompilerConfiguration`";

fn warn_missing_debug_info() {
    i_slint_core::debug_log!("{}", MISSING_DEBUG_INFO_MESSAGE)
}

mod internal {
    /// Used as base of another trait so it cannot be re-implemented
    pub trait Sealed {}
}

/// Describes the kind of layout an element represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum LayoutKind {
    /// A `HorizontalLayout`.
    HorizontalLayout,
    /// A `VerticalLayout`.
    VerticalLayout,
    /// A `GridLayout`.
    GridLayout,
    /// A flex box layout.
    FlexboxLayout,
}

impl LayoutKind {
    fn from_encoded(s: &str) -> Option<Self> {
        match s {
            "h-box" => Some(Self::HorizontalLayout),
            "v-box" => Some(Self::VerticalLayout),
            "grid" => Some(Self::GridLayout),
            "flex-box" => Some(Self::FlexboxLayout),
            _ => None,
        }
    }
}

pub(crate) use internal::Sealed;

/// Trait for type that can be searched for element. This is implemented for everything that implements [`ComponentHandle`]
pub trait ElementRoot: Sealed {
    #[doc(hidden)]
    fn item_tree(&self) -> ItemTreeRc;
    /// Returns the root of the element tree.
    fn root_element(&self) -> ElementHandle {
        let item_rc = ItemRc::new_root(self.item_tree());
        ElementHandle { item: item_rc.downgrade(), element_index: 0 }
    }
}

impl<T: ComponentHandle> ElementRoot for T {
    fn item_tree(&self) -> ItemTreeRc {
        WindowInner::from_pub(self.window()).component()
    }
}

impl<T: ComponentHandle> Sealed for T {}

/// How an [`ElementQuery`] compares one of an element's strings with the expected text.
///
/// A string converts into [`TextMatch::Exact`],
/// so the `match_*` functions take either a plain string or a `TextMatch`:
///
/// ```rust
/// # i_slint_backend_testing::init_no_event_loop();
/// # slint::slint!{ export component App { Text { text: "Save File"; } } }
/// # let app = App::new().unwrap();
/// use i_slint_backend_testing::{ElementRoot, TextMatch};
/// let root = app.root_element();
/// assert!(root.query_descendants().match_accessible_label("Save File").find_first().is_some());
/// assert!(root
///     .query_descendants()
///     .match_accessible_label(TextMatch::ContainsIgnoringCase("save".into()))
///     .find_first()
///     .is_some());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextMatch {
    /// Matches a string equal to the text.
    Exact(SharedString),
    /// Matches a string that contains the text, ignoring case.
    ContainsIgnoringCase(SharedString),
}

impl From<&str> for TextMatch {
    fn from(text: &str) -> Self {
        Self::Exact(text.into())
    }
}

impl From<&String> for TextMatch {
    fn from(text: &String) -> Self {
        Self::Exact(text.into())
    }
}

impl From<String> for TextMatch {
    fn from(text: String) -> Self {
        Self::Exact(text.into())
    }
}

impl From<&SharedString> for TextMatch {
    fn from(text: &SharedString) -> Self {
        Self::Exact(text.clone())
    }
}

impl From<SharedString> for TextMatch {
    fn from(text: SharedString) -> Self {
        Self::Exact(text)
    }
}

enum TextMatcher {
    Exact(SharedString),
    ContainsFolded(String),
}

impl From<TextMatch> for TextMatcher {
    fn from(text: TextMatch) -> Self {
        match text {
            TextMatch::Exact(text) => Self::Exact(text),
            TextMatch::ContainsIgnoringCase(text) => Self::ContainsFolded(fold_case(&text)),
        }
    }
}

impl TextMatcher {
    fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact(text) => candidate == text.as_str(),
            Self::ContainsFolded(text) => fold_case(candidate).contains(text.as_str()),
        }
    }
}

// Approximates Unicode case folding. `str::to_lowercase` keeps `ß` apart from `SS`,
// and turns a word-final `Σ` into `ς` rather than `σ`.
fn fold_case(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}

#[allow(clippy::enum_variant_names)]
enum SingleElementMatch {
    MatchById { id: String, root_base: Option<String> },
    MatchByTypeName(String),
    MatchByTypeNameOrBase(String),
    MatchByAccessibleRole(crate::AccessibleRole),
    MatchByString { read: fn(&ElementHandle) -> Option<SharedString>, text: TextMatcher },
    MatchByTestableProperty { name: SharedString, value: TextMatcher },
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
            SingleElementMatch::MatchByString { read, text } => {
                read(element).is_some_and(|candidate| text.matches(&candidate))
            }
            SingleElementMatch::MatchByTestableProperty { name, value } => element
                .testable_property_value(name)
                .is_some_and(|candidate| value.matches(&candidate)),
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
        is_listed: fn(&ItemRc) -> bool,
    ) -> (ControlFlow<()>, Vec<ElementHandle>) {
        let Some((query, tail)) = query_stack.split_first() else {
            return (control_flow_after_first_match, vec![element]);
        };

        match query {
            ElementQueryInstruction::MatchDescendants => {
                let mut results = Vec::new();
                match element.visit_descendants_impl(
                    &mut |child| {
                        let (next_control_flow, sub_results) = Self::match_recursively(
                            tail,
                            child,
                            control_flow_after_first_match,
                            active_popups,
                            is_listed,
                        );
                        results.extend(sub_results);
                        next_control_flow
                    },
                    active_popups,
                    is_listed,
                ) {
                    Some(_) => (ControlFlow::Break(()), results),
                    None => (ControlFlow::Continue(()), results),
                }
            }
            ElementQueryInstruction::MatchSingleElement(criteria) => {
                let mut results = Vec::new();
                let control_flow = if criteria.matches(&element) {
                    let (next_control_flow, sub_results) = Self::match_recursively(
                        tail,
                        element,
                        control_flow_after_first_match,
                        active_popups,
                        is_listed,
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

    /// Include only elements in the results where [`ElementHandle::accessible_label()`] matches `label`.
    pub fn match_accessible_label(self, label: impl Into<TextMatch>) -> Self {
        self.match_string(ElementHandle::accessible_label, label.into())
    }

    /// Include only elements in the results where [`ElementHandle::accessible_value()`] matches `value`.
    pub fn match_accessible_value(self, value: impl Into<TextMatch>) -> Self {
        self.match_string(ElementHandle::accessible_value, value.into())
    }

    /// Include only elements in the results where [`ElementHandle::accessible_description()`] matches `description`.
    pub fn match_accessible_description(self, description: impl Into<TextMatch>) -> Self {
        self.match_string(ElementHandle::accessible_description, description.into())
    }

    /// Include only elements in the results where [`ElementHandle::accessible_placeholder_text()`] matches `text`.
    pub fn match_accessible_placeholder_text(self, text: impl Into<TextMatch>) -> Self {
        self.match_string(ElementHandle::accessible_placeholder_text, text.into())
    }

    /// Include only elements in the results where [`ElementHandle::accessible_id()`] matches `id`.
    pub fn match_accessible_id(self, id: impl Into<TextMatch>) -> Self {
        self.match_string(ElementHandle::accessible_id, id.into())
    }

    /// Include only elements in the results that have a `@testable` property `name`
    /// whose [`ElementHandle::testable_property_value()`] matches `value`.
    ///
    /// ```rust
    /// # i_slint_backend_testing::init_no_event_loop();
    /// slint::slint!{
    ///
    /// component Item {
    ///     @testable in property <bool> selected;
    /// }
    ///
    /// export component App {
    ///     Item { }
    ///     Item { selected: true; }
    /// }
    ///
    /// }
    ///
    /// let app = App::new().unwrap();
    /// use i_slint_backend_testing::ElementRoot;
    /// let selected =
    ///     app.root_element().query_descendants().match_testable_property("selected", "true");
    /// assert_eq!(selected.find_all().len(), 1);
    /// ```
    pub fn match_testable_property(
        mut self,
        name: impl Into<SharedString>,
        value: impl Into<TextMatch>,
    ) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchByTestableProperty {
                name: name.into(),
                value: TextMatcher::from(value.into()),
            },
        ));
        self
    }

    fn match_string(
        mut self,
        read: fn(&ElementHandle) -> Option<SharedString>,
        text: TextMatch,
    ) -> Self {
        self.query_stack.push(ElementQueryInstruction::MatchSingleElement(
            SingleElementMatch::MatchByString { read, text: text.into() },
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
        self.run(ControlFlow::Break(()), ItemRc::is_visible).into_iter().next()
    }

    /// Runs the query and returns a vector of all matching elements.
    pub fn find_all(&self) -> Vec<ElementHandle> {
        self.run(ControlFlow::Continue(()), ItemRc::is_visible)
    }

    /// Runs the query like [`Self::find_first()`], but also over elements that scrolling reveals,
    /// and scrolls the first match into view with [`ElementHandle::scroll_into_view()`].
    ///
    /// The search covers elements that a `Flickable`, such as a `ScrollView`, has scrolled out of view.
    /// A `ListView` creates only the rows it shows,
    /// so the search then pages through each `Flickable` at or below the query's root
    /// that holds a `for` or `if`, from its top, until an element matches.
    /// Nested lists are searched on each page before the outer list advances.
    /// It gives up on a `Flickable` after 10,000 pages, such as one that grows as it's paged.
    /// Paging recreates a `ListView`'s rows, which invalidates earlier handles to them.
    /// Like scrolling, paging replaces a binding on the `Flickable`'s `content-y`.
    ///
    /// Returns None if no element matches, restoring each surviving `Flickable`'s position.
    /// Recreated rows lose state stored in those rows, including nested scroll positions.
    /// Also returns None if scrolling can't reveal the first match,
    /// such as one outside its `Flickable`'s `content-height`.
    ///
    /// ```rust
    /// # i_slint_backend_testing::init_no_event_loop();
    /// slint::slint!{
    ///
    /// import { ListView } from "std-widgets.slint";
    ///
    /// export component App inherits Window {
    ///     height: 100px;
    ///     ListView {
    ///         for row in 1000: Text { text: "Row " + row; height: 20px; }
    ///     }
    /// }
    ///
    /// }
    ///
    /// let app = App::new().unwrap();
    /// use i_slint_backend_testing::ElementRoot;
    /// let row = app.root_element().query_descendants().match_accessible_label("Row 500");
    /// assert!(row.find_first().is_none());
    /// assert!(row.scroll_into_view().is_some());
    /// ```
    pub fn scroll_into_view(&self) -> Option<ElementHandle> {
        let find = || {
            self.run(ControlFlow::Break(()), ItemRc::is_visible_or_clipped_by_flickable)
                .into_iter()
                .next()
        };
        let found = find().or_else(|| {
            let window_adapter = self.root.window_adapter()?;
            let window = WindowInner::from_pub(window_adapter.window());
            self.root.find_by_paging(window, &find, true)
        })?;
        found.scroll_into_view();
        found.item.upgrade().is_some_and(|item| item.is_visible()).then_some(found)
    }

    fn run(
        &self,
        control_flow_after_first_match: ControlFlow<()>,
        is_listed: fn(&ItemRc) -> bool,
    ) -> Vec<ElementHandle> {
        ElementQueryInstruction::match_recursively(
            &self.query_stack,
            self.root.clone(),
            control_flow_after_first_match,
            &self.root.active_popups(),
            is_listed,
        )
        .1
    }
}

/// Only a `for`, an `if` or a `ComponentContainer` puts a descendant in another item tree.
fn holds_repeated_elements(item: &ItemRc) -> bool {
    item.visit_descendants(|descendant| {
        if vtable::VRc::ptr_eq(descendant.item_tree(), item.item_tree()) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    })
    .is_some()
}

const MAX_PAGES: usize = 10_000;

fn page_until_found(
    flickable: &ItemRc,
    window: &WindowInner,
    find: &dyn Fn() -> Option<ElementHandle>,
) -> Option<ElementHandle> {
    let flick = flickable.downcast::<Flickable>()?;
    let flick = flick.as_pin_ref();
    let content_y = Flickable::FIELD_OFFSETS.content_y().apply_pin(flick);
    let original = content_y.get();
    let root = ElementHandle { item: flickable.downgrade(), element_index: 0 };
    let mut offset = 0 as Coord;
    let mut previous_page = None;
    for _ in 0..MAX_PAGES {
        content_y.set(LogicalLength::new(-offset));
        window.ensure_tree_instantiated();
        if let Some(found) = find().or_else(|| root.find_by_paging(window, find, false)) {
            return Some(found);
        }
        let reached = -content_y.get().get();
        let page_height = flickable.geometry().height();
        let end = (flick.content_height().get() - page_height).max(0 as Coord);
        let next = (reached + page_height).min(end);
        if next <= reached || previous_page == Some((offset, reached)) {
            break;
        }
        previous_page = Some((offset, reached));
        offset = next;
    }
    content_y.set(original);
    window.ensure_tree_instantiated();
    None
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

/// Overrides scrolling behavior for Slint's internal tests.
/// Panics if the handle has expired or does not refer to a Flickable.
#[cfg(feature = "internal")]
pub fn set_flickable_physics(
    element: &ElementHandle,
    bounce: i_slint_core::items::AutoBool,
    carry_momentum: i_slint_core::items::AutoBool,
) {
    let item = element.item.upgrade().expect("Flickable element has expired");
    let flickable = item.downcast::<Flickable>().expect("Element is not a Flickable");
    flickable.as_pin_ref().set_physics(bounce, carry_momentum);
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
        self.visit_descendants_impl(&mut |e| visitor(e), &self.active_popups(), ItemRc::is_visible)
    }

    /// Visit all descendants of this element and call the visitor to each of them, until the visitor returns [`ControlFlow::Break`].
    /// When the visitor breaks, the function returns the value. If it doesn't break, the function returns None.
    fn visit_descendants_impl<R>(
        &self,
        visitor: &mut dyn FnMut(ElementHandle) -> ControlFlow<R>,
        active_popups: &[(ItemRc, ItemTreeRc)],
        is_listed: fn(&ItemRc) -> bool,
    ) -> Option<R> {
        let self_item = self.item.upgrade()?;

        let visit_attached_popups =
            |item_rc: &ItemRc, visitor: &mut dyn FnMut(ElementHandle) -> ControlFlow<R>| {
                for (popup_elem, popup_item_tree) in active_popups {
                    if popup_elem == item_rc
                        && let Some(result) = (ElementHandle {
                            item: ItemRc::new_root(popup_item_tree.clone()).downgrade(),
                            element_index: 0,
                        })
                        .visit_descendants_impl(
                            visitor,
                            active_popups,
                            is_listed,
                        )
                    {
                        return Some(result);
                    }
                }
                None
            };

        if let Some(result) = visit_attached_popups(&self_item, visitor) {
            return Some(result);
        }

        self_item.visit_descendants(move |item_rc| {
            if !is_listed(item_rc) {
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

    /// Returns whether both handles refer to the same element of the same item.
    #[cfg(any(feature = "system-testing", feature = "mcp"))]
    pub(crate) fn is_same_element(&self, other: &ElementHandle) -> bool {
        self.item == other.item && self.element_index == other.element_index
    }

    /// Returns a hashable value that equal elements share, for use as a lookup key.
    /// Elements of different item trees can collide, so compare candidates with
    /// [`Self::is_same_element`]. Returns None once the element is gone.
    #[cfg(any(feature = "system-testing", feature = "mcp"))]
    pub(crate) fn identity_hint(&self) -> Option<(u32, usize)> {
        self.item.upgrade().map(|item| (item.index(), self.element_index))
    }

    /// Steps up the tree that [`Self::visit_descendants_impl`] walks with the same `active_popups`.
    #[cfg(feature = "mcp")]
    pub(crate) fn parent_element(
        &self,
        active_popups: &[(ItemRc, ItemTreeRc)],
    ) -> Option<ElementHandle> {
        let item = self.item.upgrade()?;
        let parent = item.parent_item(ParentItemTraversalMode::StopAtPopups).or_else(|| {
            active_popups
                .iter()
                .find(|(_, popup)| vtable::VRc::ptr_eq(popup, item.item_tree()))
                .map(|(attached_to, _)| attached_to.clone())
        })?;
        Some(ElementHandle { item: parent.downgrade(), element_index: 0 })
    }

    /// Returns whether the compiler emitted the debug info that type names, ids and
    /// descendant traversal need. See `MISSING_DEBUG_INFO_MESSAGE`.
    #[cfg(any(feature = "system-testing", feature = "mcp"))]
    pub(crate) fn has_debug_info(&self) -> bool {
        self.item.upgrade().is_some_and(|item| item.element_count().is_some())
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
        component
            .root_element()
            .query_descendants()
            .match_accessible_label(label)
            .find_all()
            .into_iter()
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
    /// #[slint_edition = "1.19"]
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
    /// #[slint_edition = "1.19"]
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
    /// #[slint_edition = "1.19"]
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
                        if !type_name.is_empty() { Some(type_name) } else { None }
                    },
                )
        })
    }

    /// Returns the layout kind if this element is a layout container;
    /// None if the element is not a layout or is not valid anymore.
    pub fn layout_kind(&self) -> Option<LayoutKind> {
        self.item.upgrade().and_then(|item| {
            item.element_layout_kind(self.element_index)
                .and_then(|s| LayoutKind::from_encoded(s.as_str()))
        })
    }

    /// Returns the value of the element's `accessible-role` property, if present. Use this property to
    /// locate elements by their type/role, i.e. buttons, checkboxes, etc.
    pub fn accessible_role(&self) -> Option<crate::AccessibleRole> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().map(|item| item.accessible_role())
    }

    /// Returns the element's `@testable` properties, including inherited ones,
    /// as `(name, type)` pairs.
    /// Only properties declared `@testable` are listed, whatever their visibility;
    /// they are kept alive for this purpose even when nothing else reads them.
    ///
    /// Returns `None` when the element is not valid anymore,
    /// or when the application was built without debug info (see [`Self::id`]);
    /// an element that declares no `@testable` property reports `Some` with an empty vector.
    pub fn testable_properties(&self) -> Option<Vec<(SharedString, SharedString)>> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| {
            let result = item.element_testable_properties();
            if result.is_none() {
                warn_missing_debug_info();
            }
            result
        })
    }

    /// Returns the current value of the testable property `name` on this element,
    /// encoded as a string:
    /// booleans as `true`/`false`,
    /// numbers in decimal (`length` in logical pixels, `duration` in milliseconds, `angle` in degrees),
    /// colors and solid brushes as `#rrggbbaa`,
    /// and enums as the `.slint` spelling of the value.
    ///
    /// Returns `None` when the element is not valid anymore,
    /// the property does not exist (see [`Self::testable_properties`]),
    /// its value has no string encoding,
    /// or the application was built without debug info (see [`Self::id`]).
    pub fn testable_property_value(&self, name: &str) -> Option<SharedString> {
        if self.element_index != 0 {
            return None;
        }
        self.item.upgrade().and_then(|item| item.element_testable_property_value(name))
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

    /// Selects the text between two UTF-8 offsets, by invoking the element's
    /// `accessible-action-set-selection-offsets` callback. Note that you can only do this if that callback
    /// is declared in your Slint code.
    pub fn set_accessible_selection_offsets(&self, anchor: i32, focus: i32) {
        if self.element_index != 0 {
            return;
        }
        if let Some(item) = self.item.upgrade() {
            item.accessible_action(&AccessibilityAction::SetSelectionOffsets(anchor, focus))
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

    /// Returns the value of the `accessible-id` property, if present
    pub fn accessible_id(&self) -> Option<SharedString> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Id))
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

    /// Returns the value of the `accessible-orientation` property, if present.
    pub fn accessible_orientation(&self) -> Option<crate::Orientation> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Orientation))
            .and_then(|s| s.parse().ok())
    }

    /// Returns the value of the `accessible-live-region` property, if present.
    pub fn accessible_live_region(&self) -> Option<crate::AccessibleLiveness> {
        if self.element_index != 0 {
            return None;
        }
        self.item
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::LiveRegion))
            .and_then(|s| s.parse().ok())
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
            .map(i_slint_core::builtin_functions::item_absolute_position)
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

    pub(crate) fn window_adapter(&self) -> Option<Rc<dyn i_slint_core::window::WindowAdapter>> {
        self.item.upgrade().and_then(|item| item.window_adapter())
    }

    /// Move the mouse to the element center and press the pointer.
    fn pointer_pressed(&self, button: PointerEventButton) {
        let Some(window_adapter) = self.window_adapter() else {
            return;
        };
        let window = window_adapter.window();
        let position = self.absolute_center();

        window.dispatch_event(WindowEvent::PointerMoved { position });
        window.dispatch_event(WindowEvent::PointerPressed { position, button });
    }

    /// Move the mouse to the element center and release the pointer.
    fn pointer_released(&self, button: PointerEventButton) {
        let Some(window_adapter) = self.window_adapter() else {
            return;
        };
        let window = window_adapter.window();
        let position = self.absolute_center();

        window.dispatch_event(WindowEvent::PointerMoved { position });
        window.dispatch_event(WindowEvent::PointerReleased { position, button });
    }

    /// Simulates a single click (or touch tap) on the element at its center point with the
    /// specified button.
    pub async fn single_click(&self, button: PointerEventButton) {
        self.pointer_pressed(button);

        wait_for(Duration::from_millis(50)).await;

        self.pointer_released(button);
    }

    /// Simulates a single click (or touch tap) on the element at its center point with the
    /// specified button.
    ///
    /// Compared to [Self::single_click()], this function uses mock time instead
    /// of an actual timer, so that it can be used in our internal tests that do not have an event
    /// loop.
    pub fn mock_single_click(&self, button: PointerEventButton) {
        self.pointer_pressed(button);

        crate::testing_backend::mock_elapsed_time(50);

        self.pointer_released(button);
    }

    /// Simulates a double click (or touch tap) on the element at its center point.
    pub async fn double_click(&self, button: PointerEventButton) {
        let Ok(click_interval) =
            i_slint_core::with_existing_context(|ctx| ctx.platform().click_interval())
        else {
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

        self.pointer_pressed(button);

        wait_for(single_click_duration).await;

        self.pointer_released(button);
        self.pointer_pressed(button);

        wait_for(single_click_duration).await;

        self.pointer_released(button);
    }

    /// Simulates a drag gesture from the element's center to the given target position.
    ///
    /// The sequence is:
    /// 1. `PointerMoved` + `PointerPressed` at the element center
    /// 2. Interpolated `PointerMoved` events from center to `target` (step size ~5 logical
    ///    pixels, with a 16 ms delay between steps)
    /// 3. `PointerMoved` + `PointerReleased` at `target`
    ///
    /// The step size is chosen to be smaller than the 8 px drag/flick detection threshold
    /// so that intermediate positions are always reported.
    pub async fn drag(&self, target: LogicalPosition, button: PointerEventButton) {
        let Some(window_adapter) = self.window_adapter() else {
            return;
        };
        let window = window_adapter.window();
        let start = self.absolute_center();

        // Press at element center.
        window.dispatch_event(WindowEvent::PointerMoved { position: start });
        window.dispatch_event(WindowEvent::PointerPressed { position: start, button });

        // Interpolate intermediate moves.
        let dx = target.x - start.x;
        let dy = target.y - start.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance > f32::EPSILON {
            // At least 2 steps to guarantee the drag threshold is crossed.
            let steps = ((distance / DRAG_STEP_SIZE).ceil() as usize).max(2);

            for i in 1..steps {
                let t = i as f32 / steps as f32;
                let pos = LogicalPosition::new(start.x + dx * t, start.y + dy * t);
                wait_for(Duration::from_millis(DRAG_STEP_DELAY_MS)).await;
                window.dispatch_event(WindowEvent::PointerMoved { position: pos });
            }

            // Final move to exact target.
            wait_for(Duration::from_millis(DRAG_STEP_DELAY_MS)).await;
            window.dispatch_event(WindowEvent::PointerMoved { position: target });
        }

        window.dispatch_event(WindowEvent::PointerReleased { position: target, button });
    }

    /// Simulates a drag gesture from the element's center to the given target position.
    ///
    /// Compared to [Self::drag()], this function uses mock time instead
    /// of an actual timer, so that it can be used in internal tests without an event loop.
    pub fn mock_drag(&self, target: LogicalPosition, button: PointerEventButton) {
        let Some(window_adapter) = self.window_adapter() else {
            return;
        };
        mock_drag_window(window_adapter.window(), self.absolute_center(), target, button);
    }

    /// The center of the element in the coordinate system that input events are dispatched in.
    /// Unlike [`Self::absolute_position()`] this includes the location of an enclosing popup that's
    /// rendered inside the window, such as a menu.
    pub(crate) fn absolute_center(&self) -> LogicalPosition {
        let Some(item) = self.item.upgrade() else {
            return Default::default();
        };
        // Map the center rather than mapping the origin and adding a local half-extent,
        // which ignores any scale or rotation an ancestor applies (#13242).
        i_slint_core::lengths::logical_position_to_api(
            item.map_to_native_window(item.geometry().center()),
        )
    }

    /// Like [`Self::absolute_center()`], for the element's origin.
    #[cfg(feature = "mcp")]
    pub(crate) fn absolute_origin(&self) -> LogicalPosition {
        let Some(item) = self.item.upgrade() else {
            return Default::default();
        };
        i_slint_core::lengths::logical_position_to_api(
            item.map_to_native_window(item.geometry().origin),
        )
    }

    /// Returns true for a `TouchArea` or gesture handler with `enabled: false`.
    #[cfg(feature = "mcp")]
    pub(crate) fn input_handler_disabled(&self) -> bool {
        use i_slint_core::items::{
            ItemRef, ScaleRotateGestureHandler, SwipeGestureHandler, TouchArea,
        };
        let Some(item) = self.item.upgrade() else {
            return false;
        };
        ItemRef::downcast_pin::<TouchArea>(item.borrow()).is_some_and(|area| !area.enabled())
            || ItemRef::downcast_pin::<SwipeGestureHandler>(item.borrow())
                .is_some_and(|handler| !handler.enabled())
            || ItemRef::downcast_pin::<ScaleRotateGestureHandler>(item.borrow())
                .is_some_and(|handler| !handler.enabled())
    }

    pub fn scroll(&self, delta_x: f32, delta_y: f32) {
        let Some(window_adapter) = self.item.upgrade().and_then(|item| item.window_adapter())
        else {
            return;
        };
        let window = window_adapter.window();

        let center = self.absolute_center();
        window.dispatch_event(WindowEvent::PointerScrolled { position: center, delta_x, delta_y });
    }

    /// Scrolls the `Flickable`s that contain the element, such as a `ScrollView` or a `ListView`,
    /// as far as they can to make the element visible.
    ///
    /// Call it before clicking an element that's partially scrolled out of view.
    /// To find an element that's scrolled out of view, or a `ListView` row that doesn't exist yet,
    /// use [`ElementQuery::scroll_into_view()`].
    pub fn scroll_into_view(&self) {
        let Some(item) = self.item.upgrade() else {
            return;
        };
        let Some(window_adapter) = item.window_adapter() else {
            return;
        };
        let window = WindowInner::from_pub(window_adapter.window());
        let enclosing_flickables = std::iter::successors(
            item.parent_item(ParentItemTraversalMode::StopAtPopups),
            |parent| parent.parent_item(ParentItemTraversalMode::StopAtPopups),
        )
        .filter(|parent| parent.downcast::<Flickable>().is_some())
        .count();
        let mut position = self.absolute_position();
        for _ in 0..enclosing_flickables {
            item.try_scroll_into_visible(ScrollBehavior::Instant);
            window.ensure_tree_instantiated();
            let revealed_position = self.absolute_position();
            if revealed_position == position {
                break;
            }
            position = revealed_position;
        }
    }

    fn find_by_paging(
        &self,
        window: &WindowInner,
        find: &dyn Fn() -> Option<ElementHandle>,
        include_self: bool,
    ) -> Option<ElementHandle> {
        let mut paged = Vec::new();
        loop {
            let flickable = self.flickable_to_page(&paged, include_self)?;
            if let Some(found) = page_until_found(&flickable, window, find) {
                return Some(found);
            }
            paged.push(flickable.downgrade());
        }
    }

    fn flickable_to_page(&self, paged: &[ItemWeak], include_self: bool) -> Option<ItemRc> {
        let to_page = |item: &ItemRc| {
            item.downcast::<Flickable>().is_some()
                && !std::iter::successors(Some(item.clone()), |ancestor| {
                    ancestor.parent_item(ParentItemTraversalMode::StopAtPopups)
                })
                .any(|ancestor| paged.contains(&ancestor.downgrade()))
                && holds_repeated_elements(item)
        };
        let self_item = self.item.upgrade()?;
        if include_self && to_page(&self_item) {
            return Some(self_item);
        }
        self.visit_descendants_impl(
            &mut |element| match element.item.upgrade() {
                Some(item) if element.element_index == 0 && to_page(&item) => {
                    ControlFlow::Break(item)
                }
                _ => ControlFlow::Continue(()),
            },
            &self.active_popups(),
            ItemRc::is_visible_or_clipped_by_flickable,
        )
    }

    pub(crate) fn active_popups(&self) -> Vec<(ItemRc, ItemTreeRc)> {
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
        #[slint_edition = "1.19"]
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
        #[slint_edition = "1.19"]
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
        #[slint_edition = "1.19"]
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

    assert_eq!(root.query_descendants().match_accessible_label("hello").find_all().len(), 1);
    assert_eq!(root.query_descendants().match_accessible_label("hell").find_all().len(), 0);

    app.set_condition(true);

    assert!(
        root.query_descendants()
            .match_id("App::visible-element")
            .match_descendants()
            .match_accessible_role(crate::AccessibleRole::Text)
            .find_first()
            .is_none()
    );

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
fn test_text_matchers() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        component Row {
            in property <string> title;
            @testable in property <bool> selected;
            @testable in property <string> status: "Draft";
            accessible-role: list-item;
            accessible-label: title;
        }

        export component App inherits Window {
            Row { title: "Save File"; }
            Row { title: "Save As…"; selected: true; status: "Published"; }
            search := TextInput {
                accessible-placeholder-text: "Search files";
                accessible-description: "Filters the list below";
                accessible-id: "search-box";
                text: "report";
            }
        }
    }

    let app = App::new().unwrap();
    let root = app.root_element();
    let count = |query: ElementQuery| query.find_all().len();
    let search = || root.query_descendants().match_id("App::search");

    assert_eq!(count(root.query_descendants().match_accessible_label("Save")), 0);
    assert_eq!(
        count(
            root.query_descendants()
                .match_accessible_label(TextMatch::ContainsIgnoringCase("save".into()))
        ),
        2
    );
    assert_eq!(
        count(
            root.query_descendants()
                .match_accessible_label(TextMatch::ContainsIgnoringCase("AS…".into()))
        ),
        1
    );

    assert_eq!(count(search().match_accessible_value("report")), 1);
    assert_eq!(count(search().match_accessible_value("repo")), 0);
    assert_eq!(count(search().match_accessible_placeholder_text("Search files")), 1);
    assert_eq!(count(search().match_accessible_description("Filters the list below")), 1);
    assert_eq!(
        count(
            search().match_accessible_description(TextMatch::ContainsIgnoringCase("LIST".into()))
        ),
        1
    );
    assert_eq!(count(root.query_descendants().match_accessible_id("search-box")), 1);
    assert_eq!(count(root.query_descendants().match_accessible_id("search")), 0);

    assert_eq!(count(root.query_descendants().match_testable_property("selected", "true")), 1);
    assert_eq!(count(root.query_descendants().match_testable_property("selected", "false")), 1);
    assert_eq!(
        root.query_descendants()
            .match_testable_property("selected", "true")
            .find_first()
            .and_then(|row| row.accessible_label())
            .unwrap_or_default(),
        "Save As…"
    );
    assert_eq!(
        count(
            root.query_descendants().match_testable_property(
                "status",
                TextMatch::ContainsIgnoringCase("publish".into())
            )
        ),
        1
    );
    assert_eq!(
        count(
            root.query_descendants()
                .match_testable_property("missing", TextMatch::ContainsIgnoringCase("".into()))
        ),
        0
    );
}

#[test]
fn test_contains_ignoring_case_folds_case() {
    let contains = |candidate: &str, text: &str| {
        TextMatcher::from(TextMatch::ContainsIgnoringCase(text.into())).matches(candidate)
    };
    assert!(contains("STRASSE", "straße"));
    assert!(contains("Straße", "SS"));
    assert!(contains("ΟΔΟΣ", "Σ"));
    assert!(contains("ΟΔΟΣΑ", "οδος"));
    assert!(!contains("ΟΔΟΣ", "x"));
}

#[test]
fn test_normalize_id() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
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
        #[slint_edition = "1.19"]
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

    assert!(
        root.query_descendants()
            .match_id("App::translucent-label")
            .find_first()
            .unwrap()
            .computed_opacity()
            .approx_eq(&0.1)
    );
    assert!(
        root.query_descendants()
            .match_id("App::definitely-there")
            .find_first()
            .unwrap()
            .computed_opacity()
            .approx_eq(&1.0)
    );
}

#[test]
fn test_popups() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
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

    assert!(
        root.query_descendants()
            .match_accessible_role(crate::AccessibleRole::Text)
            .find_all()
            .into_iter()
            .filter_map(|elem| elem.accessible_label())
            .collect::<Vec<_>>()
            .is_empty()
    );

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

#[test]
fn test_visit_descendants_stops_in_popup_of_start_element() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            public function show-popup() { popup.show(); }
            popup := PopupWindow {
                in-popup := Rectangle { }
            }
            after-popup := Rectangle { }
        }
    }

    let app = App::new().unwrap();
    app.invoke_show_popup();

    let mut visited = 0;
    let found = app.root_element().visit_descendants(|element| {
        visited += 1;
        ControlFlow::Break(element.id())
    });
    assert_eq!(found, Some(Some("App::in-popup".into())));
    assert_eq!(visited, 1);
}

#[test]
fn test_drag_touch_area() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 200px;
            height: 200px;
            out property <int> move-count: 0;
            out property <float> last-x: 0;
            out property <float> last-y: 0;
            ta := TouchArea {
                width: 100%;
                height: 100%;
                moved => {
                    root.move-count += 1;
                    root.last-x = self.mouse-x / 1px;
                    root.last-y = self.mouse-y / 1px;
                }
            }
        }
    }

    let app = App::new().unwrap();
    let ta = ElementHandle::find_by_element_id(&app, "App::ta").next().unwrap();

    // Drag from center (100,100) to (150,100) — 50px horizontal drag.
    ta.mock_drag(LogicalPosition::new(150.0, 100.0), PointerEventButton::Left);

    assert!(app.get_move_count() > 0, "moved callback should have fired");
    // The last move should land at the target position (within the element, so
    // target minus element origin = 150 - 0 = 150).
    let last_x = app.get_last_x();
    assert!((last_x - 150.0).abs() < 1.0, "last mouse-x should be near 150, got {last_x}");
}

#[test]
fn test_drag_zero_distance() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            out property <bool> was-pressed: false;
            out property <bool> was-released: false;
            out property <int> move-count: 0;
            ta := TouchArea {
                width: 100%;
                height: 100%;
                pointer-event(e) => {
                    if e.kind == PointerEventKind.down {
                        root.was-pressed = true;
                    }
                    if e.kind == PointerEventKind.up {
                        root.was-released = true;
                    }
                }
                moved => {
                    root.move-count += 1;
                }
            }
        }
    }

    let app = App::new().unwrap();
    let ta = ElementHandle::find_by_element_id(&app, "App::ta").next().unwrap();

    // Drag to the element's own center — zero distance.
    let center = ta.absolute_position();
    let sz = ta.size();
    let target = LogicalPosition::new(center.x + sz.width / 2.0, center.y + sz.height / 2.0);
    ta.mock_drag(target, PointerEventButton::Left);

    assert!(app.get_was_pressed(), "press event should have fired");
    assert!(app.get_was_released(), "release event should have fired");
    // Zero-distance drag should skip interpolation — no moved events from the
    // drag itself (the initial PointerMoved before press doesn't trigger `moved`
    // because the button isn't down yet).
    assert_eq!(app.get_move_count(), 0, "no moved events expected for zero-distance drag");
}

#[test]
fn test_scroll_into_view_reveals_flickable_content() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in-out property <length> content-y <=> flick.content-y;
            flick := Flickable {
                content-height: 2000px;
                Text { y: 1000px; height: 20px; text: "Deep"; }
            }
        }
    }

    let app = App::new().unwrap();
    let deep = app.root_element().query_descendants().match_accessible_label("Deep");
    assert!(deep.find_first().is_none());

    let found = deep.scroll_into_view().expect("the scrolled-out element wasn't found");
    assert_eq!(found.accessible_label().unwrap(), "Deep");
    assert_eq!(app.get_content_y(), -920.);
    assert!(deep.find_first().is_some());

    app.set_content_y(0.);
    found.scroll_into_view();
    assert_eq!(app.get_content_y(), -920.);
}

#[test]
fn test_scroll_into_view_creates_list_view_rows() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        import { ListView } from "std-widgets.slint";
        export component App inherits Window {
            width: 100px;
            height: 100px;
            out property <length> content-y: list.content-y;
            list := ListView {
                for row in 1000: Text { text: "Row " + row; height: 20px; }
            }
        }
    }

    let app = App::new().unwrap();
    let row = |label: &str| app.root_element().query_descendants().match_accessible_label(label);
    assert!(row("Row 500").find_first().is_none());

    let found = row("Row 500").scroll_into_view().expect("paging didn't create the row");
    assert_eq!(found.accessible_label().unwrap(), "Row 500");
    assert!(row("Row 500").find_first().is_some());

    let content_y = app.get_content_y();
    assert!(row("Row 1000").scroll_into_view().is_none());
    assert_eq!(app.get_content_y(), content_y, "a search without a match moved the list");
    assert!(row("Row 500").find_first().is_some());
}

#[test]
fn test_scroll_into_view_reveals_list_view_row_in_flickable() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        import { ListView } from "std-widgets.slint";
        export component App inherits Window {
            width: 100px;
            height: 100px;
            Flickable {
                content-height: 1000px;
                ListView {
                    y: 0;
                    width: 100px;
                    height: 100px;
                    for row in 10: Text { text: "Row " + row; height: 90px; }
                }
            }
        }
    }

    let app = App::new().unwrap();
    let row = app.root_element().query_descendants().match_accessible_label("Row 1").find_first();
    let row = row.expect("the partially visible row wasn't found");

    row.scroll_into_view();
    let top = row.absolute_position().y;
    let bottom = top + row.size().height;
    assert!(top >= 0. && bottom <= 100., "the row spans {top}..{bottom}, outside the window");
}

#[test]
fn test_scroll_into_view_pages_list_views_in_recreated_rows() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        import { ListView } from "std-widgets.slint";
        export component App inherits Window {
            width: 100px;
            height: 100px;
            out property <length> content-y: list.content-y;
            list := ListView {
                for outer in 12: ListView {
                    height: 50px;
                    for inner in 60: Text { text: "Row " + outer + "." + inner; height: 10px; }
                }
            }
        }
    }

    let app = App::new().unwrap();
    let flickable =
        app.root_element().query_descendants().match_inherits("Flickable").find_first().unwrap();
    for root in [app.root_element(), flickable] {
        for label in ["Row 0.50", "Row 10.50", "Row 11.59", "Row 0.59", "Row 10.59"] {
            let row = root.query_descendants().match_accessible_label(label);
            let found = row.scroll_into_view().expect("the nested list wasn't paged");
            assert_eq!(found.accessible_label().unwrap(), label);
            assert!(row.find_first().is_some());
        }
        let content_y = app.get_content_y();
        assert!(content_y < 0.);
        let missing = root.query_descendants().match_accessible_label("Missing");
        assert!(missing.scroll_into_view().is_none());
        assert_eq!(app.get_content_y(), content_y);
    }
}

#[test]
fn test_scroll_into_view_keeps_content_y_binding_without_repeated_elements() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <length> scrolled;
            out property <length> content-y: flick.content-y;
            flick := Flickable {
                content-height: 1000px;
                content-y: -root.scrolled;
                Text { height: 20px; text: "Top"; }
            }
        }
    }

    let app = App::new().unwrap();
    let missing = app.root_element().query_descendants().match_accessible_label("Missing");
    assert!(missing.scroll_into_view().is_none());
    app.set_scrolled(50.);
    assert_eq!(app.get_content_y(), -50., "paging replaced the binding on content-y");
}

#[test]
fn test_scroll_into_view_stops_paging_a_growing_flickable() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            out property <length> content-y: flick.content-y;
            flick := Flickable {
                content-height: 200px - self.content-y;
                for row in 1: Text { height: 20px; text: "Row " + row; }
            }
        }
    }

    let app = App::new().unwrap();
    let missing = app.root_element().query_descendants().match_accessible_label("Missing");
    assert!(missing.scroll_into_view().is_none());
    assert_eq!(app.get_content_y(), 0.);
}

#[test]
fn test_scroll_into_view_reports_a_match_it_cant_reveal() {
    crate::init_no_event_loop();

    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            Flickable {
                content-height: 500px;
                Text { y: 1000px; height: 20px; text: "Outside"; }
            }
        }
    }

    let app = App::new().unwrap();
    let outside = app.root_element().query_descendants().match_accessible_label("Outside");
    assert!(outside.scroll_into_view().is_none());
}
