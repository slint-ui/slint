// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{net::SocketAddr, rc::Rc};

use i_slint_compiler::diagnostics::Spanned;
use i_slint_core::InternalToken;
use i_slint_core::SharedString;
use i_slint_live_preview::protocol::{PreviewComponent, PreviewToLspMessage, lsp_types};
use i_slint_live_preview::remote::{Connection, ConnectionMessage, init_compiler};
use slint::ComponentHandle as _;

slint::slint! {
    export { EmptyWindow } from "remote/main.slint";
}

// CARGO_PKG_VERSION tracks the workspace version, so it is the Slint version.
const SLINT_VERSION: &str = concat!("Slint ", env!("CARGO_PKG_VERSION"));
const BUILD_COMMIT: &str = git_version::git_version!();

pub fn run(address: Option<SocketAddr>, enable_mdns: bool) -> anyhow::Result<()> {
    slint_interpreter::spawn_local(async_compat::Compat::new(async move {
        if let Err(err) = run_async(address, enable_mdns).await {
            tracing::error!("Remote viewer error: {err}");
            slint_interpreter::quit_event_loop().ok();
        }
    }))?;
    slint_interpreter::run_event_loop()?;
    Ok(())
}

async fn run_async(address: Option<SocketAddr>, enable_mdns: bool) -> anyhow::Result<()> {
    let (message_sender, mut message_receiver) = tokio::sync::mpsc::unbounded_channel();

    let connection = Rc::new(
        Connection::listen(address, device_name_override(), move |msg| {
            let _ = message_sender.send(msg);
        })
        .await?,
    );

    let mut compiler = init_compiler(Rc::downgrade(&connection));

    let mut placeholder = EmptyWindow::new()?;

    #[cfg(not(target_vendor = "apple"))]
    let mdns = enable_mdns.then(mdns_sd::ServiceDaemon::new).transpose()?;

    #[cfg(not(target_vendor = "apple"))]
    {
        let service = connection.service()?;
        mdns.as_ref().map(|mdns| mdns.register(service)).transpose()?;
    }
    #[cfg(target_vendor = "apple")]
    let mut mdns = enable_mdns
        .then(|| {
            use zeroconf_tokio::prelude::{TMdnsService as _, TTxtRecord as _};

            let mut service = zeroconf_tokio::MdnsService::new(
                zeroconf_tokio::ServiceType::new(
                    i_slint_live_preview::protocol::SERVICE_TYPE_NAME,
                    i_slint_live_preview::protocol::SERVICE_TYPE_PROTOCOL,
                )?,
                connection.local_port(),
            );
            // Deliberately don't set a name: with a NULL/empty instance name Bonjour
            // substitutes the system default service name, which is the user-assigned
            // device name (e.g. "Simon's iPhone" on iOS, the computer name on macOS).
            // This is the friendly name we want to show in the editor.
            let mut txt = zeroconf_tokio::TxtRecord::new();
            txt.insert(
                i_slint_live_preview::protocol::TXT_PROTOCOLS_KEY,
                i_slint_live_preview::protocol::PROTOCOL_SUBPROTOCOL,
            )?;
            txt.insert(
                i_slint_live_preview::protocol::TXT_SLINT_VERSION_KEY,
                i_slint_live_preview::protocol::SLINT_VERSION,
            )?;
            service.set_txt_record(txt);
            zeroconf_tokio::MdnsServiceAsync::new(service)
        })
        .transpose()
        .inspect_err(|err| tracing::error!("Failed to initialize mDNS: {err}"))
        .ok()
        .flatten();

    #[cfg(target_vendor = "apple")]
    if let Some(mdns) = &mut mdns {
        match mdns.start().await {
            Ok(registration) => connection.set_device_name(registration.name().to_owned()),
            Err(err) => tracing::error!("Failed to announce service: {err}"),
        }
    }
    // Snapshot after the Apple Bonjour overwrite above so the UI label matches the
    // advertised mDNS instance. Re-read `connection.device_name()` here (not at the
    // set_property sites) if a future change starts mutating the name post-registration.
    let device_name = connection.device_name();

    let local_port = connection.local_port();
    let local_ip_str: Vec<String> = connection
        .local_ips()
        .into_iter()
        .map(|ip| match ip {
            std::net::IpAddr::V4(ipv4_addr) => format!("{ipv4_addr}:{local_port}"),
            std::net::IpAddr::V6(ipv6_addr) => {
                format!("[{ipv6_addr}]:{local_port}")
            }
        })
        .collect();
    let address = local_ip_str.join("\n");

    placeholder.set_address(SharedString::from(address.as_str()));
    placeholder.set_name(SharedString::from(device_name.as_str()));
    placeholder.set_slint_version(SharedString::from(SLINT_VERSION));
    if !BUILD_COMMIT.is_empty() {
        placeholder.set_build_info(slint::format!("Build from {BUILD_COMMIT}"));
    }
    placeholder.show()?;

    let mut last_connection = None;
    let mut user_instance: Option<slint_interpreter::ComponentInstance> = None;
    let mut current_preview: Option<PreviewComponent> = None;
    while let Some(msg) = message_receiver.recv().await {
        match msg {
            ConnectionMessage::SetConfiguration { config } => {
                compiler.set_style(config.style);
                compiler.compiler_configuration(InternalToken).enable_experimental =
                    config.enable_experimental;
            }
            ConnectionMessage::ShowPreview { preview_component } => {
                build_and_show(
                    &compiler,
                    &preview_component,
                    &mut placeholder,
                    &mut user_instance,
                    &connection,
                    &address,
                    &device_name,
                )
                .await?;
                current_preview = Some(preview_component);
            }
            ConnectionMessage::ContentsChanged => {
                let Some(preview_component) = current_preview.clone() else { continue };
                build_and_show(
                    &compiler,
                    &preview_component,
                    &mut placeholder,
                    &mut user_instance,
                    &connection,
                    &address,
                    &device_name,
                )
                .await?;
            }
            ConnectionMessage::HighlightFromEditor { .. } => {}
            ConnectionMessage::Connected { remote_addr } => {
                placeholder.set_message(SharedString::from(format!("Connected to {remote_addr}")));
                last_connection = Some(remote_addr);
            }
            ConnectionMessage::Disconnected { remote_addr } => {
                if last_connection == Some(remote_addr) {
                    last_connection = None;
                    current_preview = None;
                    connection.set_dependencies(Vec::new());
                    swap_to_placeholder(
                        &mut placeholder,
                        &mut user_instance,
                        &address,
                        &device_name,
                        "",
                    )?;
                }
            }
        }
    }

    #[cfg(not(target_vendor = "apple"))]
    mdns.map(|mdns| mdns.shutdown())
        .transpose()
        .inspect_err(|err| tracing::error!("mdns shutdown: {err}"))?;

    Ok(())
}

