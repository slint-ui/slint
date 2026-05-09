// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use i_slint_core::SlintContextWeak;
use i_slint_core::graphics::Color;
use i_slint_core::items::ColorScheme;

use crate::SharedBackendData;

fn xdg_color_scheme_to_slint(value: zbus::zvariant::OwnedValue) -> ColorScheme {
    match value.downcast_ref::<u32>() {
        Ok(1) => ColorScheme::Dark,
        Ok(2) => ColorScheme::Light,
        _ => ColorScheme::Unknown,
    }
}

fn xdg_accent_color_to_slint(value: zbus::zvariant::OwnedValue) -> Option<Color> {
    // The accent-color setting returns a (ddd) tuple of RGB doubles in [0.0, 1.0]
    let (r, g, b) = value.downcast_ref::<(f64, f64, f64)>().ok()?;
    Some(Color::from_argb_f32(1.0, r as f32, g as f32, b as f32))
}

/// Writes the new color scheme to the SlintContext and pushes the matching
/// winit theme to every currently-mapped window so that client-side decorations
/// stay in sync.
fn apply_color_scheme(
    ctx_weak: &SlintContextWeak,
    shared_data_weak: &Weak<SharedBackendData>,
    scheme: ColorScheme,
) {
    if let Some(ctx) = ctx_weak.upgrade() {
        ctx.set_color_scheme(scheme);
    }
    let Some(shared) = shared_data_weak.upgrade() else { return };
    let theme = match scheme {
        ColorScheme::Dark => Some(winit::window::Theme::Dark),
        ColorScheme::Light => Some(winit::window::Theme::Light),
        ColorScheme::Unknown => None,
        _ => None,
    };
    for adapter_weak in shared.active_windows.borrow().values() {
        if let Some(adapter) = adapter_weak.upgrade()
            && let Some(winit_window) = adapter.winit_window()
        {
            winit_window.set_theme(theme);
        }
    }
}

async fn read_cursor_blink_settings(
    settings_proxy: &zbus::Proxy<'_>,
) -> Option<core::time::Duration> {
    let blink_enabled: zbus::Result<zbus::zvariant::OwnedValue> =
        settings_proxy.call("ReadOne", &("org.gnome.desktop.interface", "cursor-blink")).await;
    if let Ok(value) = blink_enabled
        && value.downcast_ref::<bool>() == Ok(false)
    {
        return Some(core::time::Duration::ZERO);
    }

    let blink_time: zbus::Result<zbus::zvariant::OwnedValue> =
        settings_proxy.call("ReadOne", &("org.gnome.desktop.interface", "cursor-blink-time")).await;
    if let Ok(value) = blink_time
        && let Ok(ms) = value.downcast_ref::<i32>()
        && ms > 0
    {
        return Some(core::time::Duration::from_millis(ms as u64));
    }

    None
}

pub async fn watch(
    shared_data_weak: Weak<SharedBackendData>,
    ctx_weak: SlintContextWeak,
) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let settings_proxy: zbus::Proxy = zbus::proxy::Builder::new(&connection)
        .interface("org.freedesktop.portal.Settings")?
        .path("/org/freedesktop/portal/desktop")?
        .destination("org.freedesktop.portal.Desktop")?
        .build()
        .await?;

    let initial_value: zbus::zvariant::OwnedValue =
        settings_proxy.call("ReadOne", &("org.freedesktop.appearance", "color-scheme")).await?;
    apply_color_scheme(&ctx_weak, &shared_data_weak, xdg_color_scheme_to_slint(initial_value));

    let accent_result: zbus::Result<zbus::zvariant::OwnedValue> =
        settings_proxy.call("ReadOne", &("org.freedesktop.appearance", "accent-color")).await;
    if let Some(color) = accent_result.ok().and_then(xdg_accent_color_to_slint)
        && let Some(ctx) = ctx_weak.upgrade()
    {
        ctx.set_accent_color(color);
    }

    if let Some(interval) = read_cursor_blink_settings(&settings_proxy).await
        && let Some(shared) = shared_data_weak.upgrade()
    {
        shared.cursor_blink_interval.set(interval);
    }

    use futures::stream::StreamExt;

    let mut settings_stream =
        settings_proxy.receive_signal("SettingChanged").await?.map(|message| {
            let (namespace, key, value): (String, String, zbus::zvariant::OwnedValue) =
                message.body().deserialize().ok()?;
            Some((namespace, key, value))
        });

    while let Some(Some((namespace, key, value))) = settings_stream.next().await {
        match (namespace.as_str(), key.as_str()) {
            ("org.freedesktop.appearance", "color-scheme") => {
                apply_color_scheme(&ctx_weak, &shared_data_weak, xdg_color_scheme_to_slint(value));
            }
            ("org.freedesktop.appearance", "accent-color") => {
                if let Some(color) = xdg_accent_color_to_slint(value)
                    && let Some(ctx) = ctx_weak.upgrade()
                {
                    ctx.set_accent_color(color);
                }
            }
            ("org.gnome.desktop.interface", "cursor-blink") => {
                if let Ok(enabled) = value.downcast_ref::<bool>() {
                    let interval = if enabled {
                        read_cursor_blink_settings(&settings_proxy)
                            .await
                            .unwrap_or(crate::DEFAULT_CURSOR_FLASH_CYCLE)
                    } else {
                        core::time::Duration::ZERO
                    };
                    if let Some(shared) = shared_data_weak.upgrade() {
                        shared.cursor_blink_interval.set(interval);
                    }
                }
            }
            ("org.gnome.desktop.interface", "cursor-blink-time") => {
                if let Ok(ms) = value.downcast_ref::<i32>()
                    && ms > 0
                    && let Some(shared) = shared_data_weak.upgrade()
                {
                    shared.cursor_blink_interval.set(core::time::Duration::from_millis(ms as u64));
                }
            }
            _ => {}
        }
    }

    Ok(())
}
