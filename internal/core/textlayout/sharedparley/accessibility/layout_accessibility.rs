// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Adapted from the AccessKit integration in Parley's `editor` example.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::collections::HashMap;

use accesskit::{Node, NodeId, Rect, Role, TextAlign, TextDirection, TextPosition};
use parley::Cursor;
use parley::layout::{Affinity, Alignment, Cluster, ClusterPath, Layout, LineMetrics, Run};
use parley::style::{Brush, FontStyle};
use skrifa::{
    FontRef,
    raw::{TableProvider, types::NameId},
};

/// AccessKit's `word_starts` are `u8`s, so a span can't describe more characters than that.
const MAX_CHARACTERS_PER_SPAN: usize = u8::MAX as usize + 1;

/// Maps a Parley [`Layout`] onto AccessKit [`Role::TextRun`] nodes, one per span of clusters
/// in logical order.
///
/// Reuse an instance across passes over the same layout, so that unchanged spans keep their
/// node IDs.
#[derive(Clone, Default, Debug)]
pub struct LayoutAccessibility {
    /// The nth span in logical order gets the nth ID, kept across passes.
    span_ids: Vec<NodeId>,
    /// The path of each span's first cluster.
    span_paths_by_access_id: HashMap<NodeId, ClusterPath>,
    /// Makes `cursor_to_access_position` O(1) rather than O(run length).
    span_positions_by_cluster_path: HashMap<ClusterPath, SpanPosition>,
}

#[derive(Copy, Clone, Debug)]
struct SpanPosition {
    span_index: usize,
    character_index: usize,
}

impl LayoutAccessibility {
    fn span_id(&self, span_index: usize) -> Option<NodeId> {
        self.span_ids.get(span_index).copied()
    }

