// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::{Color, Model, ModelRc, VecModel};
use slint_editor::ui::*;
use std::collections::BTreeMap;

pub fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values))
}

pub fn update_model<T: Clone + PartialEq + 'static>(
    current: ModelRc<T>,
    values: Vec<T>,
) -> ModelRc<T> {
    let Some(rows) = current.as_any().downcast_ref::<VecModel<T>>() else { return model(values) };
    while rows.row_count() > values.len() {
        rows.remove(rows.row_count() - 1);
    }
    for (index, value) in values.into_iter().enumerate() {
        if index == rows.row_count() {
            rows.push(value);
        } else if rows.row_data(index).as_ref() != Some(&value) {
            rows.set_row_data(index, value);
        }
    }
    current
}

#[derive(Clone)]
pub struct Node {
    pub view: GalleryNode,
    pub parent: Option<i32>,
    pub expanded: bool,
    pub properties: BTreeMap<String, String>,
}

#[derive(Default, Clone)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub files: Vec<FileTreeNode>,
    pub committed: Vec<Node>,
    pub selected: i32,
    pub history: Vec<Vec<Node>>,
    pub future: Vec<Vec<Node>>,
}

fn snapshot(nodes: &[Node]) -> Vec<Node> {
    nodes
        .iter()
        .cloned()
        .map(|mut n| {
            n.view.fill.stops = model(n.view.fill.stops.iter().collect());
            n
        })
        .collect()
}

pub fn node(id: i32, parent: Option<i32>, kind: ElementKind, label: &str) -> Node {
    Node {
        view: GalleryNode {
            id,
            label: label.into(),
            kind,
            x: 48. + (id - 2) as f32 * 25.,
            y: 60. + (id - 2) as f32 * 110.,
            width: 220.,
            height: 90.,
            rotation: 0.,
            radius: 12.,
            text: "Hello Slint".into(),
            fill: FillData {
                kind: BrushKind::Solid,
                color: if kind == ElementKind::Text {
                    Color::from_rgb_u8(36, 41, 47)
                } else {
                    Color::from_rgb_u8(92, 105, 225)
                },
                stops: model(vec![
                    GradientStop { color: Color::from_rgb_u8(92, 105, 225), position: 0. },
                    GradientStop { color: Color::from_rgb_u8(238, 134, 172), position: 1. },
                ]),
                angle: 120.,
                ..Default::default()
            },
        },
        parent,
        expanded: true,
        properties: BTreeMap::new(),
    }
}

