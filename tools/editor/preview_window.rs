// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::OsString;

use slint::winit_030::{WinitWindowAccessor, winit};
use slint::{PhysicalPosition, PhysicalSize, Window};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Placement {
    anchor: (i32, i32),
    monitor_origin: (i32, i32),
    monitor_size: (u32, u32),
}

pub fn placement(editor: &Window) -> Option<Placement> {
    editor.with_winit_window(|window| {
        let position = window.outer_position().ok()?;
        let size = window.outer_size();
        let monitor = window.current_monitor()?;
        let origin = monitor.position();
        let bounds = monitor.size();
        let inset = (24. * window.scale_factor()).round() as i32;
        Some(Placement {
            anchor: (position.x + size.width as i32 - inset, position.y + 2 * inset),
            monitor_origin: (origin.x + inset, origin.y + inset),
            monitor_size: (
                bounds.width.saturating_sub(2 * inset as u32),
                bounds.height.saturating_sub(2 * inset as u32),
            ),
        })
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
    if !crate::headless_requested() {
        let selector = slint::BackendSelector::new()
            .with_winit_window_attributes_hook(move |attributes| match placement {
                Some(placement) => attributes.with_position(winit::dpi::PhysicalPosition::new(
                    placement.monitor_origin.0,
                    placement.monitor_origin.1,
                )),
                None => attributes,
            })
            .with_winit_custom_application_handler(PreviewWindow { placement, presented: false });
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
    i_slint_editor_preview::child_process::run()
}

struct PreviewWindow {
    placement: Option<Placement>,
    presented: bool,
}

impl slint::winit_030::CustomApplicationHandler for PreviewWindow {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        window: Option<&winit::window::Window>,
        _slint_window: Option<&Window>,
        event: &winit::event::WindowEvent,
    ) -> slint::winit_030::EventResult {
        if !self.presented
            && matches!(event, winit::event::WindowEvent::RedrawRequested)
            && let Some(window) = window
        {
            self.presented = true;
            if let Some(placement) = self.placement {
                let size = window.outer_size();
                let position = placement.position(PhysicalSize::new(size.width, size.height));
                window
                    .set_outer_position(winit::dpi::PhysicalPosition::new(position.x, position.y));
            }
            window.focus_window();
        }
        slint::winit_030::EventResult::Propagate
    }
}

impl Placement {
    fn position(self, size: PhysicalSize) -> PhysicalPosition {
        PhysicalPosition::new(
            (self.anchor.0 - size.width as i32).clamp(
                self.monitor_origin.0,
                self.monitor_origin.0 + self.monitor_size.0.saturating_sub(size.width) as i32,
            ),
            self.anchor.1.clamp(
                self.monitor_origin.1,
                self.monitor_origin.1 + self.monitor_size.1.saturating_sub(size.height) as i32,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_arguments_round_trip_negative_monitor_coordinates() {
        let placement = Placement {
            anchor: (-800, -1200),
            monitor_origin: (-1900, -1400),
            monitor_size: (1880, 960),
        };
        let arguments = arguments(Some(placement));
        let cli = <crate::Cli as clap::Parser>::try_parse_from(
            std::iter::once(OsString::from("slint-editor")).chain(arguments),
        )
        .unwrap();
        assert!(cli.run_preview_child);
        assert_eq!(cli.preview_window_placement, Some(placement));
    }

    #[test]
    fn placement_stays_on_the_editors_monitor() {
        let placement = Placement {
            anchor: (-800, -1200),
            monitor_origin: (-1900, -1000),
            monitor_size: (1880, 960),
        };
        let size = PhysicalSize::new(600, 400);
        for (anchor, expected) in [
            ((-200, -900), (-800, -900)),
            ((300, -900), (-620, -900)),
            ((-1600, -1200), (-1900, -1000)),
            ((-200, -100), (-800, -440)),
        ] {
            assert_eq!(
                Placement { anchor, ..placement }.position(size),
                PhysicalPosition::new(expected.0, expected.1),
            );
        }
        assert_eq!(
            placement.position(PhysicalSize::new(3000, 2000)),
            PhysicalPosition::new(-1900, -1000),
        );
    }
}
