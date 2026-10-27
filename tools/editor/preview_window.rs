// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::OsString;

use slint::Window;
use slint::winit_030::{WinitWindowAccessor, winit};

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Placement {
    anchor: (f64, f64),
    bounds_origin: (f64, f64),
    bounds_size: (f64, f64),
}

fn placement(editor: &Window) -> Option<Placement> {
    editor.with_winit_window(|window| {
        let monitor = window.current_monitor()?;
        Some(Placement::from_physical(
            window.outer_position().ok()?,
            window.outer_size(),
            window.scale_factor(),
            monitor.position(),
            monitor.size(),
            monitor.scale_factor(),
            cfg!(target_os = "macos"),
        ))
    })?
}

pub fn arguments(placement: Option<Placement>) -> Vec<OsString> {
    let mut arguments = vec![OsString::from("--run-preview-child")];
    if let Some(placement) = placement {
        arguments.push(
            format!("--preview-window-placement={}", serde_json::to_string(&placement).unwrap())
                .into(),
        );
    }
    arguments
}

pub fn arguments_provider(
    editor: slint::Weak<crate::preview::ui::EditorUi>,
) -> i_slint_editor_preview::springboard::LocalPreviewArguments {
    std::sync::Arc::new(move || {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let _ = editor.upgrade_in_event_loop(move |editor| {
            use slint::ComponentHandle;
            let _ = sender.send(arguments(placement(editor.window())));
        });
        receiver
    })
}

pub fn activation_callback() -> Option<i_slint_editor_preview::child_process::PreviewReadyCallback>
{
    #[cfg(target_os = "macos")]
    if !crate::headless_requested() {
        return Some(std::sync::Arc::new(|process_id| {
            let (ready, completed) = tokio::sync::oneshot::channel();
            let _ = slint::invoke_from_event_loop(move || {
                use objc2::{MainThreadMarker, runtime::NSObjectProtocol, sel};
                use objc2_app_kit::{NSApplication, NSRunningApplication};
                let application =
                    NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
                let preview = NSRunningApplication::runningApplicationWithProcessIdentifier(
                    process_id as i32,
                );
                // macOS 14 requires the active app to yield before another app requests activation.
                if application.respondsToSelector(sel!(yieldActivationToApplication:))
                    && let Some(preview) = preview
                {
                    application.yieldActivationToApplication(&preview);
                }
                let _ = ready.send(());
            });
            completed
        }));
    }
    None
}

pub fn run(placement: Option<Placement>) -> i_slint_editor_preview::Result<()> {
    let headless = crate::headless_requested();
    if !headless {
        let selector = slint::BackendSelector::new();
        #[cfg(target_os = "macos")]
        let selector = {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            let mut builder = winit::event_loop::EventLoop::with_user_event();
            // AppKit activates a process before its asynchronously compiled window exists.
            builder.with_activate_ignoring_other_apps(false);
            selector.with_winit_event_loop_builder(builder)
        };
        selector.select()?;
    }
    i_slint_editor_preview::child_process::run_with_window_setup(async move |window| {
        if headless {
            return;
        }
        if let Some(placement) = placement {
            #[cfg(target_os = "macos")]
            window.set_position(slint::LogicalPosition::new(
                placement.bounds_origin.0 as f32,
                placement.bounds_origin.1 as f32,
            ));
            #[cfg(not(target_os = "macos"))]
            window.set_position(slint::PhysicalPosition::new(
                placement.bounds_origin.0.round() as i32,
                placement.bounds_origin.1.round() as i32,
            ));
        }
        let Ok(native) = window.winit_window().await else { return };
        if let Some(placement) = placement {
            let size = native.outer_size();
            let coordinate_scale =
                if cfg!(target_os = "macos") { native.scale_factor() } else { 1. };
            let size = size.to_logical::<f64>(coordinate_scale);
            let position = placement.position((size.width, size.height));
            #[cfg(target_os = "macos")]
            native.set_outer_position(winit::dpi::LogicalPosition::new(position.0, position.1));
            #[cfg(not(target_os = "macos"))]
            native.set_outer_position(winit::dpi::PhysicalPosition::new(
                position.0.round() as i32,
                position.1.round() as i32,
            ));
        }
        let mut presented = false;
        window.on_winit_window_event(move |window, event| {
            if !presented && matches!(event, winit::event::WindowEvent::RedrawRequested) {
                presented = true;
                window.with_winit_window(|native| native.focus_window());
            }
            slint::winit_030::EventResult::Propagate
        });
    })
}

