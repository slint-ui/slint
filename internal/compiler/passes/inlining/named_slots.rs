// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{Mapping, element_key};
use crate::object_tree::{ElementRc, OptimizedElement, recurse_elem};
use by_address::ByAddress;
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct RemovedDefaults {
    elements: HashSet<ByAddress<ElementRc>>,
    replaced_slots: HashSet<SmolStr>,
    kept_slots: HashMap<SmolStr, Option<SmolStr>>,
}

impl RemovedDefaults {
    pub(super) fn contains_original(&self, mapping: &Mapping, original: &ElementRc) -> bool {
        mapping
            .get(&element_key(original.clone()))
            .is_some_and(|copy| self.elements.contains(&element_key(copy.clone())))
    }

    pub(super) fn is_replaced_default(&self, element: &ElementRc) -> bool {
        element
            .borrow()
            .slot_default
            .as_ref()
            .is_some_and(|slot| self.replaced_slots.contains(slot))
    }

    pub(super) fn remark_kept_default(&self, element: &ElementRc) {
        let Some(slot) = element.borrow().slot_default.clone() else {
            return;
        };
        let marker = self.kept_slots.get(&slot).cloned();
        debug_assert!(marker.is_some(), "lowered default of slot '{slot}' matches no kept slot");
        if let Some(marker) = marker {
            element.borrow_mut().slot_default = marker;
        }
    }

    pub(super) fn retain_optimized(
        &mut self,
        mut optimized: Vec<OptimizedElement>,
    ) -> Vec<OptimizedElement> {
        optimized.retain(|o| {
            let replaced = self.is_replaced_default(&o.element);
            if replaced {
                self.insert_subtree(&o.element);
            } else {
                self.remark_kept_default(&o.element);
            }
            !replaced
        });
        loop {
            let len = optimized.len();
            optimized.retain(|o| {
                let removed = o
                    .former_parent
                    .upgrade()
                    .is_some_and(|parent| self.elements.contains(&element_key(parent)));
                if removed {
                    self.insert_subtree(&o.element);
                }
                !removed
            });
            if optimized.len() == len {
                return optimized;
            }
        }
    }

    fn insert_subtree(&mut self, root: &ElementRc) {
        recurse_elem(root, &(), &mut |element, _| {
            self.elements.insert(element_key(element.clone()));
        });
    }
}

