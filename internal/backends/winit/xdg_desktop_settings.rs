// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use i_slint_core::SlintContextWeak;
use i_slint_core::graphics::Color;
use i_slint_core::items::ColorScheme;
use i_slint_core::lengths::LogicalLength;

use crate::SharedBackendData;

/// How long the first windows wait for the appearance query before mapping with
/// defaults. Keep it above portal activation time; a missing portal fails fast.
const APPEARANCE_QUERY_TIMEOUT: Duration = Duration::from_millis(500);

/// Desktop settings read from the XDG portal: cursor blink and the one-time
/// appearance query the first windows wait for. Updated by [`watch`].
pub(crate) struct DesktopSettings {
    /// Whether the cursor blinks at all.
    cursor_blink_enabled: Cell<bool>,
    /// The cursor blink period, used when blinking is enabled.
    cursor_blink_time: Cell<Duration>,
    /// True while the appearance query is in flight; the backend holds the first
    /// windows in `inactive_windows` until it clears, to avoid a default flash.
    appearance_pending: Cell<bool>,
}

impl DesktopSettings {
    pub(crate) fn new() -> Self {
        Self {
            cursor_blink_enabled: Cell::new(true),
            cursor_blink_time: Cell::new(crate::DEFAULT_CURSOR_FLASH_CYCLE),
            appearance_pending: Cell::new(false),
        }
    }

    /// The cursor blink period, or zero when blinking is disabled.
    pub(crate) fn cursor_flash_cycle(&self) -> Duration {
        if self.cursor_blink_enabled.get() { self.cursor_blink_time.get() } else { Duration::ZERO }
    }

    /// Whether the first windows must still wait for the appearance query.
    pub(crate) fn is_appearance_pending(&self) -> bool {
        self.appearance_pending.get()
    }
}

const APPEARANCE: &str = "org.freedesktop.appearance";
const GNOME_INTERFACE: &str = "org.gnome.desktop.interface";

/// Handles passed to a setting's `apply` function so it can push the parsed value
/// into the runtime context or the backend.
struct SettingsContext<'a> {
    ctx: &'a SlintContextWeak,
    shared: &'a Weak<SharedBackendData>,
}

/// One desktop setting read from the portal. The initial read and the
/// `SettingChanged` handler both call `apply`, so a new setting is one row.
struct SettingDescriptor {
    namespace: &'static str,
    key: &'static str,
    apply: fn(zbus::zvariant::OwnedValue, &SettingsContext),
}

/// Every desktop setting Slint reacts to. Add a row here (and a small `apply_*`
/// function) to support a new one.
static SETTINGS: &[SettingDescriptor] = &[
    SettingDescriptor {
        namespace: APPEARANCE,
        key: "color-scheme",
        apply: apply_color_scheme_value,
    },
    SettingDescriptor { namespace: APPEARANCE, key: "accent-color", apply: apply_accent_value },
    SettingDescriptor { namespace: GNOME_INTERFACE, key: "font-name", apply: apply_font_value },
    SettingDescriptor {
        namespace: GNOME_INTERFACE,
        key: "cursor-blink",
        apply: apply_cursor_blink_value,
    },
    SettingDescriptor {
        namespace: GNOME_INTERFACE,
        key: "cursor-blink-time",
        apply: apply_cursor_blink_time_value,
    },
];

/// Parses the trailing point size from a GNOME font description such as
/// `"Helvetica 11"` or `"Sans Bold 10.5"`.
fn parse_font_points(font_name: &str) -> Option<f32> {
    let last = font_name.split_whitespace().next_back()?;
    last.parse::<f32>().ok().filter(|p| p.is_finite() && *p > 0.0)
}

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

