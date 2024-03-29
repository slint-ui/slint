// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};
use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalSize};
use slint_interpreter::ComponentInstance;

use crate::common;
use crate::language::completion;
use crate::preview::{self, element_selection, ui};
use crate::util;

use crate::preview::ext::ElementRcNodeExt;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub struct TextOffsetAdjustment {
    pub start_offset: u32,
    pub end_offset: u32,
    pub new_text_length: u32,
}

impl TextOffsetAdjustment {
    pub fn new(
        edit: &lsp_types::TextEdit,
        source_file: &i_slint_compiler::diagnostics::SourceFile,
    ) -> Self {
        let new_text_length = edit.new_text.len() as u32;
        let (start_offset, end_offset) = {
            let so = source_file
                .offset(edit.range.start.line as usize, edit.range.start.character as usize);
            let eo =
                source_file.offset(edit.range.end.line as usize, edit.range.end.character as usize);
            (std::cmp::min(so, eo) as u32, std::cmp::max(so, eo) as u32)
        };

        Self { start_offset, end_offset, new_text_length }
    }

    pub fn adjust(&self, offset: u32) -> u32 {
        // This is a bit simplistic: We ignore special cases like the offset
        // being in the area that gets removed.
        // Worst case: Some unexpected element gets selected. We can live with that.
        if offset >= self.start_offset {
            let old_length = self.end_offset - self.start_offset;
            offset + self.new_text_length - old_length
        } else {
            offset
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum DropMarkDirection {
    None,
    N,
    E,
    S,
    W,
}

impl DropMarkDirection {
    /// Create `DropMarkSetup` for a 'live' element (accpting the drop)
    pub fn for_element(
        element_geometry: &LogicalRect,
        position: LogicalPoint,
        element_is_in_layout: ui::LayoutKind,
    ) -> Self {
        if Self::element_accepts(element_geometry, position, element_is_in_layout) {
            DropMarkDirection::None
        } else {
            Self::element_direction(element_geometry, position, element_is_in_layout)
        }
    }

    fn border_size(dimension: f32) -> f32 {
        let bs = (dimension / 4.0).floor();
        if bs > 8.0 {
            8.0
        } else {
            bs
        }
    }

    // Does this hit the center area of an element that accept drops?
    fn element_accepts(
        element_geometry: &LogicalRect,
        position: LogicalPoint,
        element_is_in_layout: ui::LayoutKind,
    ) -> bool {
        if !element_geometry.contains(position) {
            return false;
        }

        let mut inner = element_geometry.clone();

        let bs_h = Self::border_size(element_geometry.size.width);
        let bs_v = Self::border_size(element_geometry.size.height);

        if bs_h < 0.9 || bs_v < 0.9 {
            // To small to sub-divide into individual drop-zones:-/
            return true;
        }

        match element_is_in_layout {
            ui::LayoutKind::None => {}
            ui::LayoutKind::Horizontal => {
                inner.origin.x += bs_h;
                inner.size.width -= 2.0 * bs_h;
            }
            ui::LayoutKind::Vertical => {
                inner.origin.y += bs_v;
                inner.size.height -= 2.0 * bs_v;
            }
            ui::LayoutKind::Grid => {
                inner.origin.x += bs_h;
                inner.size.width -= 2.0 * bs_h;
                inner.origin.y += bs_v;
                inner.size.height -= 2.0 * bs_v;
            }
        };

        inner.contains(position)
    }

    /// Create `DropMarkSetup` for an element just based on layout info and ignoring
    /// the element itself as a drop target (which is handled by `element_accepts`).
    fn element_direction(
        element_geometry: &LogicalRect,
        position: LogicalPoint,
        element_is_in_layout: ui::LayoutKind,
    ) -> Self {
        if !element_geometry.contains(position) {
            return DropMarkDirection::None;
        }

        if Self::border_size(element_geometry.size.width) <= 0.9
            || Self::border_size(element_geometry.size.height) <= 0.9
        {
            // Geometry is too small to sub-divide:
            return match element_is_in_layout {
                ui::LayoutKind::None => DropMarkDirection::None,
                ui::LayoutKind::Horizontal => DropMarkDirection::E,
                ui::LayoutKind::Vertical => DropMarkDirection::S,
                ui::LayoutKind::Grid => DropMarkDirection::E,
            };
        }

        match element_is_in_layout {
            ui::LayoutKind::None => DropMarkDirection::None,
            ui::LayoutKind::Horizontal => {
                if position.x <= element_geometry.origin.x + (element_geometry.size.width / 2.0) {
                    DropMarkDirection::W
                } else {
                    DropMarkDirection::E
                }
            }
            ui::LayoutKind::Vertical => {
                if position.y <= element_geometry.origin.y + (element_geometry.size.height / 2.0) {
                    DropMarkDirection::N
                } else {
                    DropMarkDirection::S
                }
            }
            ui::LayoutKind::Grid => {
                let x = position.x - element_geometry.origin.x;
                let y = position.y - element_geometry.origin.y;
                let ascend = element_geometry.size.height / element_geometry.size.width;

                let ne_half = y <= ascend * x;
                let nw_half = y <= (x * -ascend) + element_geometry.size.height;

                match (ne_half, nw_half) {
                    (false, false) => DropMarkDirection::S,
                    (false, true) => DropMarkDirection::W,
                    (true, false) => DropMarkDirection::E,
                    (true, true) => DropMarkDirection::N,
                }
            }
        }
    }
}

pub struct DropInformation {
    pub target_element_node: common::ElementRcNode,
    pub insert_info: InsertInformation,
    pub drop_mark: Option<DropMark>,
}

pub struct InsertInformation {
    pub insertion_position: common::VersionedPosition,
    pub replacement_range: u32,
    pub pre_indent: String,
    pub indent: String,
    pub post_indent: String,
}

#[derive(Clone, Debug)]
pub struct DropMark {
    pub start: i_slint_core::lengths::LogicalPoint,
    pub end: i_slint_core::lengths::LogicalPoint,
}

fn insert_position_at_end(
    target_element_node: &common::ElementRcNode,
) -> Option<InsertInformation> {
    target_element_node.with_element_node(|node| {
        let closing_brace = crate::util::last_non_ws_token(node)?;
        let closing_brace_offset = Into::<u32>::into(closing_brace.text_range().start());

        let before_closing = closing_brace.prev_token()?;

        let (pre_indent, indent, post_indent, offset, replacement_range) = if before_closing.kind()
            == SyntaxKind::Whitespace
            && before_closing.text().contains('\n')
        {
            let bracket_indent = before_closing.text().split('\n').last().unwrap(); // must exist in this branch
            (
                "    ".to_string(),
                format!("{bracket_indent}    "),
                bracket_indent.to_string(),
                closing_brace_offset,
                0,
            )
        } else if before_closing.kind() == SyntaxKind::Whitespace
            && !before_closing.text().contains('\n')
        {
            let indent = util::find_element_indent(&target_element_node).unwrap_or_default();
            let ws_len = before_closing.text().len() as u32;
            (
                format!("\n{indent}    "),
                format!("{indent}    "),
                indent,
                closing_brace_offset - ws_len,
                ws_len,
            )
        } else {
            let indent = util::find_element_indent(&target_element_node).unwrap_or_default();
            (format!("\n{indent}    "), format!("{indent}    "), indent, closing_brace_offset, 0)
        };

        let url = lsp_types::Url::from_file_path(node.source_file.path()).ok()?;
        let (version, _) = preview::get_url_from_cache(&url)?;

        Some(InsertInformation {
            insertion_position: common::VersionedPosition::new(
                crate::common::VersionedUrl::new(url, version),
                offset,
            ),
            replacement_range,
            pre_indent,
            indent,
            post_indent,
        })
    })
}

fn drop_target_element_node(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    component_type: &str,
) -> (Option<common::ElementRcNode>, Option<common::ElementRcNode>) {
    let mut self_node = None;
    let mut surrounding_node = None;
    let tl = component_instance.definition().type_loader();
    for sc in &element_selection::collect_all_element_nodes_covering(position, component_instance) {
        let Some(en) = sc.as_element_node() else {
            continue;
        };

        if en.with_element_node(preview::is_element_node_ignored) {
            continue;
        }

        let (path, _) = en.path_and_offset();
        let Some(doc) = tl.get_document(&path) else {
            continue;
        };
        if let Some(element_type) = en.with_element_node(|node| {
            util::lookup_current_element_type((node.clone()).into(), &doc.local_registry)
        }) {
            if en.layout_kind() == ui::LayoutKind::None
                && element_type.accepts_child_element(component_type, &doc.local_registry).is_err()
            {
                break;
            }
        }

        if !element_selection::is_same_file_as_root_node(component_instance, &en) {
            continue;
        }

        if self_node.is_some() {
            surrounding_node = Some(en);
            break;
        } else {
            self_node = Some(en);
        }
    }
    (self_node, surrounding_node)
}

fn extract_element(node: SyntaxNode) -> Option<i_slint_compiler::parser::syntax_nodes::Element> {
    match node.kind() {
        SyntaxKind::Element => Some(node.into()),
        SyntaxKind::SubElement => extract_element(node.child_node(SyntaxKind::Element)?),
        SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
            extract_element(node.child_node(SyntaxKind::SubElement)?)
        }
        _ => None,
    }
}

fn examine_target_element_node(
    context: &str,
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    target_element_node: &common::ElementRcNode,
) {
    let geometry = target_element_node.geometry_at(component_instance, position);
    // let mut result = Vec::new();
    if let Some(geometry) = geometry {
        for (i, c) in target_element_node.element.borrow().children.iter().enumerate() {
            let c = common::ElementRcNode::new(c.clone(), 0).unwrap();
        }
        target_element_node.with_element_node(|node| {
            for (i, c) in node.children().enumerate() {
                let element_data = if let Some(c_element) = extract_element(c.clone()) {
                    format!(
                        "\n        {:?}:{:?}",
                        c_element.source_file.path(),
                        c_element.text_range().start()
                    )
                } else {
                    String::new()
                };
            }
        });
    }
}

fn drop_into_layout(
    component_instance: &ComponentInstance,
    element_node: common::ElementRcNode,
    insert_position: Option<(DropMarkDirection, common::ElementRcNode)>,
    position: LogicalPoint,
) -> Option<DropInformation> {
    let geometry = element_node.geometry_at(component_instance, position)?;
    let insert_info = insert_position_at_end(&element_node)?;

    Some(DropInformation {
        target_element_node: element_node,
        insert_info,
        drop_mark: Some(DropMark {
            start: geometry.origin + LogicalSize::new(0.0, geometry.size.height - 1.0),
            end: geometry.origin + geometry.size,
        }),
    })
}

fn drop_into_element(
    component_instance: &ComponentInstance,
    element_node: common::ElementRcNode,
    surround_node: Option<common::ElementRcNode>,
    position: LogicalPoint,
) -> Option<DropInformation> {
    let geometry = element_node.geometry_at(component_instance, position)?;
    let drop_mark_direction = DropMarkDirection::for_element(
        &geometry,
        position,
        surround_node.as_ref().map(|n| n.layout_kind()).unwrap_or(ui::LayoutKind::None),
    );

    if drop_mark_direction == DropMarkDirection::None {
        let insert_info = insert_position_at_end(&element_node)?;

        Some(DropInformation { target_element_node: element_node, insert_info, drop_mark: None })
    } else {
        drop_into_layout(
            component_instance,
            surround_node.unwrap(),
            Some((drop_mark_direction, element_node)),
            position,
        )
    }
}

fn find_drop_location(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    component_type: &str,
) -> Option<DropInformation> {
    let (drop_element_node, drop_surrounding_element_node) =
        drop_target_element_node(component_instance, position, component_type);

    let drop_element_node = drop_element_node?;

    examine_target_element_node("Drop target", component_instance, position, &drop_element_node);
    if let Some(sn) = &drop_surrounding_element_node {
        examine_target_element_node("Surrounding", component_instance, position, sn);
    }

    if drop_element_node.layout_kind() != ui::LayoutKind::None {
        drop_into_layout(component_instance, drop_element_node, None, position)
    } else {
        drop_into_element(
            component_instance,
            drop_element_node,
            drop_surrounding_element_node,
            position,
        )
    }
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_drop_at(x: f32, y: f32, component: &common::ComponentInformation) -> bool {
    let component_type = component.name.to_string();
    let position = LogicalPoint::new(x, y);
    if let Some(dm) = &super::component_instance()
        .and_then(|ci| find_drop_location(&ci, position, &component_type))
    {
        super::set_drop_mark(&dm.drop_mark);
        true
    } else {
        super::set_drop_mark(&None);
        false
    }
}

/// Extra data on an added Element, relevant to the Preview side only.
#[derive(Clone, Debug)]
pub struct DropData {
    /// The offset to select next. This is different from the insert position
    /// due to indentation, etc.
    pub selection_offset: u32,
    pub path: std::path::PathBuf,
}

/// Find a location in a file that would be a good place to insert the new component at
///
/// Return a WorkspaceEdit to send to the editor and extra info for the live preview in
/// the DropData struct.
pub fn drop_at(
    x: f32,
    y: f32,
    component: &common::ComponentInformation,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let position = LogicalPoint::new(x, y);
    let component_type = &component.name;
    let component_instance = preview::component_instance()?;
    let tl = component_instance.definition().type_loader();
    let drop_info = find_drop_location(&component_instance, position, component_type)?;

    let properties = {
        let mut props = component.default_properties.clone();

        if drop_info.target_element_node.layout_kind() == ui::LayoutKind::None
            && !component.fills_parent
        {
            if let Some(area) =
                drop_info.target_element_node.geometry_at(&component_instance, position)
            {
                props.push(common::PropertyChange::new("x", format!("{}px", x - area.origin.x)));
                props.push(common::PropertyChange::new("y", format!("{}px", y - area.origin.y)));
            }
        }

        props
    };

    let new_text = if properties.is_empty() {
        format!(
            "{}{} {{ }}\n{}",
            drop_info.insert_info.pre_indent, component_type, drop_info.insert_info.post_indent
        )
    } else {
        let mut to_insert = format!("{}{} {{\n", drop_info.insert_info.pre_indent, component_type);
        for p in &properties {
            to_insert += &format!("{}    {}: {};\n", drop_info.insert_info.indent, p.name, p.value);
        }
        to_insert +=
            &format!("{}}}\n{}", drop_info.insert_info.indent, drop_info.insert_info.post_indent);
        to_insert
    };

    let mut selection_offset = drop_info.insert_info.insertion_position.offset()
        + new_text.chars().take_while(|c| c.is_whitespace()).map(|c| c.len_utf8()).sum::<usize>()
            as u32;

    let (path, _) = drop_info.target_element_node.path_and_offset();

    let doc = tl.get_document(&path)?;
    let mut edits = Vec::with_capacity(2);
    let import_file = component.import_file_name(&lsp_types::Url::from_file_path(&path).ok());
    if let Some(edit) = completion::create_import_edit(doc, component_type, &import_file) {
        if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
            selection_offset = TextOffsetAdjustment::new(&edit, sf).adjust(selection_offset);
        }
        edits.push(edit);
    }

    let source_file = doc.node.as_ref().unwrap().source_file.clone();

    let start_pos =
        util::map_position(&source_file, drop_info.insert_info.insertion_position.offset().into());
    let end_pos = util::map_position(
        &source_file,
        (drop_info.insert_info.insertion_position.offset()
            + drop_info.insert_info.replacement_range)
            .into(),
    );
    edits.push(lsp_types::TextEdit { range: lsp_types::Range::new(start_pos, end_pos), new_text });

    Some((
        common::create_workspace_edit_from_source_file(&source_file, edits)?,
        DropData { selection_offset, path },
    ))
}

#[cfg(test)]
mod tests {
    use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalSize};

    use crate::preview::{drop_location::DropMarkDirection, ui};

    #[test]
    fn test_drop_mark_direction_border_size() {
        // The border size starts at 0.0 and grows by 1px from there
        // Two borders always fit into the dimension passed into border_size
        let mut expected = 0.0;
        for i in 0_u16..100 {
            let dimension = f32::from(i) / 10.0;
            let bs = DropMarkDirection::border_size(dimension);
            assert!(
                bs >= (expected - 0.05) && bs < (expected + 0.05)
                    || (bs >= (expected + 0.95) && bs < (expected + 1.05))
            );
            assert!((bs * 3.0) <= dimension); // this makes sure the first bs is 0.0
            expected = bs.round();
        }
        // The maximum border size is 4px:
        assert!(expected <= 8.05);
    }

    #[test]
    fn test_drop_mark_direction_no_area() {
        let rect = LogicalRect::new(LogicalPoint::new(50.0, 50.0), LogicalSize::new(0.0, 0.0));
        let position = LogicalPoint::new(50.0, 50.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );

        let rect = LogicalRect::new(LogicalPoint::new(50.0, 50.0), LogicalSize::new(100.0, 0.0));
        let position = LogicalPoint::new(50.0, 50.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );

        let rect = LogicalRect::new(LogicalPoint::new(50.0, 50.0), LogicalSize::new(0.0, 100.0));
        let position = LogicalPoint::new(50.0, 50.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );
    }

    #[test]
    fn test_drop_mark_direction_outside_position() {
        let rect = LogicalRect::new(LogicalPoint::new(50.0, 50.0), LogicalSize::new(50.0, 50.0));
        let position = LogicalPoint::new(45.0, 75.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );

        let position = LogicalPoint::new(105.0, 75.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );

        let position = LogicalPoint::new(75.0, 45.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );

        let position = LogicalPoint::new(75.0, 105.0);
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::None),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Horizontal),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Vertical),
            DropMarkDirection::None
        );
        assert_eq!(
            DropMarkDirection::for_element(&rect, position, ui::LayoutKind::Grid),
            DropMarkDirection::None
        );
    }

    #[test]
    fn test_drop_mark_direction_valid_position() {
        for width in 1_u16..50 {
            for height in 1_u16..50 {
                let width = f32::from(width);
                let height = f32::from(height);

                let rect = LogicalRect::new(
                    LogicalPoint::new(50.0, 50.0),
                    LogicalSize::new(width, height),
                );
                let bs_h = DropMarkDirection::border_size(rect.size.width);
                let bs_v = DropMarkDirection::border_size(rect.size.height);

                // Center: Drop into self, no drop mark ever:
                let pos = LogicalPoint::new(50.0 + (width / 2.0), 50.0 + (height / 2.0));

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    DropMarkDirection::None
                );

                // N-side (in border):
                let pos = LogicalPoint::new(50.0 + (width / 2.0), 49.0 + bs_v);

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::N
                    } else {
                        DropMarkDirection::None
                    }
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::N
                    } else {
                        DropMarkDirection::None
                    }
                );

                // N-side (outside border):
                let pos = LogicalPoint::new(50.0 + (width / 2.0), 50.0 + bs_v);

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    DropMarkDirection::None
                );

                // E-side (inside border):
                let pos = LogicalPoint::new(50.0 + width - bs_h, 50.0 + (height / 2.0));

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::E
                    } else {
                        DropMarkDirection::None
                    }
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::E
                    } else {
                        DropMarkDirection::None
                    }
                );

                // E-side (outside border):
                let pos = LogicalPoint::new(49.0 + width - bs_h, 50.0 + (height / 2.0));

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    DropMarkDirection::None
                );

                // S-side (in border):
                let pos = LogicalPoint::new(50.0 + (width / 2.0), 50.0 + height - bs_v);

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::S
                    } else {
                        DropMarkDirection::None
                    }
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::S
                    } else {
                        DropMarkDirection::None
                    }
                );

                // S-side (outside border):
                let pos = LogicalPoint::new(50.0 + (width / 2.0), 49.0 + height - bs_v);

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    DropMarkDirection::None
                );

                // W-side (inside border):
                let pos = LogicalPoint::new(49.0 + bs_h, 50.0 + (height / 2.0));

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::W
                    } else {
                        DropMarkDirection::None
                    }
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    if bs_h > 0.9 && bs_v > 0.9 {
                        DropMarkDirection::W
                    } else {
                        DropMarkDirection::None
                    }
                );

                // W-side (outside border):
                let pos = LogicalPoint::new(50.0 + bs_h, 50.0 + (height / 2.0));

                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::None),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Horizontal),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Vertical),
                    DropMarkDirection::None
                );
                assert_eq!(
                    DropMarkDirection::for_element(&rect, pos, ui::LayoutKind::Grid),
                    DropMarkDirection::None
                );
            }
        }
    }
}
