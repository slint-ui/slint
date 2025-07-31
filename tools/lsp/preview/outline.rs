// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::preview::{self, ui};
use core::cell::RefCell;
use i_slint_compiler::object_tree;
use i_slint_compiler::parser::{self, syntax_nodes, TextSize};
use lsp_types::Url;
use slint::{ComponentHandle as _, Model, ModelRc, SharedString, ToSharedString as _};
use std::rc::Rc;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TreeNodeChange {
    None,
    Collapse,
    Expand,
}

trait Tree {
    /// The slint::Model::Data that is being used in the UI
    type Data: Clone + std::fmt::Debug;
    /// An Id or index that can be used to identify the data
    type Id: Clone + std::fmt::Debug;

    /// map id to data
    fn data(&self, id: &Self::Id) -> Option<Self::Data>;
    /// return the children of the given parent.
    /// None means the root.
    fn children(&self, parent: Option<&Self::Id>) -> impl Iterator<Item = Self::Id>;
    /// return the level in the tree of the given Id
    fn level(&self, id: &Self::Id) -> usize;

    /// Update the data for a given id
    /// Returns whether there was a change and we need to collapse or expand the node
    ///
    /// The id is mutable in case changing the data also changes the id
    fn update_data(&self, id: &mut Self::Id, data: Self::Data) -> TreeNodeChange;
}

struct TreeAdapterModel<T: Tree> {
    cached_layout: RefCell<Vec<T::Id>>,
    model_tracker: slint::ModelNotify,

    source: T,
}

impl<T: Tree> TreeAdapterModel<T> {
    pub fn new(source: T) -> Self {
        Self {
            cached_layout: RefCell::new(source.children(None).collect()),
            model_tracker: Default::default(),
            source,
        }
    }

    fn expand(&self, row: usize) {
        let mut count = 0;
        let mut cached_layout = self.cached_layout.borrow_mut();
        let parent = cached_layout[row].clone();
        let index = row + 1;
        cached_layout
            .splice(index..index, self.source.children(Some(&parent)).inspect(|_| count += 1));
        drop(cached_layout);
        self.model_tracker.row_added(index, count);
    }

    fn collapse(&self, row: usize) {
        let mut cached_layout = self.cached_layout.borrow_mut();
        let level = self.source.level(&cached_layout[row]);
        let mut count = 0;
        while row + 1 + count < cached_layout.len()
            && self.source.level(&cached_layout[row + 1 + count]) > level
        {
            count += 1;
        }
        cached_layout.drain(row + 1..row + 1 + count);
        self.model_tracker.row_removed(row + 1, count);
    }
}

impl<T: Tree> Model for TreeAdapterModel<T> {
    type Data = T::Data;

    fn row_count(&self) -> usize {
        self.cached_layout.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.cached_layout.borrow().get(row).and_then(|id| self.source.data(id))
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.model_tracker
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let mut cached_layout = self.cached_layout.borrow_mut();
        let Some(old) = cached_layout.get_mut(row) else { return };
        let change = self.source.update_data(old, data);
        drop(cached_layout);
        self.model_tracker.row_changed(row);
        match change {
            TreeNodeChange::None => {}
            TreeNodeChange::Collapse => self.collapse(row),
            TreeNodeChange::Expand => self.expand(row),
        }
    }
}

struct OutlineModel {
    root_component: Rc<object_tree::Component>,
}

impl OutlineModel {
    pub fn new(root_component: Rc<object_tree::Component>) -> Self {
        Self { root_component }
    }
}

impl Tree for OutlineModel {
    type Data = ui::OutlineTreeNode;
    type Id = (syntax_nodes::Element, ui::OutlineTreeNode);

    fn data(&self, id: &Self::Id) -> Option<Self::Data> {
        Some(id.1.clone())
    }

    fn children(&self, parent: Option<&Self::Id>) -> impl Iterator<Item = Self::Id> {
        match parent {
            None => {
                let root = self.root_component.node.as_ref().map(|n| {
                    let elem = n.Element();
                    let name = match elem.QualifiedName() {
                        None => n.DeclaredIdentifier().text().to_shared_string(),
                        Some(base) => slint::format!(
                            "{} inherits {} ",
                            n.DeclaredIdentifier().text(),
                            base.text()
                        ),
                    };
                    let data = create_node(&elem, 0, name);
                    (elem, data)
                });
                itertools::Either::Left(root.into_iter())
            }
            Some(parent) => {
                let indent_level = parent.1.indent_level + 1;
                let mut iter = parent
                    .0
                    .children()
                    .filter_map(move |n| {
                        let se = match n.kind() {
                            parser::SyntaxKind::SubElement => syntax_nodes::SubElement::from(n),
                            parser::SyntaxKind::RepeatedElement => {
                                syntax_nodes::RepeatedElement::from(n).SubElement()
                            }
                            parser::SyntaxKind::ConditionalElement => {
                                syntax_nodes::ConditionalElement::from(n).SubElement()
                            }
                            _ => return None,
                        };
                        let elem = se.Element();
                        let base = elem
                            .QualifiedName()
                            .map(|x| x.text().to_shared_string())
                            .unwrap_or_default();
                        let name = match se.child_text(parser::SyntaxKind::Identifier) {
                            None => base,
                            Some(id) => slint::format!("{id} = {base}"),
                        };
                        let node = create_node(&elem, indent_level, name);
                        Some((elem, node))
                    })
                    .peekable();
                itertools::Either::Right(std::iter::from_fn(move || {
                    iter.next().map(|(elem, mut node)| {
                        node.is_last_child = iter.peek().is_none();
                        (elem, node)
                    })
                }))
            }
        }
    }

