// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore nesw

/*!
This module contains the code moving the keyboard focus between items
*/

use crate::item_tree::ItemTreeNodeArray;

pub fn step_out_of_node(
    index: u32,
    item_tree: &crate::item_tree::ItemTreeNodeArray,
) -> Option<u32> {
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

pub fn default_next_in_local_focus_chain(
    index: u32,
    item_tree: &crate::item_tree::ItemTreeNodeArray,
) -> Option<u32> {
    if let Some(child) = item_tree.first_child(index) {
        return Some(child);
    }

    step_out_of_node(index, item_tree)
}

fn step_into_node(item_tree: &ItemTreeNodeArray, index: u32) -> u32 {
    let mut node = index;
    loop {
        if let Some(last_child) = item_tree.last_child(node) {
            node = last_child;
        } else {
            return node;
        }
    }
}

pub fn default_previous_in_local_focus_chain(
    index: u32,
    item_tree: &crate::item_tree::ItemTreeNodeArray,
) -> Option<u32> {
    if let Some(previous) = item_tree.previous_sibling(index) {
        Some(step_into_node(item_tree, previous))
    } else {
        item_tree.parent(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    use crate::item_tree::ItemTreeNode;

    fn validate_focus_chains(item_tree: ItemTreeNodeArray<'_>) {
        let forward_chain = {
            let mut tmp = alloc::vec::Vec::with_capacity(item_tree.node_count());
            let mut node = 0;

            loop {
                tmp.push(node);
                if let Some(next_node) = default_next_in_local_focus_chain(node, &item_tree) {
                    node = next_node;
                } else {
                    break;
                }
            }
            tmp
        };
        let reverse_backward_chain = {
            let mut tmp = alloc::vec::Vec::with_capacity(item_tree.node_count());
            let mut node = step_into_node(&item_tree, 0);

            loop {
                tmp.push(node);
                if let Some(next_node) = default_previous_in_local_focus_chain(node, &item_tree) {
                    node = next_node;
                } else {
                    break;
                }
            }
            tmp.reverse();
            tmp
        };

        assert_eq!(forward_chain, reverse_backward_chain);
        assert_eq!(forward_chain.len(), item_tree.node_count());
    }

    #[test]
    fn test_focus_chain_root_only() {
        let nodes = vec![ItemTreeNode::Item {
            is_accessible: false,
            children_count: 0,
            children_index: 1,
            parent_index: 0,
            item_array_index: 0,
        }];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();
        validate_focus_chains(tree);
    }

    #[test]
    fn test_focus_chain_one_child() {
        let nodes = vec![
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 1,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 2,
                parent_index: 0,
                item_array_index: 0,
            },
        ];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();
        validate_focus_chains(tree);
    }

    #[test]
    fn test_focus_chain_three_children() {
        let nodes = vec![
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 3,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                is_accessible: false,
                children_count: 0,
                children_index: 4,
                parent_index: 0,
                item_array_index: 0,
            },
        ];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();
        validate_focus_chains(tree);
    }

    #[test]
    fn test_focus_chain_complex_tree() {
        let nodes = vec![
            ItemTreeNode::Item {
                // 0
                is_accessible: false,
                children_count: 2,
                children_index: 1,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 1
                is_accessible: false,
                children_count: 2,
                children_index: 3,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 2
                is_accessible: false,
                children_count: 1,
                children_index: 11,
                parent_index: 0,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 3
                is_accessible: false,
                children_count: 1,
                children_index: 5,
                parent_index: 1,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 4
                is_accessible: false,
                children_count: 2,
                children_index: 6,
                parent_index: 1,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 5
                is_accessible: false,
                children_count: 0,
                children_index: 0,
                parent_index: 3,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 6
                is_accessible: false,
                children_count: 2,
                children_index: 8,
                parent_index: 4,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 7
                is_accessible: false,
                children_count: 1,
                children_index: 10,
                parent_index: 4,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 8
                is_accessible: false,
                children_count: 0,
                children_index: 0,
                parent_index: 6,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 9
                is_accessible: false,
                children_count: 0,
                children_index: 0,
                parent_index: 6,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 10
                is_accessible: false,
                children_count: 0,
                children_index: 0,
                parent_index: 7,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 11
                is_accessible: false,
                children_count: 2,
                children_index: 12,
                parent_index: 2,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 12
                is_accessible: false,
                children_count: 0,
                children_index: 0,
                parent_index: 11,
                item_array_index: 0,
            },
            ItemTreeNode::Item {
                // 13
                is_accessible: false,
                children_count: 0,
                children_index: 0,
                parent_index: 11,
                item_array_index: 0,
            },
        ];

        let tree: ItemTreeNodeArray = (nodes.as_slice()).into();
        validate_focus_chains(tree);
    }
}
