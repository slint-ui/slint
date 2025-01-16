// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use i_slint_core::items::ColorScheme;

use crate::WinitWindowAdapter;

fn xdg_color_scheme_to_slint(value: zbus::zvariant::OwnedValue) -> ColorScheme {
    match value.downcast_ref::<u32>() {
        Ok(1) => ColorScheme::Dark,
        Ok(2) => ColorScheme::Light,
        _ => ColorScheme::Unknown,
    }
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

    if let Some(window) = window_weak.upgrade() {
        window.set_color_scheme(xdg_color_scheme_to_slint(initial_value));
    }

    use futures::stream::StreamExt;

    let mut color_scheme_stream = settings_proxy
        .receive_signal_with_args(
            "SettingChanged",
            &[(0, "org.freedesktop.appearance"), (1, "color-scheme")],
        )
        .await?
        .map(|message| {
            let (_, _, scheme): (String, String, zbus::zvariant::OwnedValue) =
                message.body().deserialize().ok()?;
            Some(scheme)
        });

    while let Some(Some(new_scheme)) = color_scheme_stream.next().await {
        if let Some(window) = window_weak.upgrade() {
            window.set_color_scheme(xdg_color_scheme_to_slint(new_scheme));
        }
    }

    Ok(())
}
