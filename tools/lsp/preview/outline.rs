// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::preview::ui;
use core::cell::RefCell;
use i_slint_compiler::object_tree;
use i_slint_compiler::parser::{self, syntax_nodes};
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
                itertools::Either::Right(parent.0.SubElement().map(move |se| {
                    let elem = se.Element();
                    let base = elem
                        .QualifiedName()
                        .map(|x| x.text().to_shared_string())
                        .unwrap_or_default();
                    let name = match se.child_text(parser::SyntaxKind::Identifier) {
                        None => base,
                        Some(id) => slint::format!("{id} = {base}"),
                    };
                    let data = create_node(&elem, indent_level, name);
                    (elem, data)
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
    }
}

pub fn reset_outline(ui: &ui::PreviewUi, root_component: Option<Rc<object_tree::Component>>) {
    let api = ui.global::<ui::Api>();
    match root_component {
        Some(root) => api.set_outline(ModelRc::new(TreeAdapterModel::new(OutlineModel::new(root)))),
        None => api.set_outline(Default::default()),
    }
}