impl Scene {
    pub fn reset(&mut self, scenario: &str) {
        self.nodes = vec![
            node(1, None, ElementKind::Component, "Main"),
            node(2, Some(1), ElementKind::Rectangle, "card"),
            node(3, Some(1), ElementKind::Text, "title"),
            node(4, Some(1), ElementKind::Image, "artwork"),
            node(5, Some(2), ElementKind::TouchArea, "interaction"),
        ];
        self.selected = match scenario {
            "Text" => 3,
            "Image" => 4,
            "TouchArea" => 5,
            "No selection" | "Empty" => -1,
            _ => 2,
        };
        match scenario {
            "Empty" => self.nodes.clear(),
            "Collapsed" => self.nodes[0].expanded = false,
            "Deep tree" => {
                for id in 6..14 {
                    self.nodes.push(node(
                        id,
                        Some(if id == 6 { 2 } else { id - 1 }),
                        ElementKind::Rectangle,
                        &format!("nested-{id}"),
                    ));
                }
            }
            "Long names" => {
                for n in &mut self.nodes {
                    n.view.label =
                        format!("{}-with-a-long-descriptive-component-name", n.view.label).into();
                }
            }
            "Transparent" => self.nodes[1].view.fill.color = Color::from_argb_u8(80, 92, 105, 225),
            "Rotated" => self.nodes[1].view.rotation = 30.,
            "Clipped" => {
                self.nodes[1].view.x = -45.;
                self.nodes[1].view.y = -25.;
            }
            "Linear" | "Radial" | "Conic" | "Hard edge" => {
                let fill = &mut self.nodes[1].view.fill;
                fill.kind = match scenario {
                    "Radial" => BrushKind::Radial,
                    "Conic" => BrushKind::Conic,
                    _ => BrushKind::Linear,
                };
                if scenario == "Hard edge" {
                    let a = fill.stops.row_data(0).unwrap().color;
                    let b = fill.stops.row_data(1).unwrap().color;
                    fill.stops = model(vec![
                        GradientStop { color: a, position: 0. },
                        GradientStop { color: a, position: 0.5 },
                        GradientStop { color: b, position: 0.5 },
                        GradientStop { color: b, position: 1. },
                    ]);
                }
            }
            _ => {}
        }
        self.committed = snapshot(&self.nodes);
        self.history.clear();
        self.future.clear();
    }
    pub fn selected(&self) -> Option<&Node> {
        self.nodes.iter().find(|n| n.view.id == self.selected)
    }
    pub fn selected_mut(&mut self) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.view.id == self.selected)
    }
    pub fn refresh_committed(&mut self) {
        self.committed = snapshot(&self.nodes);
    }

    pub fn commit(&mut self) {
        self.history.push(snapshot(&self.committed));
        self.committed = snapshot(&self.nodes);
        self.future.clear();
    }
    pub fn cancel(&mut self) {
        let expanded: BTreeMap<_, _> = self.nodes.iter().map(|n| (n.view.id, n.expanded)).collect();
        self.nodes = snapshot(&self.committed);
        for n in &mut self.nodes {
            if let Some(value) = expanded.get(&n.view.id) {
                n.expanded = *value;
            }
        }
    }
    pub fn undo(&mut self, redo: bool) {
        let next = if redo { self.future.pop() } else { self.history.pop() };
        if let Some(next) = next {
            if redo {
                self.history.push(snapshot(&self.committed));
            } else {
                self.future.push(snapshot(&self.committed));
            }
            self.nodes = next;
            self.committed = snapshot(&self.nodes);
        }
    }
    pub fn outline(&self) -> Vec<OutlineTreeNode> {
        fn visit(scene: &Scene, parent: Option<i32>, depth: i32, rows: &mut Vec<OutlineTreeNode>) {
            let children: Vec<_> = scene.nodes.iter().filter(|n| n.parent == parent).collect();
            for (i, n) in children.iter().enumerate() {
                let has_children = scene.nodes.iter().any(|c| c.parent == Some(n.view.id));
                rows.push(OutlineTreeNode {
                    indent_level: depth,
                    has_children,
                    is_expanded: n.expanded,
                    is_last_child: i + 1 == children.len(),
                    icon_kind: n.view.kind,
                    element_type: type_name(n.view.kind).into(),
                    element_id: n.view.label.clone(),
                    uri: "gallery".into(),
                    offset: n.view.id,
                });
                if n.expanded {
                    visit(scene, Some(n.view.id), depth + 1, rows);
                }
            }
        }
        let mut rows = Vec::new();
        visit(self, None, 0, &mut rows);
        rows
    }
    pub fn can_drop(&self, source: Option<i32>, target: i32, location: DropLocation) -> bool {
        let Some(dest) = self.nodes.iter().find(|n| n.view.id == target) else { return false };
        if location != DropLocation::Onto && dest.parent.is_none() {
            return false;
        }
        if location == DropLocation::Onto
            && !matches!(
                dest.view.kind,
                ElementKind::Component | ElementKind::Rectangle | ElementKind::TouchArea
            )
        {
            return false;
        }
        if let Some(source) = source {
            if self.nodes.iter().all(|n| n.view.id != source || n.parent.is_none()) {
                return false;
            }
            let mut ancestor = Some(target);
            while let Some(id) = ancestor {
                if id == source {
                    return false;
                }
                ancestor = self.nodes.iter().find(|n| n.view.id == id).and_then(|n| n.parent);
            }
        }
        true
    }
    pub fn drop_node(
        &mut self,
        source: Option<i32>,
        kind: ElementKind,
        target: i32,
        location: DropLocation,
    ) -> bool {
        if !self.can_drop(source, target, location) {
            return false;
        }
        let target_node = self.nodes.iter().find(|n| n.view.id == target).unwrap();
        let parent = if location == DropLocation::Onto { Some(target) } else { target_node.parent };
        let mut moved = if let Some(id) = source {
            let index = self.nodes.iter().position(|n| n.view.id == id).unwrap();
            self.nodes.remove(index)
        } else {
            let id = self.nodes.iter().map(|n| n.view.id).max().unwrap_or(0) + 1;
            let mut n = node(id, parent, kind, &format!("{}-{id}", type_name(kind).to_lowercase()));
            n.view.x = 70.;
            n.view.y = 80.;
            n
        };
        moved.parent = parent;
        self.selected = moved.view.id;
        let index = self.nodes.iter().position(|n| n.view.id == target).unwrap();
        if location == DropLocation::Onto {
            self.nodes[index].expanded = true;
        }
        self.nodes.insert(index + usize::from(location != DropLocation::Before), moved);
        self.commit();
        true
    }
}

pub fn type_name(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::Rectangle => "Rectangle",
        ElementKind::Text => "Text",
        ElementKind::Image => "Image",
        ElementKind::TouchArea => "TouchArea",
        _ => "Window",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reparent_preserves_descendants_and_rejects_cycles() {
        let mut s = Scene::default();
        s.reset("Default");
        assert!(!s.drop_node(Some(2), ElementKind::None, 5, DropLocation::Onto));
        assert!(!s.drop_node(Some(1), ElementKind::None, 2, DropLocation::Onto));
        assert!(s.drop_node(Some(3), ElementKind::None, 2, DropLocation::Onto));
        assert_eq!(s.nodes.iter().find(|n| n.view.id == 3).unwrap().parent, Some(2));
        s.undo(false);
        assert_eq!(s.nodes.iter().find(|n| n.view.id == 3).unwrap().parent, Some(1));
        s.undo(true);
        assert_eq!(s.nodes.iter().find(|n| n.view.id == 3).unwrap().parent, Some(2));
    }
    #[test]
    fn cancel_restores_gradient_stops_and_reset_clears_history() {
        let mut s = Scene::default();
        s.reset("Hard edge");
        s.selected_mut().unwrap().view.fill.stops = model(vec![]);
        s.cancel();
        assert_eq!(s.selected().unwrap().view.fill.stops.row_count(), 4);
        s.commit();
        s.reset("Empty");
        assert!(s.nodes.is_empty());
        assert!(s.history.is_empty());
    }
}