    /// Pushes a `TextRun` node per span of `layout` onto `text_runs`, as children of
    /// `parent_node`.
    ///
    /// `text` is the text `layout` was built from. `x_offset` and `y_offset` place the layout
    /// in the accessibility tree's coordinates. `next_node_id` allocates IDs for new spans.
    ///
    /// `line_break` is the hard line break that follows `text` but wasn't shaped with it.
    /// AccessKit accepts only LF or CRLF there, appended to the last span as one character;
    /// otherwise a caret crossing the break announces the next line's first character.
    #[allow(clippy::too_many_arguments)]
    pub fn build_nodes<B: Brush>(
        &mut self,
        text: &str,
        layout: &Layout<B>,
        text_runs: &mut Vec<(NodeId, Node)>,
        parent_node: &mut Node,
        mut next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
        line_break: Option<&str>,
    ) {
        debug_assert!(matches!(line_break, None | Some("\n" | "\r\n")));
        self.span_paths_by_access_id.clear();
        self.span_positions_by_cluster_path.clear();
        let mut spans_started = 0;
        let mut runs = Vec::new();
        let alignment = layout.alignment();
        let line_count = layout.len();

        for (line_index, line) in layout.lines().enumerate() {
            let metrics = line.metrics();
            // Each node is added once the next one is known, so the two can be linked.
            let mut last_node: Option<(NodeId, Node)> = None;

            runs.clear();
            runs.reserve(line.len());
            {
                let mut run_offset = metrics.offset;
                for run in line.runs() {
                    let advance = run.advance();
                    runs.push((run, run_offset));
                    run_offset += advance;
                }
            }
            runs.sort_by_key(|(r, _)| r.text_range().start);

            let last_run_index = runs.len().checked_sub(1);
            for (run_index, (run, run_offset)) in runs.drain(..).enumerate() {
                let mut span_path = run_start_path(line_index, &run);
                let (mut id, mut node) = self.span_id_and_node(
                    &mut next_node_id,
                    spans_started,
                    &run,
                    alignment,
                    span_path,
                );
                spans_started += 1;
                if run.is_empty() {
                    // So that a cursor in the empty run can still be addressed.
                    self.span_positions_by_cluster_path.insert(
                        span_path,
                        SpanPosition { span_index: spans_started - 1, character_index: 0 },
                    );
                }

                if let Some((last_id, mut last_node)) = last_node.take() {
                    link_spans(last_id, &mut last_node, id, &mut node);
                    add_span(text_runs, parent_node, last_id, last_node);
                }

                let mut prev_style_index: Option<u16> = None;
                let mut span_text = String::new();
                let mut character_lengths = Vec::new();
                let mut span_offset = 0.0;
                let mut span_advance = 0.0;
                let mut character_positions = Vec::new();
                let mut character_widths = Vec::new();
                let mut word_starts = Vec::new();

                for cluster in run.clusters() {
                    let style_index = cluster.style_index();
                    if let Some(prev_index) = prev_style_index
                        && (prev_index != style_index
                            || character_lengths.len() >= MAX_CHARACTERS_PER_SPAN)
                    {
                        prev_style_index = None;
                        finish_span(
                            &mut node,
                            x_offset,
                            y_offset,
                            metrics,
                            run_offset,
                            span_offset,
                            span_advance,
                            core::mem::take(&mut span_text),
                            core::mem::take(&mut character_lengths),
                            core::mem::take(&mut character_positions),
                            core::mem::take(&mut character_widths),
                            core::mem::take(&mut word_starts),
                        );
                        span_offset += span_advance;
                        span_advance = 0.0;
                        (id, node) = {
                            let (old_id, mut old_node) = (id, node);
                            span_path = cluster.path();
                            let (new_id, mut new_node) = self.span_id_and_node(
                                &mut next_node_id,
                                spans_started,
                                &run,
                                alignment,
                                span_path,
                            );
                            spans_started += 1;
                            link_spans(old_id, &mut old_node, new_id, &mut new_node);
                            add_span(text_runs, parent_node, old_id, old_node);
                            (new_id, new_node)
                        };
                    }

                    if prev_style_index.is_none() {
                        prev_style_index = Some(style_index);
                        if let Some(locale) = &cluster.style().locale {
                            node.set_language(locale.as_str());
                        }
                    }

                    let cluster_text = &text[cluster.text_range()];
                    span_text.push_str(cluster_text);
                    if cluster.is_word_boundary() && !cluster.is_space_or_nbsp() {
                        word_starts.push(character_lengths.len() as _);
                    }
                    let character_index = character_lengths.len();
                    character_lengths.push(cluster_text.len() as _);
                    character_positions.push(span_advance);
                    character_widths.push(cluster.advance());
                    span_advance += cluster.advance();
                    self.span_positions_by_cluster_path.insert(
                        cluster.path(),
                        SpanPosition { span_index: spans_started - 1, character_index },
                    );
                }

                if line_index + 1 == line_count
                    && Some(run_index) == last_run_index
                    && let Some(line_break) = line_break
                {
                    span_text.push_str(line_break);
                    character_lengths.push(line_break.len() as _);
                    character_positions.push(span_advance);
                    character_widths.push(0.0);
                }

                finish_span(
                    &mut node,
                    x_offset,
                    y_offset,
                    metrics,
                    run_offset,
                    span_offset,
                    span_advance,
                    span_text,
                    character_lengths,
                    character_positions,
                    character_widths,
                    word_starts,
                );
                last_node = Some((id, node));
            }

            if let Some((id, node)) = last_node {
                add_span(text_runs, parent_node, id, node);
            }
        }
    }

    /// Converts a position within the nodes built by [`Self::build_nodes`] into a [`Cursor`].
    pub fn cursor_from_access_position<B: Brush>(
        &self,
        pos: &TextPosition,
        layout: &Layout<B>,
    ) -> Option<Cursor> {
        let span_path = self.span_paths_by_access_id.get(&pos.node)?;
        let run = span_path.run(layout)?;
        let index = run
            .clusters()
            .skip_while(|cluster| cluster.path() != *span_path)
            .nth(pos.character_index)
            .map(|cluster| cluster.text_range().start)
            .unwrap_or(layout.text_len());
        Some(Cursor::from_byte_index(layout, index, Affinity::Downstream))
    }