pub(super) fn take_defaults(
    following: &[ElementRc],
    slot_name: &SmolStr,
    supplied: bool,
    kept_marker: Option<&SmolStr>,
    removed: &mut RemovedDefaults,
) -> usize {
    let len = following
        .iter()
        .take_while(|child| child.borrow().slot_default.as_ref() == Some(slot_name))
        .count();
    let defaults = &following[..len];
    if supplied {
        removed.replaced_slots.insert(slot_name.clone());
        for default in defaults {
            removed.insert_subtree(default);
        }
        len
    } else {
        removed.kept_slots.insert(slot_name.clone(), kept_marker.cloned());
        for default in defaults {
            default.borrow_mut().slot_default = kept_marker.cloned();
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{RemovedDefaults, element_key};
    use crate::diagnostics::BuildDiagnostics;
    use crate::expression_tree::Expression;
    use crate::object_tree::{Component, Element, ElementRc, OptimizedElement};
    use std::cell::RefCell;
    use std::rc::Rc;

    const SOURCE: &str = r#"
component Host inherits Rectangle {
    slot content << Rectangle {
        init => { debug("default init"); }
        default-timer := Timer {
            interval: 10ms;
            triggered => { debug("default timer"); }
        }
        p := PopupWindow { }
        TouchArea { clicked => { p.show(); } }
        ContextMenuArea {
            default-menu := Menu {
                MenuItem { title: "item"; }
            }
        }
        GridLayout {
            default-row := Row {
                Timer { interval: 10ms; }
                Rectangle { }
            }
        }
    }
    VerticalLayout {
        content { }
    }
}

component Middle inherits Rectangle {
    slot outer;
    Host {
        content << outer;
    }
}

export component Replaced inherits Window {
    Host {
        content << Text { text: "caller"; }
    }
}

export component Kept inherits Window {
    Host { }
}

export component Cleared inherits Window {
    Host {
        content << { }
    }
}

component Clearing inherits Host {
    content << { }
}

export component DerivedCleared inherits Window {
    Clearing { }
}

export component Forwarded inherits Window {
    Middle {
        outer << Text { text: "caller"; }
    }
}

component Overriding inherits Host {
    content << Rectangle {
        init => { debug("derived init"); }
        derived-timer := Timer {
            interval: 10ms;
            triggered => { debug("derived timer"); }
        }
    }
}

export component DerivedKept inherits Window {
    Overriding { }
}

export component DerivedReplaced inherits Window {
    Overriding {
        content << Text { text: "caller"; }
    }
}
"#;

    const LOWERED_SOURCE: &str = r#"
component TimerHost inherits Rectangle {
    slot content << default-timer := Timer { interval: 10ms; }
    VerticalLayout {
        content { }
    }
}

component PopupHost inherits Rectangle {
    slot content << default-popup := PopupWindow { }
    VerticalLayout {
        content { }
    }
}

component TimerMiddle inherits Rectangle {
    slot outer;
    TimerHost {
        content << outer;
    }
}

component TimerFilling inherits TimerHost {
    content << derived-timer := Timer { interval: 10ms; }
}

component PopupMiddle inherits Rectangle {
    slot outer;
    PopupHost {
        content << outer;
    }
}

component PopupFilling inherits PopupHost {
    content << derived-popup := PopupWindow { }
}

component Plain inherits TimerHost { }

component SameName inherits Rectangle {
    slot content << default-outer-timer := Timer { interval: 10ms; }
    VerticalLayout {
        content { }
        TimerHost {
            content << Rectangle { }
        }
    }
}

export component Kept inherits Window {
    TimerHost { }
    PopupHost { }
}

export component Replaced inherits Window {
    TimerHost {
        content << Rectangle { }
    }
    PopupHost {
        content << Rectangle { }
    }
}

export component Cleared inherits Window {
    TimerHost {
        content << { }
    }
    PopupHost {
        content << { }
    }
}

export component Forwarded inherits Window {
    TimerMiddle {
        outer << Rectangle { }
    }
}

export component ForwardedKept inherits Window {
    TimerMiddle { }
}

export component DerivedKept inherits Window {
    TimerFilling { }
}

export component DerivedReplaced inherits Window {
    TimerFilling {
        content << Rectangle { }
    }
}

export component SuppliedTimer inherits Window {
    TimerHost {
        content << caller-timer := Timer { interval: 10ms; }
    }
}

export component SuppliedPopup inherits Window {
    TimerHost {
        content << PopupWindow { }
    }
}

export component PopupForwarded inherits Window {
    PopupMiddle {
        outer << Rectangle { }
    }
}

export component PopupForwardedKept inherits Window {
    PopupMiddle { }
}

export component PopupDerivedKept inherits Window {
    PopupFilling { }
}

export component PopupDerivedReplaced inherits Window {
    PopupFilling {
        content << Rectangle { }
    }
}

export component ReexportedKept inherits Window {
    Plain { }
}

export component ReexportedReplaced inherits Window {
    Plain {
        content << Rectangle { }
    }
}

export component SameNameKept inherits Window {
    SameName { }
}

export component SameNameReplaced inherits Window {
    SameName {
        content << Rectangle { }
    }
}

export component PopupSuppliedForPopup inherits Window {
    PopupHost {
        content << caller-popup := PopupWindow { }
    }
}
"#;

    fn exported_roots() -> Vec<Rc<Component>> {
        compile(SOURCE)
    }

    fn compile(source: &str) -> Vec<Rc<Component>> {
        let mut compiler_config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Llr);
        compiler_config.enable_experimental = true;
        compiler_config.inline_all_elements = true;
        let mut diag = BuildDiagnostics::default();
        let doc_node = crate::parser::parse(
            source.into(),
            Some(crate::source_path::SourcePath::new("test.slint")),
            &mut diag,
        );
        let (doc, diag, _) =
            spin_on::spin_on(crate::compile_syntax_node(doc_node, diag, compiler_config));
        assert!(!diag.has_errors(), "compile error: {:?}", diag.to_string_vec());
        doc.exported_roots().collect()
    }

    fn root(roots: &[Rc<Component>], id: &str) -> Rc<Component> {
        roots.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no root {id}")).clone()
    }

    fn init_count(component: &Component, literal: &str) -> usize {
        let mut count = 0;
        for expression in component.init_code.borrow().iter() {
            expression.visit_recursive(&mut |e| {
                if matches!(e, Expression::StringLiteral(s) if s == literal) {
                    count += 1;
                }
            });
        }
        count
    }

    fn optimized_default_ids(component: &Component) -> Vec<String> {
        component
            .optimized_elements
            .borrow()
            .iter()
            .map(|o| o.element.borrow().id.to_string())
            .filter(|id| id.contains("default-"))
            .collect()
    }

    #[test]
    fn replaced_default_drops_what_it_created() {
        let roots = exported_roots();
        for id in ["Replaced", "Forwarded", "Cleared", "DerivedCleared"] {
            let component = root(&roots, id);
            assert_eq!(component.popup_windows.borrow().len(), 0, "{id}");
            assert_eq!(component.timers.borrow().len(), 0, "{id}");
            assert_eq!(component.menu_item_tree.borrow().len(), 0, "{id}");
            assert_eq!(init_count(&component, "default init"), 0, "{id}");
            assert_eq!(optimized_default_ids(&component), Vec::<String>::new(), "{id}");
        }
    }

    #[test]
    fn derived_content_drops_what_the_base_default_created() {
        let component = root(&exported_roots(), "DerivedKept");
        assert_eq!(component.popup_windows.borrow().len(), 0);
        assert_eq!(component.menu_item_tree.borrow().len(), 0);
        assert_eq!(init_count(&component, "default init"), 0);
        assert_eq!(optimized_default_ids(&component), Vec::<String>::new());
        assert_eq!(init_count(&component, "derived init"), 1);
        assert_eq!(component.timers.borrow().len(), 1);
    }

    #[test]
    fn replaced_derived_content_drops_what_it_created() {
        let component = root(&exported_roots(), "DerivedReplaced");
        assert_eq!(component.popup_windows.borrow().len(), 0);
        assert_eq!(component.timers.borrow().len(), 0);
        assert_eq!(component.menu_item_tree.borrow().len(), 0);
        assert_eq!(init_count(&component, "default init"), 0);
        assert_eq!(init_count(&component, "derived init"), 0);
        assert_eq!(optimized_default_ids(&component), Vec::<String>::new());
    }

    #[test]
    fn removed_chain_drops_every_link() {
        let element =
            |id: &str| Rc::new(RefCell::new(Element { id: id.into(), ..Default::default() }));
        let grid = element("grid");
        let default = element("default");
        default.borrow_mut().children.push(grid.clone());
        let (row, timer, kept, kept_parent) =
            (element("row"), element("timer"), element("kept"), element("kept-parent"));
        let optimized = |element: &ElementRc, former_parent: &ElementRc| OptimizedElement {
            element: element.clone(),
            former_parent: Rc::downgrade(former_parent),
        };

        let mut removed = RemovedDefaults::default();
        removed.insert_subtree(&default);
        let retained = removed.retain_optimized(vec![
            optimized(&timer, &row),
            optimized(&kept, &kept_parent),
            optimized(&row, &grid),
        ]);

        let ids = retained.iter().map(|o| o.element.borrow().id.to_string()).collect::<Vec<_>>();
        assert_eq!(ids, ["kept"]);
        assert!(removed.elements.contains(&element_key(timer)));
    }

    #[test]
    fn kept_default_keeps_what_it_created() {
        let component = root(&exported_roots(), "Kept");
        assert_eq!(component.popup_windows.borrow().len(), 1);
        assert_eq!(component.timers.borrow().len(), 2);
        assert_eq!(component.menu_item_tree.borrow().len(), 1);
        assert_eq!(init_count(&component, "default init"), 1);
        let ids = optimized_default_ids(&component);
        for name in ["default-timer", "default-menu", "default-row"] {
            assert!(ids.iter().any(|id| id.contains(name)), "{name} not in {ids:?}");
        }
    }

    fn timer_ids(component: &Component) -> Vec<String> {
        component
            .timers
            .borrow()
            .iter()
            .map(|t| t.element.upgrade().unwrap().borrow().id.to_string())
            .collect()
    }

    fn popup_ids(component: &Component) -> Vec<String> {
        component
            .popup_windows
            .borrow()
            .iter()
            .map(|p| p.component.root_element.borrow().id.to_string())
            .collect()
    }

    #[test]
    fn lowered_default_is_kept_or_dropped_with_its_slot() {
        let roots = compile(LOWERED_SOURCE);
        for (id, timers, popups) in [
            ("Kept", 1, 1),
            ("ForwardedKept", 1, 0),
            ("PopupForwardedKept", 0, 1),
            ("ReexportedKept", 1, 0),
            ("SameNameKept", 1, 0),
            ("Replaced", 0, 0),
            ("Cleared", 0, 0),
            ("Forwarded", 0, 0),
            ("PopupForwarded", 0, 0),
            ("DerivedReplaced", 0, 0),
            ("PopupDerivedReplaced", 0, 0),
            ("ReexportedReplaced", 0, 0),
            ("SameNameReplaced", 0, 0),
        ] {
            let component = root(&roots, id);
            assert_eq!(component.timers.borrow().len(), timers, "{id}");
            assert_eq!(component.popup_windows.borrow().len(), popups, "{id}");
            assert_eq!(optimized_default_ids(&component).len(), timers, "{id}");
        }
    }

    #[test]
    fn lowered_content_replaces_the_default() {
        let roots = compile(LOWERED_SOURCE);
        for (id, timer) in [
            ("DerivedKept", "derived-timer"),
            ("SuppliedTimer", "caller-timer"),
            ("SameNameKept", "default-outer-timer"),
        ] {
            let ids = timer_ids(&root(&roots, id));
            assert!(ids.len() == 1 && ids[0].contains(timer), "{id}: {ids:?}");
        }
        let component = root(&roots, "SuppliedPopup");
        assert_eq!(component.timers.borrow().len(), 0);
        assert_eq!(component.popup_windows.borrow().len(), 1);
        for (id, popup) in
            [("PopupDerivedKept", "derived-popup"), ("PopupSuppliedForPopup", "caller-popup")]
        {
            let ids = popup_ids(&root(&roots, id));
            assert!(ids.len() == 1 && ids[0].contains(popup), "{id}: {ids:?}");
        }
    }
}