/// Returns `Err` only on unrecoverable platform failure; compile errors and missing
/// components reinstall the placeholder and return `Ok(())`.
async fn build_and_show(
    compiler: &slint_interpreter::Compiler,
    preview_component: &PreviewComponent,
    placeholder: &mut EmptyWindow,
    user_instance: &mut Option<slint_interpreter::ComponentInstance>,
    connection: &Rc<Connection>,
    address: &str,
    name: &str,
) -> anyhow::Result<()> {
    tracing::debug!("build_and_show");

    let Ok(path) = preview_component.url.to_file_path() else {
        tracing::error!("Not a file URL: {}", preview_component.url);
        return Ok(());
    };
    let file = match connection.request_file(preview_component.url.clone()).await {
        Ok(file) => file,
        Err(err) => {
            tracing::error!("Failed fetching {}: {err}", preview_component.url);
            return Ok(());
        }
    };
    let compilation_result = compiler
        .build_from_source(String::from_utf8_lossy(&file.contents).into_owned(), path)
        .await;
    // Set even on errors so edits to imported files still trigger a rebuild.
    let watch_urls: Vec<lsp_types::Url> = compilation_result
        .watch_paths(InternalToken)
        .iter()
        .filter_map(|p| lsp_types::Url::from_file_path(p).ok())
        .collect();
    connection.set_dependencies(watch_urls);

    if compilation_result.has_errors() {
        send_diagnostics(&compilation_result, &preview_component.url, connection);
        let message = compilation_result
            .diagnostics()
            .inspect(|d| tracing::warn!("Compiler diagnostic: {d}"))
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        swap_to_placeholder(placeholder, user_instance, address, name, &message)?;
        return Ok(());
    }

    let Some(component) = preview_component
        .component
        .as_deref()
        .or_else(|| compilation_result.component_names().next())
        .and_then(|name| compilation_result.component(name))
    else {
        // No compile errors but no component — skip send_diagnostics so we don't clobber
        // unrelated LSP diagnostics for this URI.
        tracing::error!("Component not found");
        swap_to_placeholder(placeholder, user_instance, address, name, "Component not found")?;
        return Ok(());
    };

    // Send the (possibly empty) list so the editor clears stale errors.
    send_diagnostics(&compilation_result, &preview_component.url, connection);

    let connection = Rc::downgrade(connection);
    component.set_debug_handler(
        move |location, message| {
            let Some(connection) = connection.upgrade() else {
                return;
            };
            let location = location.and_then(|location| {
                location.source_file().map(|file| {
                    let (line, column) = file.line_column(
                        location.span.offset,
                        i_slint_compiler::diagnostics::ByteFormat::Utf8,
                    );
                    (file.path().to_owned(), line, column)
                })
            });
            connection
                .send(PreviewToLspMessage::DebugMessage { location, message: message.into() })
                .ok();
        },
        i_slint_core::InternalToken,
    );
    let new_instance = component
        .create_with_existing_window(placeholder.window())
        .map_err(|err| anyhow::anyhow!("Cannot create component instance: {err}"))?;

    new_instance.show().map_err(|err| anyhow::anyhow!("Cannot show component: {err}"))?;
    *user_instance = Some(new_instance);
    Ok(())
}

