// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that warns about an accessible text input nested in another one.
//!
//! Both of them describe the text of the `TextInput` they contain,
//! which hands an assistive technology the same text under two elements.

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::element_type::ElementType;
use crate::expression_tree::Expression;
use crate::object_tree::{Component, Document, ElementRc};
use crate::source_path::SourcePath;
use smol_str::SmolStr;
use std::collections::HashSet;
use std::rc::Rc;

/// The item above the one being visited that exposes a text input, and the role it does it with.
type Enclosing = (ElementRc, SmolStr);

pub fn check_nested_text_inputs(doc: &Document, diag: &mut BuildDiagnostics) {
    let mut check = Check { diag, reported: HashSet::new(), visited_bases: HashSet::new() };
    for component in doc.exported_roots() {
        check.visit_component(&component);
    }
}

/// An accessible role whose item describes the text of the `TextInput` below it.
/// Keep in sync with `wraps_text_input` in the winit accesskit backend.
fn exposes_text_input(role: &SmolStr) -> bool {
    role == "text-input" || role == "spinbox"
}

/// The role the item ends up with, which is the one the most derived element sets: setting the
/// role where a component is used writes it to the root element of that component.
fn accessible_role(elem: &ElementRc) -> Option<SmolStr> {
    crate::layout::find_binding(elem, "accessible-role", |binding, _, _| {
        match binding.value_expression() {
            Expression::EnumerationValue(value) => {
                Some(value.enumeration.values[value.value].clone())
            }
            _ => None,
        }
    })
    .flatten()
}

fn base_component(elem: &ElementRc) -> Option<Rc<Component>> {
    match &elem.borrow().base_type {
        ElementType::Component(base) => Some(base.clone()),
        _ => None,
    }
}

struct Check<'a> {
    diag: &'a mut BuildDiagnostics,
    /// Source location of the elements warned about, to warn about each once:
    /// inlining copies an element per use site, and the copies share one source location.
    reported: HashSet<(Option<SourcePath>, usize)>,
    /// Base components already walked, with whether a text input enclosed them.
    /// A component used in several places is one `Rc`, and its subtree warns the same way each time.
    visited_bases: HashSet<(*const Component, bool)>,
}

impl Check<'_> {
    fn visit_component(&mut self, component: &Rc<Component>) {
        self.visit_item(&component.root_element, None);
        self.visit_popups(component);
    }

    /// A popup is a tree of its own:
    /// its items nest under the popup, not under the element that declares it.
    fn visit_popups(&mut self, component: &Component) {
        for popup in component.popup_windows.borrow().iter() {
            self.visit_component(&popup.component);
        }
    }

    /// Visits the item `elem` stands for, with `enclosing` the nearest item above it that exposes
    /// a text input.
    fn visit_item(&mut self, elem: &ElementRc, enclosing: Option<&Enclosing>) {
        let role = accessible_role(elem).filter(exposes_text_input);
        if role.is_some()
            && let Some((enclosing, enclosing_role)) = enclosing
            && self.reported.insert(reported(elem))
        {
            self.report(elem, enclosing, enclosing_role);
        }
        let here = role.map(|role| (elem.clone(), role));
        let enclosing = here.as_ref().or(enclosing);

        // An element instantiating a component is one item together with that component's root,
        // so the item's children are the element's own plus those of every component it inherits.
        let mut level = Some(elem.clone());
        while let Some(e) = level {
            let children = e.borrow().children.clone();
            for child in &children {
                self.visit_item(child, enclosing);
            }
            let Some(base) = base_component(&e) else { break };
            if !self.visited_bases.insert((Rc::as_ptr(&base), enclosing.is_some())) {
                break;
            }
            self.visit_popups(&base);
            level = Some(base.root_element.clone());
        }
    }

    fn report(&mut self, elem: &ElementRc, enclosing: &ElementRc, enclosing_role: &SmolStr) {
        let enclosing_name = element_name(enclosing);
        self.diag.push_warning(
            format!(
                "This element exposes the same text to a screen reader as the enclosing '{enclosing_name}' with 'accessible-role: {enclosing_role}'; set 'accessible-role: none' here"
            ),
            &*elem.borrow(),
        );
        self.diag.push_note("The enclosing element is declared here".into(), &*enclosing.borrow());
    }
}

/// Names an element the way its source declares it, such as `foo := Rectangle`.
/// The type comes from the source text: `base_type` and the debug info's `type_name` hold the
/// component's own name, not the alias an `import { A as B }` gave it.
fn element_name(elem: &ElementRc) -> String {
    let elem = elem.borrow();
    let type_name = elem.debug.first().and_then(|info| info.node.QualifiedName()).map_or_else(
        || elem.base_type.to_string(),
        |name| name.text().to_string().trim().to_string(),
    );
    // `root` is the id the compiler gives a component's root element, not one the source wrote.
    if elem.id.is_empty() || elem.id == "root" {
        type_name
    } else {
        format!("{} := {type_name}", elem.id)
    }
}

fn reported(elem: &ElementRc) -> (Option<SourcePath>, usize) {
    let location = elem.borrow().to_source_location();
    (location.source_file.map(|file| file.path().clone()), location.span.offset)
}
