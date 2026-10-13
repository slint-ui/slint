// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{Mapping, element_key};
use crate::object_tree::{ElementRc, OptimizedElement, recurse_elem};
use by_address::ByAddress;
use smol_str::SmolStr;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct RemovedDefaults(HashSet<ByAddress<ElementRc>>);

impl RemovedDefaults {
    pub(super) fn contains_original(&self, mapping: &Mapping, original: &ElementRc) -> bool {
        mapping
            .get(&element_key(original.clone()))
            .is_some_and(|copy| self.0.contains(&element_key(copy.clone())))
    }

    pub(super) fn retain_optimized(
        &mut self,
        mut optimized: Vec<OptimizedElement>,
    ) -> Vec<OptimizedElement> {
        loop {
            let len = optimized.len();
            optimized.retain(|o| {
                let removed = o
                    .former_parent
                    .upgrade()
                    .is_some_and(|parent| self.0.contains(&element_key(parent)));
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
            self.0.insert(element_key(element.clone()));
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
        for default in defaults {
            removed.insert_subtree(default);
        }
        len
    } else {
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

    fn exported_roots() -> Vec<Rc<Component>> {
        let mut compiler_config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Llr);
        compiler_config.enable_experimental = true;
        compiler_config.inline_all_elements = true;
        let mut diag = BuildDiagnostics::default();
        let doc_node = crate::parser::parse(
            SOURCE.into(),
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
        for id in ["Replaced", "Forwarded"] {
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
        assert!(removed.0.contains(&element_key(timer)));
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
}
