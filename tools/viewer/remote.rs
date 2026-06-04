// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{net::SocketAddr, rc::Rc};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::diagnostics::Spanned;
use i_slint_core::InternalToken;
use i_slint_core::SharedString;
use i_slint_live_preview::protocol::{PreviewComponent, PreviewToLspMessage, lsp_types};
use i_slint_live_preview::remote::{Connection, ConnectionMessage, init_compiler};
use slint_interpreter::ComponentHandle as _;

const MAIN_SLINT: &str = include_str!("remote/main.slint");

fn idle_ui_source_path() -> std::path::PathBuf {
    app_bundle_dir().join("main.slint")
}

fn app_bundle_dir() -> std::path::PathBuf {
    if let Ok(mut path) = std::env::current_exe() {
        loop {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".app"))
            {
                return path;
            }
            if !path.pop() {
                break;
            }
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(std::env::temp_dir)
}

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
    let source_path = idle_ui_source_path();
    let compilation_result = compiler.build_from_source(MAIN_SLINT.to_owned(), source_path).await;
    if compilation_result.has_errors() {
        let mut build_diagnostics = BuildDiagnostics::default();
        for d in compilation_result.diagnostics() {
            build_diagnostics.push_compiler_error(d);
        }
        let diagnostics = build_diagnostics.diagnostics_as_string();
        tracing::error!("Failed compiling main.slint: {diagnostics}");
        anyhow::bail!("Failed compiling main.slint: {diagnostics}");
    }
    let main_ui = compilation_result.component("EmptyWindow").unwrap();
    let window = main_ui.create().unwrap();

    let mut inner_window = window.clone_strong();
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

    if let Err(err) = window.set_property("address", SharedString::from(address.as_str()).into()) {
        tracing::error!("Failed setting property: {err}");
    }

    if let Err(err) = window.set_property("name", SharedString::from(device_name.clone()).into()) {
        tracing::error!("Failed setting property: {err}");
    }

    window.show().inspect_err(|err| tracing::error!("window show: {err}"))?;

    let mut last_connection = None;
    let mut instance = inner_window.clone_strong();
    let mut current_preview: Option<PreviewComponent> = None;
    while let Some(msg) = message_receiver.recv().await {
        match msg {
            ConnectionMessage::SetConfiguration { config } => {
                compiler.set_style(config.style);
                compiler.compiler_configuration(InternalToken).enable_experimental =
                    config.enable_experimental;
            }
            ConnectionMessage::ShowPreview { preview_component } => {
                if let Some(new_instance) = build_and_show(
                    &compiler,
                    &preview_component,
                    &main_ui,
                    &mut inner_window,
                    &instance,
                    &connection,
                    &address,
                    &device_name,
                )
                .await?
                {
                    instance = new_instance;
                }
                current_preview = Some(preview_component);
            }
            ConnectionMessage::ContentsChanged => {
                let Some(preview_component) = current_preview.clone() else { continue };
                if let Some(new_instance) = build_and_show(
                    &compiler,
                    &preview_component,
                    &main_ui,
                    &mut inner_window,
                    &instance,
                    &connection,
                    &address,
                    &device_name,
                )
                .await?
                {
                    instance = new_instance;
                }
            }
            ConnectionMessage::HighlightFromEditor { .. } => {}
            ConnectionMessage::Connected { remote_addr } => {
                if let Err(err) = inner_window.set_property(
                    "message",
                    SharedString::from(format!("Connected to {remote_addr}")).into(),
                ) {
                    tracing::error!("Failed setting property: {err}");
                }
                last_connection = Some(remote_addr);
            }
            ConnectionMessage::Disconnected { remote_addr } => {
                if last_connection == Some(remote_addr) {
                    last_connection = None;
                    current_preview = None;
                    connection.set_dependencies(Vec::new());
                    inner_window =
                        show_placeholder(&main_ui, &instance, &address, &device_name, "");
                    instance = inner_window.clone_strong();
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

/// Compile `preview_component` from the connection's file cache and replace the visible
/// instance with it. Returns `Ok(Some(new))` after a successful build or after swapping
/// back to the placeholder because the build failed, `Ok(None)` for requests we can't
/// act on at all (bad URL, file fetch failure), and `Err` only when the platform can no
/// longer host a Slint window — the caller should propagate it and exit, since retrying
/// on every keystroke would only repeat the underlying failure.
///
/// When the build fails after a project was previously shown, the placeholder window is
/// brought back so the user actually sees the diagnostics instead of the now-stale UI.
async fn build_and_show(
    compiler: &slint_interpreter::Compiler,
    preview_component: &PreviewComponent,
    main_ui: &slint_interpreter::ComponentDefinition,
    inner_window: &mut slint_interpreter::ComponentInstance,
    instance: &slint_interpreter::ComponentInstance,
    connection: &Rc<Connection>,
    address: &str,
    name: &str,
) -> anyhow::Result<Option<slint_interpreter::ComponentInstance>> {
    tracing::debug!("build_and_show");

    let Ok(path) = preview_component.url.to_file_path() else {
        tracing::error!("Not a file URL: {}", preview_component.url);
        return Ok(None);
    };
    let file = match connection.request_file(preview_component.url.clone()).await {
        Ok(file) => file,
        Err(err) => {
            tracing::error!("Failed fetching {}: {err}", preview_component.url);
            return Ok(None);
        }
    };
    let compilation_result = compiler
        .build_from_source(String::from_utf8_lossy(&file.contents).into_owned(), path)
        .await;
    // Watch paths come from the type loader and are populated even on errors, so the
    // connection keeps reacting to edits in the right files while the user types a fix.
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
        let placeholder = show_placeholder(main_ui, instance, address, name, &message);
        *inner_window = placeholder.clone_strong();
        return Ok(Some(placeholder));
    }

    let Some(component) = preview_component
        .component
        .as_deref()
        .or_else(|| compilation_result.component_names().next())
        .and_then(|name| compilation_result.component(name))
    else {
        // Compilation produced no errors but the requested component is missing.
        // Don't publish a Diagnostics message: that would clobber whatever the LSP
        // (or a previous compile) was showing in the editor for this URI, while
        // the only signal we have to surface is local to the viewer window.
        tracing::error!("Component not found");
        let placeholder = show_placeholder(main_ui, instance, address, name, "Component not found");
        *inner_window = placeholder.clone_strong();
        return Ok(Some(placeholder));
    };

    // Clean build: publish the (possibly empty) diagnostic list so the editor
    // clears any errors we surfaced from the previous build.
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
        .create_with_existing_window(instance.window())
        .map_err(|err| anyhow::anyhow!("Cannot create component instance: {err}"))?;

    new_instance.show().map_err(|err| anyhow::anyhow!("Cannot show component: {err}"))?;

    Ok(Some(new_instance))
}

/// Create a fresh instance of the placeholder window on top of `instance`'s existing
/// window, populate it with the static identification fields plus an optional `message`,
/// and show it. Used both when no editor is connected and when a build fails — in either
/// case it's the only UI we can give the user since the previously visible component
/// instance can no longer be trusted to reflect the current file.
fn show_placeholder(
    main_ui: &slint_interpreter::ComponentDefinition,
    instance: &slint_interpreter::ComponentInstance,
    address: &str,
    name: &str,
    message: &str,
) -> slint_interpreter::ComponentInstance {
    let placeholder = main_ui
        .create_with_existing_window(instance.window())
        .unwrap_or_else(|_| main_ui.create().unwrap());
    if let Err(err) = placeholder.set_property("address", SharedString::from(address).into()) {
        tracing::error!("Failed setting property: {err}");
    }
    if let Err(err) = placeholder.set_property("name", SharedString::from(name).into()) {
        tracing::error!("Failed setting property: {err}");
    }
    if let Err(err) = placeholder.set_property("message", SharedString::from(message).into()) {
        tracing::error!("Failed setting property: {err}");
    }
    placeholder.show().unwrap();
    placeholder
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