    fn level(&self, id: &Self::Id) -> usize {
        id.1.indent_level as usize
    }

    fn update_data(&self, id: &mut Self::Id, data: Self::Data) -> TreeNodeChange {
        let r = if id.1.is_expended == data.is_expended {
            TreeNodeChange::None
        } else if data.is_expended {
            TreeNodeChange::Expand
        } else {
            TreeNodeChange::Collapse
        };
        id.1 = data;
        r
    }
}

fn create_node(
    element: &syntax_nodes::Element,
    indent_level: i32,
    name: SharedString,
) -> ui::OutlineTreeNode {
    ui::OutlineTreeNode {
        has_children: element.SubElement().next().is_some(),
        is_expended: false,
        indent_level,
        name,
        file_name: element.source_file.path().display().to_shared_string(),
        offset: usize::from(element.text_range().start()) as i32,
        is_last_child: true,
    }
}

pub fn reset_outline(ui: &ui::PreviewUi, root_component: Option<Rc<object_tree::Component>>) {
    let api = ui.global::<ui::Api>();
    match root_component {
        Some(root) => api.set_outline(ModelRc::new(TreeAdapterModel::new(OutlineModel::new(root)))),
        None => api.set_outline(Default::default()),
    }
}

pub fn setup(ui: &ui::PreviewUi) {
    let api = ui.global::<ui::Api>();
    api.on_outline_select_element(|path, offset| {
        super::element_selection::select_element_at_source_code_position(
            std::path::PathBuf::from(path.as_str()),
            TextSize::new(offset as u32),
            None,
            super::SelectionNotification::Now,
        );
    });
    api.on_outline_drop(|data, target_file, target_offset, location| {
        let Some(edit) = drop_edit(data, target_file, target_offset, location) else {
            return;
        };
        preview::send_workspace_edit("Drop element".to_string(), edit, true);
    });
}

fn drop_edit(
    data: SharedString,
    target_file: SharedString,
    target_offset: i32,
    location: i32,
) -> Option<lsp_types::WorkspaceEdit> {
    let document_cache = super::document_cache()?;
    let url = Url::from_file_path(target_file.as_str()).ok()?;
    let target_elem =
        document_cache.element_at_offset(&url, TextSize::new(target_offset as u32))?;

    let drop_info = if location == 0 {
        preview::drop_location::DropInformation {
            insert_info: preview::drop_location::insert_position_at_end(&target_elem)?,
            target_element_node: target_elem,
            drop_mark: None,
            child_index: 0,
        }
    } else {
        let parent = target_elem.parent()?;
        let children = parent.children();
        let index = children.iter().position(|c| c == &target_elem)?;
        if location < 0 {
            preview::drop_location::DropInformation {
                insert_info: preview::drop_location::insert_position_before_child(&parent, index)?,
                target_element_node: parent,
                drop_mark: None,
                child_index: index,
            }
        } else if index == children.len() - 1 {
            preview::drop_location::DropInformation {
                insert_info: preview::drop_location::insert_position_at_end(&parent)?,
                target_element_node: parent,
                drop_mark: None,
                child_index: index,
            }
        } else {
            preview::drop_location::DropInformation {
                insert_info: preview::drop_location::insert_position_before_child(
                    &parent,
                    index + 1,
                )?,
                target_element_node: parent,
                drop_mark: None,
                child_index: index + 1,
            }
        }
    };

    let workspace_edit = if let Some((item_file, item_offset)) = data.rsplit_once(':') {
        if *item_file != *target_file {
            eprintln!("Can Only move elements in the same file");
            return None;
        }
        let moving_element =
            document_cache.element_at_offset(&url, TextSize::new(item_offset.parse().ok()?))?;
        if moving_element == drop_info.target_element_node {
            return None;
        }
        preview::drop_location::create_swap_element_workspace_edit(
            &drop_info,
            &moving_element,
            Default::default(),
        )?
    } else if let Ok(library_index) = data.parse::<usize>() {
        let component = super::PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            preview_state.known_components.get(library_index).cloned()
        })?;
        preview::drop_location::create_drop_element_workspace_edit(
            &document_cache,
            &component,
            &drop_info,
        )?
    } else {
        eprintln!("Invalid data {data:?}");
        return None;
    };

    Some(workspace_edit.0)
}