/// Reinstall a fresh placeholder onto the existing window and drop the user instance.
fn swap_to_placeholder(
    placeholder: &mut EmptyWindow,
    user_instance: &mut Option<slint_interpreter::ComponentInstance>,
    address: &str,
    name: &str,
    message: &str,
) -> anyhow::Result<()> {
    let fresh = EmptyWindow::new_with_existing_window(placeholder.window())
        .map_err(|err| anyhow::anyhow!("Cannot create placeholder: {err}"))?;
    fresh.set_address(SharedString::from(address));
    fresh.set_name(SharedString::from(name));
    fresh.set_message(SharedString::from(message));
    fresh.set_slint_version(SharedString::from(SLINT_VERSION));
    if !BUILD_COMMIT.is_empty() {
        fresh.set_build_info(slint::format!("Build from {BUILD_COMMIT}"));
    }
    fresh.show().map_err(|err| anyhow::anyhow!("Cannot show placeholder: {err}"))?;
    *placeholder = fresh;
    *user_instance = None;
    Ok(())
}

/// Platform-specific override for the friendly device name. Returns `None` on platforms
/// where the default chain in `Connection` (pretty hostname → hostname, then Bonjour on
/// Apple) is already best.
fn device_name_override() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        ANDROID_DEVICE_NAME.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Set by `android_main` before `run` is called so the connection picks up the
/// user-set device name from `Settings.Global.DEVICE_NAME`.
#[cfg(target_os = "android")]
pub(crate) static ANDROID_DEVICE_NAME: std::sync::Mutex<Option<String>> =
    std::sync::Mutex::new(None);

fn send_diagnostics(
    compilation_result: &slint_interpreter::CompilationResult,
    uri: &lsp_types::Url,
    connection: &Connection,
) {
    let message = PreviewToLspMessage::Diagnostics {
        uri: uri.clone(),
        version: None,
        diagnostics: compilation_result
            .diagnostics()
            .map(|diagnostic| {
                i_slint_live_preview::protocol::to_lsp_diagnostic(
                    &diagnostic,
                    i_slint_compiler::diagnostics::ByteFormat::Utf8,
                )
            })
            .collect(),
    };
    connection.send(message).ok();
}