impl Placement {
    fn from_physical(
        editor_origin: winit::dpi::PhysicalPosition<i32>,
        editor_size: winit::dpi::PhysicalSize<u32>,
        editor_scale: f64,
        monitor_origin: winit::dpi::PhysicalPosition<i32>,
        monitor_size: winit::dpi::PhysicalSize<u32>,
        monitor_scale: f64,
        logical: bool,
    ) -> Self {
        // AppKit global positions are points; winit physical positions use the queried screen's scale.
        let editor_coordinate_scale = if logical { editor_scale } else { 1. };
        let monitor_coordinate_scale = if logical { monitor_scale } else { 1. };
        let origin = editor_origin.to_logical::<f64>(editor_coordinate_scale);
        let size = editor_size.to_logical::<f64>(editor_coordinate_scale);
        let bounds_origin = monitor_origin.to_logical::<f64>(monitor_coordinate_scale);
        let bounds_size = monitor_size.to_logical::<f64>(monitor_coordinate_scale);
        let inset = 24. * editor_scale / editor_coordinate_scale;
        Self {
            anchor: (origin.x + size.width - inset, origin.y + 2. * inset),
            bounds_origin: (bounds_origin.x + inset, bounds_origin.y + inset),
            bounds_size: (
                (bounds_size.width - 2. * inset).max(0.),
                (bounds_size.height - 2. * inset).max(0.),
            ),
        }
    }

    fn position(self, size: (f64, f64)) -> (f64, f64) {
        (
            (self.anchor.0 - size.0).clamp(
                self.bounds_origin.0,
                self.bounds_origin.0 + (self.bounds_size.0 - size.0).max(0.),
            ),
            self.anchor.1.clamp(
                self.bounds_origin.1,
                self.bounds_origin.1 + (self.bounds_size.1 - size.1).max(0.),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::{LogicalPosition, LogicalSize};

    #[test]
    fn launch_arguments_round_trip_negative_monitor_coordinates() {
        let placement = Placement {
            anchor: (-800., -1200.),
            bounds_origin: (-1900., -1400.),
            bounds_size: (1880., 960.),
        };
        let cli = <crate::Cli as clap::Parser>::try_parse_from(
            std::iter::once(OsString::from("slint-editor")).chain(arguments(Some(placement))),
        )
        .unwrap();
        assert!(cli.run_preview_child);
        assert_eq!(cli.preview_window_placement, Some(placement));
    }

    #[test]
    fn placement_stays_on_the_editors_monitor() {
        let placement = Placement {
            anchor: (-800., -1200.),
            bounds_origin: (-1900., -1000.),
            bounds_size: (1880., 960.),
        };
        for (anchor, expected) in [
            ((-200., -900.), (-800., -900.)),
            ((300., -900.), (-620., -900.)),
            ((-1600., -1200.), (-1900., -1000.)),
            ((-200., -100.), (-800., -440.)),
        ] {
            assert_eq!(Placement { anchor, ..placement }.position((600., 400.)), expected);
        }
        assert_eq!(placement.position((3000., 2000.)), (-1900., -1000.));
    }

    #[test]
    fn appkit_placement_uses_points_across_different_backing_scales() {
        for (editor_scale, monitor_scale, child_scale) in [(1., 1., 2.), (2., 2., 1.), (2., 1., 2.)]
        {
            for monitor_x in [1512., -1920.] {
                let placement = Placement::from_physical(
                    LogicalPosition::new(monitor_x + 100., 100.).to_physical(editor_scale),
                    LogicalSize::new(1200., 800.).to_physical(editor_scale),
                    editor_scale,
                    LogicalPosition::new(monitor_x, 0.).to_physical(monitor_scale),
                    LogicalSize::new(1920., 1080.).to_physical(monitor_scale),
                    monitor_scale,
                    true,
                );
                let child_size = LogicalSize::new(480., 320.)
                    .to_physical::<u32>(child_scale)
                    .to_logical::<f64>(child_scale);
                assert_eq!(
                    placement.position((child_size.width, child_size.height)),
                    (monitor_x + 796., 148.)
                );
                assert_eq!(placement.bounds_origin, (monitor_x + 24., 24.));
            }
        }
    }

    #[test]
    fn physical_desktop_coordinates_do_not_change_with_monitor_scale() {
        let placement = Placement::from_physical(
            winit::dpi::PhysicalPosition::new(3000, 200),
            winit::dpi::PhysicalSize::new(1600, 1000),
            2.,
            winit::dpi::PhysicalPosition::new(2560, 0),
            winit::dpi::PhysicalSize::new(2560, 1440),
            1.,
            false,
        );
        assert_eq!(placement.bounds_origin, (2608., 48.));
        assert_eq!(placement.position((960., 640.)), (3592., 296.));
    }
}
