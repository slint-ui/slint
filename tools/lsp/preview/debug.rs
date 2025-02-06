// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore fillcolor fontcolor graphviz tpdf

use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};

use i_slint_compiler::object_tree::{Component, Element, ElementRc};

#[derive(Clone)]
struct ElementInfo {
    id: usize,
    component_id: String,
    node_definition: String,
}

impl ElementInfo {
    fn node_id(&self) -> String {
        format!("n{}", self.id)
    }
}

pub struct ElementMap(HashMap<*mut Element, String>);

impl ElementMap {
    #[allow(unused)]
    pub fn node_name(&self, element: &ElementRc) -> Option<&String> {
        self.0.get(&element.as_ptr())
    }
}

#[derive(Default)]
struct State {
    components: HashMap<*const Component, String>,
    elements: HashMap<*mut Element, ElementInfo>,
    current_node_number: usize,
    mark_up: Option<ElementRc>,
}

impl State {
    fn generate_subgraph(&self, name: &str, elements: &[ElementInfo]) -> Vec<String> {
        let mut result = Vec::new();

        let spaces = if name == "root" {
            "  "
        } else {
            result.push(format!("  subgraph cluster_{name} {{"));
            result.push(format!("    label = \"{name}\""));
            result.push("    color = black".to_string());
            "    "
        };

        for ei in elements {
            if ei.component_id == name {
                result.push(format!("{spaces}{}", ei.node_definition));
            }
        }

        if name != "root" {
            result.push("  }".to_string());
        }

        result
    }

    fn generate(&self, connections: Vec<String>) -> String {
        let mut result = vec!["digraph ElementTree {".to_string()];

        let mut values: Vec<_> = self.components.values().cloned().collect();
        values.sort_unstable();
        let mut elements: Vec<_> = self.elements.values().cloned().collect();
        elements.sort_unstable_by(|left, right| left.id.partial_cmp(&right.id).unwrap());

        for cn in &values {
            result.extend_from_slice(&self.generate_subgraph(cn, &elements));
        }

        result.extend_from_slice(&connections);
        result.push("}".to_string());

        result.join("\n")
    }

    fn get_node_number(&mut self) -> usize {
        let result = self.current_node_number;
        self.current_node_number += 1;
        result
    }

    fn element_info(&mut self, element: &ElementRc) -> Option<ElementInfo> {
        self.elements.get(&element.as_ptr()).cloned()
    }

    fn register_component(&mut self, comp: &Rc<Component>, name: String) {
        self.components.entry(Rc::as_ptr(comp)).or_insert(name);
    }
}

/// This function prints a tree of `Element`s starting below `element` in
/// graphviz format. You may optionally pass one element to highlight in the
/// output.
///
/// Arrows moving from one component to the next are printed in blue. Elements
/// lowered from a Layout are printed as boxes, all other elements are round.
///
/// The function returns a map from `ElementRc` to a String containing the
/// node name of that element in the dot file. This is nice as it allows to
/// follow some process as it traverses the tree without having to regenerate
/// the output all the time.
///
/// You can cut and paste the output into
/// <https://dreampuf.github.io/GraphvizOnline/>
/// or into some text file and then run `dot -Tpdf /path/to/file > graph.pdf`
/// to generate a PDF file out of it.
#[allow(unused)]
pub fn as_dot(element: &ElementRc, mark_up: Option<ElementRc>) -> ElementMap {
    let mut state = State { mark_up, ..State::default() };
    state.register_component(
        &Weak::upgrade(&element.borrow().enclosing_component).unwrap(),
        "root".to_string(),
    );

    let (_, connections) = recurse_into_element(&mut state, element);

    i_slint_core::debug_log!(
        "Statistics: {} elements in {} components",
        state.elements.len(),
        state.components.len()
    );
    i_slint_core::debug_log!("{}", state.generate(connections));

    let map: HashMap<_, _> = state.elements.iter().map(|(k, v)| (*k, v.node_id())).collect();
    ElementMap(map)
}

fn recurse_into_element(state: &mut State, element: &ElementRc) -> (usize, Vec<String>) {
    if let Some(existing) = state.element_info(element) {
        return (existing.id, Vec::new());
    }

    let node_number = state.get_node_number();
    add_element_node(state, element, node_number);

    let e = element.borrow();

    let mut lines = Vec::new();

    if let i_slint_compiler::langtype::ElementType::Component(comp) = &e.base_type {
        state.register_component(comp, format!("c{}", state.components.len()));
        let component_root = comp.root_element.clone();
        let (root_id, component_lines) = recurse_into_element(state, &component_root);
        lines.extend_from_slice(&component_lines);
        lines.push(format!("  n{node_number} -> n{root_id} [color = blue]"));
    }

    for c in &e.children {
        let (child_node_number, child_result) = recurse_into_element(state, c);
        lines.extend_from_slice(&child_result);
        lines.push(format!("  n{node_number} -> n{child_node_number}"));
    }

    (node_number, lines)
}

fn add_element_node(state: &mut State, element: &ElementRc, node_number: usize) {
    let e = element.borrow();
    let layout = if e.debug.iter().any(|d| d.layout.is_some()) { ",shape = box" } else { "" };
    let repeated = if e.repeated.is_some() { ",color = blue" } else { "" };
    let component = if matches!(e.base_type, i_slint_compiler::langtype::ElementType::Component(_))
    {
        ",fontcolor = blue"
    } else {
        ""
    };
    let mark_up = if let Some(mu) = &state.mark_up {
        if Rc::ptr_eq(element, mu) {
            ",style=filled,fillcolor=lightgrey"
        } else {
            ""
        }
    } else {
        ""
    };

    let component_id = state.components.get(&e.enclosing_component.as_ptr()).unwrap().clone();

    state.elements.insert(
        element.as_ptr(),
        ElementInfo {
            id: node_number,
            component_id,
            node_definition: format!(
                "    \"n{node_number}\" [label=\"n{node_number}:{}:{}\"{}{}{}{}]",
                e.id, e.base_type, repeated, layout, component, mark_up
            ),
        },
    );
}
