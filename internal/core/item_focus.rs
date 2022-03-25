// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore dealloc nesw

/*!
This module contains the code moving the keyboard focus between items
*/

use crate::item_tree::ComponentItemTree;

pub fn default_next_in_local_focus_chain(
    index: usize,
    item_tree: &crate::item_tree::ComponentItemTree,
) -> Option<usize> {
    if let Some(child) = item_tree.first_child(index) {
        return Some(child);
    }

    let mut self_or_ancestor = index;
    loop {
        if let Some(sibling) = item_tree.next_sibling(self_or_ancestor) {
            return Some(sibling);
        }
        if let Some(ancestor) = item_tree.parent(self_or_ancestor) {
            self_or_ancestor = ancestor;
        } else {
            return None;
        }
    }
}

pub fn default_previous_in_local_focus_chain(
    index: usize,
    item_tree: &crate::item_tree::ComponentItemTree,
) -> Option<usize> {
    fn rightmost_node(item_tree: &ComponentItemTree, index: usize) -> usize {
        let mut node = index;
        loop {
            if let Some(last_child) = item_tree.last_child(node) {
                node = last_child;
            } else {
                return node;
            }
        }
    }

    if let Some(previous) = item_tree.previous_sibling(index) {
        Some(rightmost_node(item_tree, previous))
    } else {
        item_tree.parent(index)
    }
}
