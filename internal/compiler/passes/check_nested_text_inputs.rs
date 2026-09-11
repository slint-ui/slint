// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that warns about an accessible text input nested in another one.
//!
//! Both of them describe the text of the `TextInput` they contain,
//! which hands an assistive technology the same text under two elements.

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::Expression;
use crate::langtype::ElementType;
use crate::object_tree::{Component, Document, ElementRc};
use by_address::ByAddress;
use smol_str::SmolStr;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

/// The item above the one being visited that exposes a text input, and the role it does it with.
type Enclosing = (ElementRc, SmolStr);

/// What identifies the element a warning is about, to warn about it once.
/// Inlining copies an element per use site, and the copies share one source location.
#[derive(PartialEq, Eq, Hash)]
enum Reported {
    Source(PathBuf, usize),
    /// An element the compiler synthesized, which has no source of its own.
    Element(ByAddress<ElementRc>),
}

pub fn check_nested_text_inputs(doc: &Document, diag: &mut BuildDiagnostics) {
    let mut check = Check { diag, reported: HashSet::new() };
    for component in doc.exported_roots() {
        check.visit_component(&component, None);
    }
}

/// An accessible role whose item describes the text of the `TextInput` below it.
fn exposes_text_input(role: &SmolStr) -> bool {
    role == "text-input" || role == "spinbox"
}

/// The role the item ends up with, which is the one the most derived element sets: setting the
/// role where a component is used writes it to the root element of that component.
fn accessible_role(elem: &ElementRc) -> Option<SmolStr> {
    let mut level = Some(elem.clone());
    while let Some(e) = level {
        let role = e.borrow().binding("accessible-role").map(|binding| {
            match binding.value_expression() {
                // `none` is a role of its own, and leaves nothing for the bases to say
                Expression::EnumerationValue(value) if value.value != 0 => {
                    Some(value.enumeration.values[value.value].clone())
                }
                _ => None,
            }
        });
        if let Some(role) = role {
            return role;
        }
        level = base_component(&e).map(|base| base.root_element.clone());
    }
    None
}

fn base_component(elem: &ElementRc) -> Option<Rc<Component>> {
    match &elem.borrow().base_type {
        ElementType::Component(base) => Some(base.clone()),
        _ => None,
    }
}

struct Check<'a> {
    diag: &'a mut BuildDiagnostics,
    reported: HashSet<Reported>,
}

impl Check<'_> {
    fn visit_component(&mut self, component: &Rc<Component>, enclosing: Option<&Enclosing>) {
        self.visit_item(&component.root_element, enclosing);
        self.visit_popups(component);
    }

    /// A popup is a tree of its own:
    /// its items nest under the popup, not under the element that declares it.
    fn visit_popups(&mut self, component: &Component) {
        let popups = component
            .popup_windows
            .borrow()
            .iter()
            .map(|p| p.component.clone())
            .collect::<Vec<_>>();
        for popup in &popups {
            self.visit_component(popup, None);
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
            self.visit_popups(&base);
            level = Some(base.root_element.clone());
        }
    }

    fn report(&mut self, elem: &ElementRc, enclosing: &ElementRc, enclosing_role: &SmolStr) {
        self.diag.push_warning(
            format!(
                "This element and the '{enclosing_role}' around it expose the same text to a screen reader; set 'accessible-role: none' here"
            ),
            &*elem.borrow(),
        );
        self.diag.push_note("The enclosing element is declared here".into(), &*enclosing.borrow());
    }
}

fn reported(elem: &ElementRc) -> Reported {
    let location = elem.borrow().to_source_location();
    match location.source_file {
        Some(file) => Reported::Source(file.path().to_path_buf(), location.span.offset),
        None => Reported::Element(ByAddress(elem.clone())),
    }
}