/// Sets the color scheme on the context and pushes the matching winit theme to
/// every mapped window so client-side decorations stay in sync.
fn apply_color_scheme_value(value: zbus::zvariant::OwnedValue, cx: &SettingsContext) {
    let scheme = xdg_color_scheme_to_slint(value);
    if let Some(ctx) = cx.ctx.upgrade() {
        ctx.set_color_scheme(scheme);
    }
    let Some(shared) = cx.shared.upgrade() else { return };
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

fn apply_accent_value(value: zbus::zvariant::OwnedValue, cx: &SettingsContext) {
    if let Some(color) = xdg_accent_color_to_slint(value)
        && let Some(ctx) = cx.ctx.upgrade()
    {
        ctx.set_accent_color(color);
    }
}

fn apply_font_value(value: zbus::zvariant::OwnedValue, cx: &SettingsContext) {
    if let Ok(name) = value.downcast_ref::<&str>()
        && let Some(points) = parse_font_points(name)
        && let Some(ctx) = cx.ctx.upgrade()
    {
        ctx.set_platform_default_font_size(Some(LogicalLength::new(points * 96.0 / 72.0)));
    }
}

fn apply_cursor_blink_value(value: zbus::zvariant::OwnedValue, cx: &SettingsContext) {
    if let Ok(enabled) = value.downcast_ref::<bool>()
        && let Some(shared) = cx.shared.upgrade()
    {
        shared.desktop_settings.cursor_blink_enabled.set(enabled);
    }
}

fn apply_cursor_blink_time_value(value: zbus::zvariant::OwnedValue, cx: &SettingsContext) {
    if let Ok(ms) = value.downcast_ref::<i32>()
        && ms > 0
        && let Some(shared) = cx.shared.upgrade()
    {
        shared.desktop_settings.cursor_blink_time.set(Duration::from_millis(ms as u64));
    }
}

/// Reads every entry in [`SETTINGS`] concurrently and applies each result, so the
/// whole batch costs roughly one round-trip instead of one per setting.
async fn read_all_settings(settings_proxy: &zbus::Proxy<'_>, cx: &SettingsContext<'_>) {
    futures::future::join_all(SETTINGS.iter().map(|setting| async move {
        if let Ok(value) = read_setting(settings_proxy, setting).await {
            (setting.apply)(value, cx);
        }
    }))
    .await;
}

/// Reads one setting.
/// Portals older than 1.15 (Ubuntu 22.04 and earlier) only implement the deprecated `Read`,
/// whose extra variant wrapping `downcast_ref()` unwraps transparently.
async fn read_setting(
    settings_proxy: &zbus::Proxy<'_>,
    setting: &SettingDescriptor,
) -> zbus::Result<zbus::zvariant::OwnedValue> {
    let args = (setting.namespace, setting.key);
    #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
    match settings_proxy.call("ReadOne", &args).await {
        Err(zbus::Error::MethodError(name, ..))
            if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" =>
        {
            settings_proxy.call("Read", &args).await
        }
        result => result,
    }
}

/// Clears the pending flag so the backend creates and shows the first windows.
fn finish_appearance_query(shared: &Weak<SharedBackendData>) {
    if let Some(shared) = shared.upgrade() {
        shared.desktop_settings.appearance_pending.set(false);
    }
}

async fn watch(
    shared_data_weak: &Weak<SharedBackendData>,
    ctx_weak: SlintContextWeak,
) -> zbus::Result<()> {
    // Safety net: create the windows anyway if the portal never answers.
    // After the timer fires, the event loop returns to `about_to_wait`,
    // which then creates any pending inactive windows.
    {
        let shared_weak = shared_data_weak.clone();
        i_slint_core::timers::Timer::single_shot(APPEARANCE_QUERY_TIMEOUT, move || {
            finish_appearance_query(&shared_weak);
        });
    }

    let connection = zbus::Connection::session().await?;
    let settings_proxy: zbus::Proxy = zbus::proxy::Builder::new(&connection)
        .interface("org.freedesktop.portal.Settings")?
        .path("/org/freedesktop/portal/desktop")?
        .destination("org.freedesktop.portal.Desktop")?
        .build()
        .await?;

    let cx = SettingsContext { ctx: &ctx_weak, shared: shared_data_weak };

    // Subscribe before the initial read so no change is missed in the gap between them.
    use futures::stream::StreamExt;
    let mut settings_stream = settings_proxy.receive_signal("SettingChanged").await?;

    read_all_settings(&settings_proxy, &cx).await;

    // The appearance is known now, so the first windows can be created and shown.
    finish_appearance_query(shared_data_weak);

    while let Some(message) = settings_stream.next().await {
        let Ok((namespace, key, value)) =
            message.body().deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
        else {
            continue;
        };
        if let Some(setting) = SETTINGS.iter().find(|s| s.namespace == namespace && s.key == key) {
            (setting.apply)(value, &cx);
        }
    }

    Ok(())
}

/// True when the error only means the session bus or the settings portal is missing,
/// which is normal on headless systems and bare compositors.
fn portal_unavailable(err: &zbus::Error) -> bool {
    #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
    match err {
        // No session bus address, or nothing listening on it.
        zbus::Error::Address(_) | zbus::Error::InputOutput(_) => true,
        zbus::Error::MethodError(name, ..) => matches!(
            name.as_str(),
            "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
        ),
        zbus::Error::FDO(err) => matches!(
            **err,
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
        ),
        _ => false,
    }
}

/// Starts the portal watcher and the timeout that releases the first windows if
/// the portal is slow or missing. Returns the task so the backend can abort it.
pub(crate) fn spawn(
    shared_data: &Rc<SharedBackendData>,
    ctx: &SlintContextWeak,
) -> Option<i_slint_core::future::JoinHandle<()>> {
    let strong_ctx = ctx.upgrade().expect("spawn is called while the SlintContext is still alive");
    // Hold back the first windows until the query applies the real appearance.
    shared_data.desktop_settings.appearance_pending.set(true);

    let shared_weak = Rc::downgrade(shared_data);
    let ctx_weak = ctx.clone();
    strong_ctx
        .spawn_local(async move {
            if let Err(err) = watch(&shared_weak, ctx_weak).await {
                if !portal_unavailable(&err) {
                    i_slint_core::debug_log!("Error watching for xdg desktop settings: {err}");
                }
                // The portal is unavailable; create the waiting windows anyway.
                finish_appearance_query(&shared_weak);
            }
        })
        .ok()
}