    /// Converts a [`Cursor`] into a position within the nodes built by [`Self::build_nodes`].
    pub fn cursor_to_access_position<B: Brush>(
        &self,
        cursor: Cursor,
        layout: &Layout<B>,
    ) -> Option<TextPosition> {
        if layout.text_len() == 0 {
            let run = layout.get(0)?.runs().next()?;
            let position = self.span_positions_by_cluster_path.get(&run_start_path(0, &run))?;
            return Some(TextPosition {
                node: self.span_id(position.span_index)?,
                character_index: 0,
            });
        }
        // At the end of the text there is no downstream cluster, so take the upstream one
        // and step past it.
        let (offset, path) =
            cursor
                .downstream_cluster(layout)
                .map(|cluster| (0, cluster.path()))
                .or_else(|| cursor.upstream_cluster(layout).map(|cluster| (1, cluster.path())))?;
        // A cursor after a trailing newline belongs to the phantom line after it, which has
        // the geometry AccessKit needs.
        let (span_index, character_index) =
            if cursor.index() == layout.text_len() && ends_with_hard_line_break(layout) {
                let line_index = path.line_index() + 1;
                let run = layout.get(line_index)?.runs().next()?;
                let position =
                    self.span_positions_by_cluster_path.get(&run_start_path(line_index, &run))?;
                (position.span_index, 0)
            } else {
                let position = self.span_positions_by_cluster_path.get(&path)?;
                (position.span_index, position.character_index + offset)
            };
        Some(TextPosition { node: self.span_id(span_index)?, character_index })
    }

    fn span_id_and_node<B: Brush>(
        &mut self,
        next_node_id: &mut impl FnMut() -> NodeId,
        span_index: usize,
        run: &Run<'_, B>,
        alignment: Option<Alignment>,
        span_path: ClusterPath,
    ) -> (NodeId, Node) {
        let id = match self.span_ids.get(span_index) {
            Some(id) => *id,
            None => {
                let id = next_node_id();
                self.span_ids.push(id);
                id
            }
        };
        self.span_paths_by_access_id.insert(id, span_path);
        let mut node = Node::new(Role::TextRun);
        node.set_text_direction(if run.is_rtl() {
            TextDirection::RightToLeft
        } else {
            TextDirection::LeftToRight
        });

        let font = run.font();
        if let Ok(font_ref) = FontRef::from_index(font.data.as_ref(), font.index)
            && let Ok(name) = font_ref.name()
        {
            for n in name.name_record().iter() {
                if n.name_id.get() == NameId::FAMILY_NAME {
                    if let Ok(string) = n.string(name.string_data()) {
                        node.set_font_family(string.to_string());
                    }
                    break;
                }
            }
        }
        node.set_font_size(run.font_size());
        let attrs = run.font_attrs();
        node.set_font_weight(attrs.weight.value());
        if matches!(attrs.style, FontStyle::Italic) {
            node.set_italic();
        }
        if let Some(align) = alignment {
            node.set_text_align(match align {
                Alignment::Start if run.is_rtl() => TextAlign::Right,
                Alignment::Start => TextAlign::Left,
                Alignment::End if run.is_rtl() => TextAlign::Left,
                Alignment::End => TextAlign::Right,
                Alignment::Left => TextAlign::Left,
                Alignment::Center => TextAlign::Center,
                Alignment::Right => TextAlign::Right,
                Alignment::Justify => TextAlign::Justify,
            });
        }

        (id, node)
    }
}

/// Whether the layout has a trailing phantom line, which a cursor at the end of the text
/// belongs to.
fn ends_with_hard_line_break<B: Brush>(layout: &Layout<B>) -> bool {
    layout
        .text_len()
        .checked_sub(1)
        .and_then(|index| Cluster::from_byte_index(layout, index))
        .is_some_and(|cluster| cluster.is_hard_line_break())
}

