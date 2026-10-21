// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ops::RangeInclusive;

use i_slint_compiler::source_path::SourcePath;
use i_slint_core::{
    graphics::euclid::approxeq::ApproxEq,
    lengths::{ItemTransform, LogicalPoint},
};
use slint::ComponentHandle;
use slint_interpreter::snapping::OccurrenceRelationship;

use super::ui;

const TARGET_MERGE_EPSILON: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SnapAxis {
    X,
    Y,
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
    editor_ui.global::<ui::Snapping>().on_guides(guides);
}

fn guides(
    source_uri: slint::SharedString,
    offset: i32,
    instance_index: i32,
) -> slint::ModelRc<ui::SnapGuide> {
    super::PREVIEW_STATE.with_borrow(|state| {
        if let Some(api) = state.api.upgrade() {
            api.get_inspector_generation();
        }
    });
    let Some(path) = lsp_types::Url::parse(source_uri.as_str())
        .ok()
        .map(i_slint_compiler::source_path::SourcePath::from)
    else {
        return Default::default();
    };
    let Ok(instance_index) = usize::try_from(instance_index) else {
        return Default::default();
    };
    let Some(targets) = collect_targets(&path, offset as u32, instance_index) else {
        return Default::default();
    };
    let guides = targets
        .targets
        .iter()
        .map(|target| {
            let endpoints = [*target.extent.start(), *target.extent.end()].map(|extent| {
                let point = match target.axis {
                    SnapAxis::X => LogicalPoint::new(target.position, extent),
                    SnapAxis::Y => LogicalPoint::new(extent, target.position),
                };
                targets.parent_to_root.transform_point(point.cast()).cast::<f32>()
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
        })
        .collect::<Vec<_>>();
    std::rc::Rc::new(slint::VecModel::from(guides)).into()
}
