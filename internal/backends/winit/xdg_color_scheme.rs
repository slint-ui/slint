// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use i_slint_core::graphics::Color;
use i_slint_core::items::ColorScheme;

use crate::WinitWindowAdapter;

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

pub async fn watch(window_weak: Weak<WinitWindowAdapter>) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let settings_proxy: zbus::Proxy = zbus::proxy::Builder::new(&connection)
        .interface("org.freedesktop.portal.Settings")?
        .path("/org/freedesktop/portal/desktop")?
        .destination("org.freedesktop.portal.Desktop")?
        .build()
        .await?;

    let initial_value: zbus::zvariant::OwnedValue =
        settings_proxy.call("ReadOne", &("org.freedesktop.appearance", "color-scheme")).await?;

    let Some(window) = window_weak.upgrade() else { return Ok(()) };
    window.set_color_scheme(xdg_color_scheme_to_slint(initial_value));

    let accent_result: zbus::Result<zbus::zvariant::OwnedValue> =
        settings_proxy.call("ReadOne", &("org.freedesktop.appearance", "accent-color")).await;
    if let Some(color) = accent_result.ok().and_then(xdg_accent_color_to_slint) {
        window.set_accent_color(color);
    }
    drop(window);

    use futures::stream::StreamExt;

    let mut settings_stream = settings_proxy
        .receive_signal_with_args("SettingChanged", &[(0, "org.freedesktop.appearance")])
        .await?
        .map(|message| {
            let (_, key, value): (String, String, zbus::zvariant::OwnedValue) =
                message.body().deserialize().ok()?;
            Some((key, value))
        });

    while let Some(Some((key, value))) = settings_stream.next().await {
        let Some(window) = window_weak.upgrade() else { return Ok(()) };
        match key.as_str() {
            "color-scheme" => {
                window.set_color_scheme(xdg_color_scheme_to_slint(value));
            }
            "accent-color" => {
                if let Some(color) = xdg_accent_color_to_slint(value) {
                    window.set_accent_color(color);
                }
            }
            _ => {}
        }
    }

    Ok(())
}