/// The path of `run`'s first cluster. Empty runs have no clusters, so they get a path no
/// cluster can have.
fn run_start_path<B: Brush>(line_index: usize, run: &Run<'_, B>) -> ClusterPath {
    run.clusters()
        .next()
        .map(|cluster| cluster.path())
        .unwrap_or_else(|| ClusterPath::new(line_index as u32, run.index() as u32, 0))
}

fn link_spans(prev_id: NodeId, prev: &mut Node, next_id: NodeId, next: &mut Node) {
    prev.set_next_on_line(next_id);
    next.set_previous_on_line(prev_id);
}

#[allow(clippy::too_many_arguments)]
fn finish_span(
    node: &mut Node,
    x_offset: f64,
    y_offset: f64,
    metrics: &LineMetrics,
    run_offset: f32,
    span_offset: f32,
    span_advance: f32,
    span_text: String,
    character_lengths: Vec<u8>,
    character_positions: Vec<f32>,
    character_widths: Vec<f32>,
    word_starts: Vec<u8>,
) {
    node.set_bounds(Rect {
        x0: x_offset + (run_offset + span_offset) as f64,
        y0: y_offset + metrics.content_block_min_coord as f64,
        x1: x_offset + (run_offset + span_offset + span_advance) as f64,
        y1: y_offset + metrics.content_block_max_coord as f64,
    });
    node.set_value(span_text);
    node.set_character_lengths(character_lengths);
    node.set_character_positions(character_positions);
    node.set_character_widths(character_widths);
    node.set_word_starts(word_starts);
}

fn add_span(text_runs: &mut Vec<(NodeId, Node)>, parent_node: &mut Node, id: NodeId, node: Node) {
    text_runs.push((id, node));
    parent_node.push_child(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(
        text: &str,
        line_break: Option<&str>,
    ) -> (Layout<()>, LayoutAccessibility, Vec<(NodeId, Node)>) {
        let mut font_ctx = crate::textlayout::sharedparley::tests::test_font_context();
        let mut layout_ctx = parley::LayoutContext::new();
        let mut layout = layout_ctx.ranged_builder(&mut font_ctx, text, 1.0, true).build(text);
        layout.break_all_lines(None);

        let mut access = LayoutAccessibility::default();
        let mut text_runs = Vec::new();
        let mut next_id = 1;
        access.build_nodes(
            text,
            &layout,
            &mut text_runs,
            &mut Node::new(Role::TextInput),
            || {
                next_id += 1;
                NodeId(next_id)
            },
            0.0,
            0.0,
            line_break,
        );
        (layout, access, text_runs)
    }

    #[test]
    fn line_break_ends_the_last_span() {
        let (layout, access, text_runs) = build("ab", Some("\r\n"));
        let (id, node) = text_runs.last().unwrap();
        assert_eq!(node.value(), Some("ab\r\n"));
        assert_eq!(node.character_lengths(), [1, 1, 2]);
        let widths = node.character_widths().unwrap();
        assert_eq!(widths[2], 0.0);
        assert_eq!(node.character_positions().unwrap()[2], widths[0] + widths[1]);

        // The end of the paragraph is the position just before the line break.
        let end = Cursor::from_byte_index(&layout, 2, Affinity::Downstream);
        let position = access.cursor_to_access_position(end, &layout).unwrap();
        assert_eq!(position, TextPosition { node: *id, character_index: 2 });
        assert_eq!(access.cursor_from_access_position(&position, &layout).unwrap().index(), 2);
    }

    #[test]
    fn line_break_in_an_empty_paragraph() {
        let (_, _, text_runs) = build("", Some("\n"));
        let (_, node) = text_runs.last().unwrap();
        assert_eq!(node.value(), Some("\n"));
        assert_eq!(node.character_lengths(), [1]);
    }

    #[test]
    fn no_line_break_after_the_last_paragraph() {
        let (_, _, text_runs) = build("ab", None);
        let (_, node) = text_runs.last().unwrap();
        assert_eq!(node.value(), Some("ab"));
        assert_eq!(node.character_lengths(), [1, 1]);
    }
}
