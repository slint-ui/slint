// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ops::RangeInclusive;

use i_slint_compiler::source_path::SourcePath;
use i_slint_core::{
    graphics::euclid::approxeq::ApproxEq,
    lengths::{ItemTransform, LogicalPoint, LogicalRect, LogicalSize, LogicalVector},
};
use slint::ComponentHandle;
use slint_interpreter::snapping::OccurrenceRelationship;

use super::ui;

const TARGET_MERGE_EPSILON: f32 = 0.01;
const SNAP_DISTANCE: f32 = 6.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SnapAxis {
    X,
    Y,
}

impl SnapAxis {
    fn coordinate(self, point: LogicalPoint) -> f32 {
        match self {
            Self::X => point.x,
            Self::Y => point.y,
        }
    }

    fn correction(self, amount: f32) -> LogicalVector {
        match self {
            Self::X => LogicalVector::new(amount, 0.),
            Self::Y => LogicalVector::new(0., amount),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SnapFeature {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
    Center,
}

#[derive(Clone, Debug)]
struct SnapTarget {
    axis: SnapAxis,
    position: f32,
    extent: RangeInclusive<f32>,
    path: SourcePath,
    offset: u32,
    instance_index: usize,
    features: Vec<SnapFeature>,
}

impl SnapTarget {
    fn matches(&self, point: LogicalPoint) -> bool {
        self.axis.coordinate(point).approx_eq_eps(&self.position, &TARGET_MERGE_EPSILON)
    }
}

struct SnapMatch {
    target_index: usize,
    feature_index: usize,
    distance_squared: f32,
    rect: LogicalRect,
}

struct SnapTargets {
    parent_to_root: ItemTransform,
    targets: Vec<SnapTarget>,
}

fn collect_targets(path: &SourcePath, offset: u32, instance_index: usize) -> Option<SnapTargets> {
    let instance = super::component_instance()?;
    let geometry =
        instance.snapping_geometry(path, offset, instance_index, i_slint_core::InternalToken)?;
    let mut targets: Vec<SnapTarget> = Vec::new();
    for occurrence in geometry.occurrences {
        if !matches!(
            occurrence.relationship,
            OccurrenceRelationship::Parent | OccurrenceRelationship::Sibling
        ) {
            continue;
        }
        let target_start = targets.len();
        let center = occurrence.corners[0].lerp(occurrence.corners[2], 0.5);
        let points = occurrence
            .corners
            .into_iter()
            .zip([
                SnapFeature::TopLeft,
                SnapFeature::TopRight,
                SnapFeature::BottomRight,
                SnapFeature::BottomLeft,
            ])
            .chain(std::iter::once((center, SnapFeature::Center)));
        for (point, feature) in points {
            for axis in [SnapAxis::X, SnapAxis::Y] {
                let (position, extent) = match axis {
                    SnapAxis::X => (point.x, point.y),
                    SnapAxis::Y => (point.y, point.x),
                };
                if let Some(target) = targets[target_start..].iter_mut().find(|target| {
                    target.axis == axis
                        && target.position.approx_eq_eps(&position, &TARGET_MERGE_EPSILON)
                }) {
                    target.extent =
                        target.extent.start().min(extent)..=target.extent.end().max(extent);
                    target.features.push(feature);
                } else {
                    targets.push(SnapTarget {
                        axis,
                        position,
                        extent: extent..=extent,
                        path: occurrence.path.clone(),
                        offset: occurrence.offset,
                        instance_index: occurrence.instance_index,
                        features: vec![feature],
                    });
                }
            }
        }
        for target in &mut targets[target_start..] {
            if target.extent.start() == target.extent.end() {
                target.extent = (target.extent.start() - 1.)..=(target.extent.end() + 1.);
            }
        }
    }
    Some(SnapTargets { parent_to_root: geometry.parent_to_root, targets })
}

pub fn setup(editor_ui: &ui::EditorUi) {
    let snapping = editor_ui.global::<ui::Snapping>();
    snapping.on_snap_move(snap_move);
    let minimum_square_size = editor_ui.global::<ui::Api>().get_minimum_square_resize_size();
    snapping.on_snap_resize(move |geometry, handle, square, zoom| {
        snap_resize(geometry, handle, square, zoom, minimum_square_size)
    });
}

#[derive(Clone, Copy)]
struct GeometryDirection {
    origin: LogicalVector,
    size: LogicalSize,
}

impl GeometryDirection {
    fn apply(self, rect: LogicalRect, amount: f32) -> LogicalRect {
        LogicalRect::new(rect.origin + self.origin * amount, rect.size + self.size * amount)
    }
}

fn selected_geometry() -> Option<slint_interpreter::highlight::HighlightedRect> {
    let selection = super::selected_element()?;
    let element = selection.as_element_node()?;
    let instance = super::component_instance()?;
    use super::ext::ElementRcNodeExt;
    element
        .geometries(&instance)
        .get(selection.instance_index)
        .copied()
        .filter(|geometry| geometry.renders_as_rectangle)
}

fn selected_targets() -> Option<SnapTargets> {
    let selection = super::selected_element()?;
    collect_targets(&selection.path, selection.offset.into(), selection.instance_index)
}

fn logical_rect(geometry: &ui::Geometry) -> LogicalRect {
    LogicalRect::new(
        LogicalPoint::new(geometry.x, geometry.y),
        LogicalSize::new(geometry.width, geometry.height),
    )
}

fn ui_geometry(rect: LogicalRect) -> ui::Geometry {
    ui::Geometry { x: rect.origin.x, y: rect.origin.y, width: rect.width(), height: rect.height() }
}

fn frame_points(
    geometry: &slint_interpreter::highlight::HighlightedRect,
    rect: LogicalRect,
) -> Option<[LogicalPoint; 5]> {
    let to_parent = geometry.parent_transform.inverse()?;
    let rotation = geometry.angle.to_radians();
    let width_axis = LogicalVector::new(
        rotation.cos() * geometry.rect.width() / geometry.local_rect.width(),
        rotation.sin() * geometry.rect.width() / geometry.local_rect.width(),
    );
    let height_axis = LogicalVector::new(
        -rotation.sin() * geometry.rect.height() / geometry.local_rect.height(),
        rotation.cos() * geometry.rect.height() / geometry.local_rect.height(),
    );
    let width_axis = to_parent.transform_vector(width_axis.cast()).cast();
    let height_axis = to_parent.transform_vector(height_axis.cast()).cast();
    let center = to_parent.transform_point(geometry.rect.center().cast()).cast()
        + (rect.center() - geometry.local_rect.center());
    Some([(0., 0.), (1., 0.), (1., 1.), (0., 1.), (0.5, 0.5)].map(|(horizontal, vertical)| {
        center
            + width_axis * ((horizontal - 0.5) * rect.width())
            + height_axis * ((vertical - 0.5) * rect.height())
    }))
}

fn snap_resize(
    parent_geometry: ui::Geometry,
    handle: ui::ResizeHandle,
    square: bool,
    zoom: f32,
    minimum_square_size: f32,
) -> ui::SnapResult {
    let fallback =
        ui::SnapResult { parent_geometry: parent_geometry.clone(), ..Default::default() };
    let Some(geometry) = selected_geometry() else { return fallback };
    let Some(points) = frame_points(&geometry, geometry.local_rect) else { return fallback };
    // Unlike moving, resizing only snaps the dragged corner or the dragged edge's endpoints.
    // Either endpoint measures the same correction for an edge resize.
    let (left, top, eligible_features, moving_feature): (_, _, &[usize], _) = match handle {
        ui::ResizeHandle::TopLeft => (true, true, &[0], 0),
        ui::ResizeHandle::TopRight => (false, true, &[1], 1),
        ui::ResizeHandle::BottomRight => (false, false, &[2], 2),
        ui::ResizeHandle::BottomLeft => (true, false, &[3], 3),
        ui::ResizeHandle::Top => (true, true, &[0, 1], 0),
        ui::ResizeHandle::Right => (false, true, &[1, 2], 1),
        ui::ResizeHandle::Bottom => (true, false, &[2, 3], 3),
        ui::ResizeHandle::Left => (true, true, &[0, 3], 0),
    };
    // Changing a rotated size also shifts the source origin to keep the opposite corner or edge fixed.
    let horizontal = GeometryDirection {
        origin: (points[1] - points[0]) / geometry.local_rect.width()
            * (if left { -0.5 } else { 0.5 })
            - LogicalVector::new(0.5, 0.),
        size: LogicalSize::new(1., 0.),
    };
    let vertical = GeometryDirection {
        origin: (points[3] - points[0]) / geometry.local_rect.height()
            * (if top { -0.5 } else { 0.5 })
            - LogicalVector::new(0., 0.5),
        size: LogicalSize::new(0., 1.),
    };
    // Edge and square resizes have one independent size change; free corners have two.
    let (directions, dimensions) = if square {
        // Compensate for scaling so width and height remain equal in the rendered square.
        let ratio = (geometry.rect.width() / geometry.local_rect.width())
            / (geometry.rect.height() / geometry.local_rect.height());
        (
            [
                GeometryDirection {
                    origin: horizontal.origin + vertical.origin * ratio,
                    size: horizontal.size + vertical.size * ratio,
                },
                vertical,
            ],
            1,
        )
    } else {
        match handle {
            ui::ResizeHandle::Top | ui::ResizeHandle::Bottom => ([vertical, horizontal], 1),
            ui::ResizeHandle::Left | ui::ResizeHandle::Right => ([horizontal, vertical], 1),
            _ => ([horizontal, vertical], 2),
        }
    };
    let minimum_size = if square { minimum_square_size } else { 0. };
    let mut rect = logical_rect(&parent_geometry);
    // Round the free size parameters before snapping while preserving the fixed anchor and resize constraint.
    // Snapped sizes retain fractional values.
    if square {
        let scale = geometry.rect.width() / geometry.local_rect.width();
        // Round up the source minimum so scaling cannot shrink the rendered square below its limit.
        let source_width = rect.width().round().max((minimum_size / scale).ceil());
        rect = directions[0].apply(rect, source_width - rect.width());
    } else {
        if dimensions == 1 {
            // Coordinate conversion can perturb the fixed dimension; restore it for edge drags.
            let unchanged_size = if directions[0].size.width != 0. {
                geometry.local_rect.height() - rect.height()
            } else {
                geometry.local_rect.width() - rect.width()
            };
            rect = directions[1].apply(rect, unchanged_size);
        }
        for direction in &directions[..dimensions] {
            let size = if direction.size.width != 0. { rect.width() } else { rect.height() };
            rect = direction.apply(rect, size.round() - size);
        }
    }
    let Some(targets) = selected_targets() else {
        return ui::SnapResult { parent_geometry: ui_geometry(rect), ..fallback };
    };
    let tolerance = SNAP_DISTANCE / zoom;
    let Some(points) = frame_points(&geometry, rect) else { return fallback };
    // Measure how a one-unit size change moves each point in parent coordinates.
    // Rotation can make one size affect both snapping axes.
    let direction_points =
        directions.map(|direction| frame_points(&geometry, direction.apply(rect, 1.)));
    let [Some(first_direction_points), Some(second_direction_points)] = direction_points else {
        return fallback;
    };
    let direction_points = [first_direction_points, second_direction_points];
    let response = |feature_index: usize, axis: SnapAxis| {
        [0, 1].map(|index| {
            axis.coordinate(direction_points[index][feature_index])
                - axis.coordinate(points[feature_index])
        })
    };
    // Existing alignment on an axis this handle cannot change must not produce a snapping guide.
    let eligible = |feature_index: usize, target: &SnapTarget| {
        eligible_features.contains(&feature_index)
            && response(feature_index, target.axis)[..dimensions]
                .iter()
                .any(|coefficient| coefficient.is_normal())
    };
    // Check feasibility before ranking so an unreachable target cannot hide a usable one.
    let apply_parameters = |parameters: [f32; 2]| {
        let candidate =
            directions[1].apply(directions[0].apply(rect, parameters[0]), parameters[1]);
        let rendered_width =
            candidate.width() * geometry.rect.width() / geometry.local_rect.width();
        let rendered_height =
            candidate.height() * geometry.rect.height() / geometry.local_rect.height();
        if !candidate.origin.x.is_finite()
            || !candidate.origin.y.is_finite()
            || !rendered_width.is_finite()
            || !rendered_height.is_finite()
            || candidate.width() <= 0.
            || candidate.height() <= 0.
            || rendered_width <= 0.
            || rendered_height <= 0.
            || rendered_width < minimum_size
            || rendered_height < minimum_size
        {
            return None;
        }
        let candidate_points = frame_points(&geometry, candidate)?;
        // A small axis gap can require a large handle movement for a nearly parallel rotated edge.
        within_tolerance(
            &targets.parent_to_root,
            candidate_points[moving_feature] - points[moving_feature],
            tolerance,
        )
        .then_some(candidate)
    };
    // Rank both axes from the same unsnapped proposal, considering only reachable targets.
    let matches = [SnapAxis::X, SnapAxis::Y].map(|axis| {
        closest_target(&targets, &points, axis, tolerance, |feature_index, target| {
            if !eligible(feature_index, target) {
                return None;
            }
            let coefficients = response(feature_index, axis);
            let distance = target.position - axis.coordinate(points[feature_index]);
            // One size change can move both coordinates; two sizes let this candidate hold the other coordinate fixed.
            let parameters = if dimensions == 1 {
                [distance / coefficients[0], 0.]
            } else {
                let other_axis = match axis {
                    SnapAxis::X => SnapAxis::Y,
                    SnapAxis::Y => SnapAxis::X,
                };
                solve_parameters(coefficients, response(feature_index, other_axis), distance, 0.)?
            };
            let candidate = apply_parameters(parameters)?;
            let candidate_points = frame_points(&geometry, candidate)?;
            target.matches(candidate_points[feature_index]).then_some(candidate)
        })
    });
    // If the winners conflict, the better proximity score decides which target to satisfy.
    let mut best_rect = matches
        .iter()
        .flatten()
        .min_by(|first, second| first.distance_squared.total_cmp(&second.distance_squared))
        .map_or(rect, |selected_match| selected_match.rect);
    // A free corner can satisfy both winners by solving for width and height together.
    // Keep the better single winner if their combination violates the resize limits.
    if dimensions == 2
        && let [Some(first), Some(second)] = &matches
    {
        let first_target = &targets.targets[first.target_index];
        let second_target = &targets.targets[second.target_index];
        let parameters = solve_parameters(
            response(first.feature_index, first_target.axis),
            response(second.feature_index, second_target.axis),
            first_target.position - first_target.axis.coordinate(points[first.feature_index]),
            second_target.position - second_target.axis.coordinate(points[second.feature_index]),
        );
        if let Some(candidate) = parameters.and_then(apply_parameters)
            && let Some(candidate_points) = frame_points(&geometry, candidate)
            && first_target.matches(candidate_points[first.feature_index])
            && second_target.matches(candidate_points[second.feature_index])
        {
            best_rect = candidate;
        }
    }
    let Some(points) = frame_points(&geometry, best_rect) else { return fallback };
    // For a one-parameter resize, both guides survive only if the chosen size satisfies both targets.
    snap_result(best_rect, &targets, &matches, &points, eligible)
}

fn solve_parameters(
    first: [f32; 2],
    second: [f32; 2],
    first_distance: f32,
    second_distance: f32,
) -> Option<[f32; 2]> {
    let determinant = first[0] * second[1] - first[1] * second[0];
    determinant.is_normal().then(|| {
        [
            (first_distance * second[1] - first[1] * second_distance) / determinant,
            (first[0] * second_distance - first_distance * second[0]) / determinant,
        ]
    })
}

fn snap_move(parent_geometry: ui::Geometry, zoom: f32) -> ui::SnapResult {
    let mut fallback =
        ui::SnapResult { parent_geometry: parent_geometry.clone(), ..Default::default() };
    let Some(geometry) = selected_geometry() else { return fallback };
    let rect = LogicalRect::new(
        LogicalPoint::new(parent_geometry.x.round(), parent_geometry.y.round()),
        geometry.local_rect.size,
    );
    fallback.parent_geometry = ui_geometry(rect);
    let Some(targets) = selected_targets() else { return fallback };
    let Some(points) = frame_points(&geometry, rect) else { return fallback };
    let tolerance = SNAP_DISTANCE / zoom;
    let matches = [SnapAxis::X, SnapAxis::Y].map(|axis| {
        closest_target(&targets, &points, axis, tolerance, |feature_index, target| {
            let amount = target.position - axis.coordinate(points[feature_index]);
            Some(LogicalRect::new(rect.origin + axis.correction(amount), rect.size))
        })
    });
    let correction = LogicalVector::new(
        matches[0]
            .as_ref()
            .map_or(0., |selected_match| selected_match.rect.origin.x - rect.origin.x),
        matches[1]
            .as_ref()
            .map_or(0., |selected_match| selected_match.rect.origin.y - rect.origin.y),
    );
    snap_result(
        LogicalRect::new(rect.origin + correction, rect.size),
        &targets,
        &matches,
        &points.map(|point| point + correction),
        |_, _| true,
    )
}

fn within_tolerance(
    parent_to_root: &ItemTransform,
    correction: LogicalVector,
    tolerance: f32,
) -> bool {
    [LogicalVector::new(correction.x, 0.), LogicalVector::new(0., correction.y)].into_iter().all(
        |axis_correction| {
            parent_to_root.transform_vector(axis_correction.cast()).length() as f32 <= tolerance
        },
    )
}

fn closest_target(
    targets: &SnapTargets,
    points: &[LogicalPoint; 5],
    axis: SnapAxis,
    tolerance: f32,
    candidate: impl Fn(usize, &SnapTarget) -> Option<LogicalRect>,
) -> Option<SnapMatch> {
    let mut best_match: Option<SnapMatch> = None;
    for (target_index, target) in
        targets.targets.iter().enumerate().filter(|(_, target)| target.axis == axis)
    {
        let point_at = |extent| match axis {
            SnapAxis::X => LogicalPoint::new(target.position, extent),
            SnapAxis::Y => LogicalPoint::new(extent, target.position),
        };
        let start = point_at(*target.extent.start());
        let segment = targets
            .parent_to_root
            .transform_vector((point_at(*target.extent.end()) - start).cast());
        let segment_length_squared = segment.square_length();
        if !segment_length_squared.is_normal() {
            continue;
        }
        for (feature_index, point) in points.iter().copied().enumerate() {
            let amount = target.position - axis.coordinate(point);
            if !within_tolerance(&targets.parent_to_root, axis.correction(amount), tolerance) {
                continue;
            }
            let Some(rect) = candidate(feature_index, target) else { continue };
            let offset = targets.parent_to_root.transform_vector((point - start).cast());
            let projection = (offset.dot(segment) / segment_length_squared).clamp(0., 1.);
            let distance_squared = (offset - segment * projection).square_length() as f32;
            if best_match.as_ref().is_none_or(|current| distance_squared < current.distance_squared)
            {
                best_match =
                    Some(SnapMatch { target_index, feature_index, distance_squared, rect });
            }
        }
    }
    best_match
}

fn snap_result(
    rect: LogicalRect,
    targets: &SnapTargets,
    matches: &[Option<SnapMatch>; 2],
    points: &[LogicalPoint; 5],
    eligible: impl Fn(usize, &SnapTarget) -> bool,
) -> ui::SnapResult {
    let guides = matches
        .iter()
        .flatten()
        .filter_map(|selected_match| {
            let target = &targets.targets[selected_match.target_index];
            if !target.matches(points[selected_match.feature_index]) {
                return None;
            }
            let extent = points
                .iter()
                .enumerate()
                .filter(|(feature_index, point)| {
                    eligible(*feature_index, target) && target.matches(**point)
                })
                .fold(target.extent.clone(), |extent, (_, point)| {
                    let position = match target.axis {
                        SnapAxis::X => point.y,
                        SnapAxis::Y => point.x,
                    };
                    extent.start().min(position)..=extent.end().max(position)
                });
            Some(target_guide(target, extent, &targets.parent_to_root))
        })
        .collect::<Vec<_>>();
    ui::SnapResult {
        parent_geometry: ui_geometry(rect),
        guides: std::rc::Rc::new(slint::VecModel::from(guides)).into(),
    }
}

fn target_guide(
    target: &SnapTarget,
    extent: RangeInclusive<f32>,
    parent_to_root: &ItemTransform,
) -> ui::SnapGuide {
    let endpoints = [*extent.start(), *extent.end()].map(|extent| {
        let point = match target.axis {
            SnapAxis::X => LogicalPoint::new(target.position, extent),
            SnapAxis::Y => LogicalPoint::new(extent, target.position),
        };
        parent_to_root.transform_point(point.cast()).cast::<f32>()
    });
    ui::SnapGuide {
        start_x: endpoints[0].x,
        start_y: endpoints[0].y,
        end_x: endpoints[1].x,
        end_y: endpoints[1].y,
        label: format!(
            "Snap {:?} {:?} {}:{} [{}]",
            target.axis, target.features, target.path, target.offset, target.instance_index
        )
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{PREVIEW_STATE, element_selection::ElementSelection};

    const SOURCE: &str = r#"
component Tile inherits Rectangle {}
export component Main inherits Window {
    width: 600px;
    height: 400px;
    container := Rectangle {
        x: 40px; y: 50px; width: 300px; height: 200px;
        selected := Rectangle {
            x: 20px; y: 30px; width: 40px; height: 30px;
            descendant := Rectangle { x: 2px; y: 3px; width: 4px; height: 5px; }
        }
        sibling := Rectangle { x: 90px; y: 60px; width: 50px; height: 20px; }
        for index in 2: repeated := Rectangle {
            x: 10px + index * 60px; y: 120px; width: 30px; height: 20px;
        }
        tile := Tile { x: 180px; y: 80px; width: 40px; height: 30px; }
        other_tile := Tile { x: 240px; y: 30px; width: 20px; height: 40px; }
    }
    unrelated := Rectangle { x: 400px; y: 20px; width: 50px; height: 60px; }
}
"#;

    fn offset(id: &str) -> u32 {
        let declaration = SOURCE.find(&format!("{id} := ")).expect("element declaration");
        (declaration + format!("{id} := ").len()) as u32
    }

    fn install_selection(id: &str, instance_index: usize) -> slint_interpreter::ComponentInstance {
        let instance = crate::preview::test::interpret_test("fluent", SOURCE);
        PREVIEW_STATE.with_borrow_mut(|state| {
            *state = Default::default();
            state.handle.replace(Some(instance.clone_strong()));
            state.selected = Some(ElementSelection {
                path: i_slint_editor_preview::test::main_test_file_name(),
                offset: offset(id).into(),
                instance_index,
                editing_text: false,
            });
        });
        instance
    }

    fn assert_rectangle(targets: &[&SnapTarget], origin: (f32, f32), size: (f32, f32)) {
        assert_eq!(targets.len(), 6, "{targets:?}");
        for (axis, position, extent, features) in [
            (
                SnapAxis::X,
                origin.0,
                origin.1..=origin.1 + size.1,
                vec![SnapFeature::TopLeft, SnapFeature::BottomLeft],
            ),
            (
                SnapAxis::X,
                origin.0 + size.0,
                origin.1..=origin.1 + size.1,
                vec![SnapFeature::TopRight, SnapFeature::BottomRight],
            ),
            (
                SnapAxis::Y,
                origin.1,
                origin.0..=origin.0 + size.0,
                vec![SnapFeature::TopLeft, SnapFeature::TopRight],
            ),
            (
                SnapAxis::Y,
                origin.1 + size.1,
                origin.0..=origin.0 + size.0,
                vec![SnapFeature::BottomRight, SnapFeature::BottomLeft],
            ),
            (
                SnapAxis::X,
                origin.0 + size.0 / 2.,
                origin.1 + size.1 / 2. - 1. ..=origin.1 + size.1 / 2. + 1.,
                vec![SnapFeature::Center],
            ),
            (
                SnapAxis::Y,
                origin.1 + size.1 / 2.,
                origin.0 + size.0 / 2. - 1. ..=origin.0 + size.0 / 2. + 1.,
                vec![SnapFeature::Center],
            ),
        ] {
            let target = targets
                .iter()
                .find(|target| target.axis == axis && target.position == position)
                .expect("rectangle target");
            assert_eq!(target.extent, extent);
            assert_eq!(target.features, features);
        }
    }

    #[test]
    fn targets_include_only_parent_and_siblings() {
        let _instance = install_selection("selected", 0);
        let targets = selected_targets().expect("selected targets");
        assert_eq!(targets.targets.len(), 36);
        let path = i_slint_editor_preview::test::main_test_file_name();
        assert!(targets.targets.iter().all(|target| target.path == path));
        for (id, origin, size) in
            [("container", (0., 0.), (300., 200.)), ("sibling", (90., 60.), (50., 20.))]
        {
            let rectangle = targets
                .targets
                .iter()
                .filter(|target| target.offset == offset(id))
                .collect::<Vec<_>>();
            assert_rectangle(&rectangle, origin, size);
        }
        for id in ["selected", "descendant", "unrelated"] {
            assert!(targets.targets.iter().all(|target| target.offset != offset(id)), "{id}");
        }
        assert_eq!(
            targets.parent_to_root.transform_point(LogicalPoint::zero().cast()).cast::<f32>(),
            LogicalPoint::new(40., 50.)
        );
    }

    #[test]
    fn repeated_targets_have_live_coordinates_and_exclude_all_selected_occurrences() {
        let _instance = install_selection("selected", 0);
        let path = i_slint_editor_preview::test::main_test_file_name();
        let targets = collect_targets(&path, offset("selected"), 0).expect("targets");
        for instance_index in 0..2 {
            let rectangle = targets
                .targets
                .iter()
                .filter(|target| {
                    target.offset == offset("repeated") && target.instance_index == instance_index
                })
                .collect::<Vec<_>>();
            assert_rectangle(&rectangle, (10. + instance_index as f32 * 60., 120.), (30., 20.));
            let selected = collect_targets(&path, offset("repeated"), instance_index)
                .expect("repeated selection targets");
            assert!(selected.targets.iter().all(|target| target.offset != offset("repeated")));
            assert_eq!(selected.targets.len(), 30);
        }
    }

    #[test]
    fn component_source_and_use_site_share_one_runtime_target() {
        let instance = install_selection("selected", 0);
        let path = i_slint_editor_preview::test::main_test_file_name();
        let definition_offset = SOURCE.find("Rectangle {}").unwrap() as u32;
        let geometries = instance.component_positions(&path, definition_offset);
        assert_eq!(geometries.len(), 2);
        let targets = selected_targets().expect("targets");
        for (instance_index, id, origin, size) in
            [(0, "tile", (180., 80.), (40., 30.)), (1, "other_tile", (240., 30.), (20., 40.))]
        {
            assert_eq!(
                geometries[instance_index].local_rect.origin,
                LogicalPoint::new(origin.0, origin.1)
            );
            assert_eq!(instance.component_positions(&path, offset(id)).len(), 1);
            let rectangle = targets
                .targets
                .iter()
                .filter(|target| {
                    target.offset == offset(id)
                        || target.offset == definition_offset
                            && target.instance_index == instance_index
                })
                .collect::<Vec<_>>();
            assert_rectangle(&rectangle, origin, size);
            let selected =
                collect_targets(&path, offset(id), 0).expect("use site selection targets");
            assert!(selected.targets.iter().all(|target| target.offset != offset(id)
                && !(target.offset == definition_offset
                    && target.instance_index == instance_index)));
            assert_eq!(selected.targets.len(), 36);
            let selected = collect_targets(&path, definition_offset, instance_index)
                .expect("component source selection targets");
            assert!(selected.targets.iter().all(|target| {
                ![definition_offset, offset("tile"), offset("other_tile")].contains(&target.offset)
            }));
            assert_eq!(selected.targets.len(), 30);
        }
    }
}
