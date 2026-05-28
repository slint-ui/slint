// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{net::SocketAddr, rc::Rc};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_core::InternalToken;
use i_slint_core::SharedString;
use i_slint_live_preview::protocol::PreviewToLspMessage;
use i_slint_live_preview::remote::{CacheEntry, Connection, ConnectionMessage, init_compiler};
use slint_interpreter::ComponentHandle as _;

const MAIN_SLINT: &str = include_str!("remote/main.slint");

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
        Connection::listen(address, move |msg| {
            let _ = message_sender.send(msg);
        })
        .await?,
    );

    let mut compiler = init_compiler(Rc::downgrade(&connection));
    let base_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_owned()))
        .unwrap_or_else(std::env::temp_dir);
    let compilation_result = compiler.build_from_source(MAIN_SLINT.to_owned(), base_path).await;
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
            use zeroconf_tokio::prelude::TMdnsService as _;

            let mut service = zeroconf_tokio::MdnsService::new(
                zeroconf_tokio::ServiceType::new(
                    i_slint_live_preview::protocol::SERVICE_TYPE_NAME,
                    i_slint_live_preview::protocol::SERVICE_TYPE_PROTOCOL,
                )?,
                connection.local_port(),
            );
            service.set_name("viewer");
            zeroconf_tokio::MdnsServiceAsync::new(service)
        })
        .transpose()
        .inspect_err(|err| tracing::error!("Failed to initialize mDNS: {err}"))
        .ok()
        .flatten();

    #[cfg(target_vendor = "apple")]
    if let Some(mdns) = &mut mdns
        && let Err(err) = mdns.start().await
    {
        tracing::error!("Failed to announce service: {err}");
    }

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

    if let Err(err) =
        window.set_property("address", SharedString::from(local_ip_str.join("\n")).into())
    {
        tracing::error!("Failed setting property: {err}");
    }

    println!("{}", local_ip_str.join("\n"));

    window.show().inspect_err(|err| tracing::error!("window show: {err}"))?;

    let mut last_connection = None;
    let mut instance = inner_window.clone_strong();
    while let Some(msg) = message_receiver.recv().await {
        match msg {
            ConnectionMessage::SetConfiguration { config } => {
                compiler.set_style(config.style);
                compiler.compiler_configuration(InternalToken).enable_experimental =
                    config.enable_experimental;
            }
            ConnectionMessage::ShowPreview { preview_component, file_cache } => {
                tracing::debug!(
                    "Cached files: {:#?}",
                    file_cache.iter().map(|entry| entry.key().to_string()).collect::<Vec<_>>()
                );
                let compilation_result = if let Some(entry) = file_cache.get(
                    preview_component.url.to_file_path().unwrap().as_os_str().to_str().unwrap(),
                ) && let CacheEntry::Ready(file) = &*entry
                {
                    tracing::debug!("Fetched file {} from cache.", preview_component.url);
                    compiler
                        .build_from_source(
                            str::from_utf8(&file.contents).unwrap().to_owned(),
                            preview_component.url.path().into(),
                        )
                        .await
                } else {
                    tracing::debug!("Failed fetching file {} from cache.", preview_component.url);
                    compiler.build_from_path(preview_component.url.path()).await
                };
                if compilation_result.has_errors() {
                    let mut build_diagnostics = BuildDiagnostics::default();
                    for d in compilation_result.diagnostics() {
                        tracing::warn!("Compiler error: {d}");
                        build_diagnostics.push_compiler_error(d);
                    }

                    if let Err(err) = inner_window.set_property(
                        "message",
                        SharedString::from(build_diagnostics.to_string_vec().join("\n")).into(),
                    ) {
                        tracing::error!("Failed setting property: {err}");
                    }

                    let message = PreviewToLspMessage::Diagnostics {
                        uri: preview_component.url,
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

                    continue;
                }
                if let Err(err) = inner_window.set_property("message", SharedString::new().into()) {
                    tracing::error!("Failed setting property: {err}");
                }

                let Some(component) = preview_component
                    .component
                    .as_deref()
                    .or_else(|| compilation_result.component_names().next())
                    .and_then(|name| compilation_result.component(name))
                else {
                    if let Err(err) = inner_window
                        .set_property("message", SharedString::from("Component not found").into())
                    {
                        tracing::error!("Failed setting property: {err}");
                    }
                    tracing::error!("Component not found");
                    continue;
                };

                let Ok(new_instance) =
                    component.create_with_existing_window(instance.window()).inspect_err(|err| {
                        if let Err(err) = inner_window
                            .set_property("message", SharedString::from(format!("{err}")).into())
                        {
                            tracing::error!("Failed setting property: {err}");
                        }
                        tracing::warn!("Platform error: {err}");
                    })
                else {
                    return Ok(());
                };

                if let Err(err) = new_instance.show() {
                    if let Err(err) = inner_window
                        .set_property("message", SharedString::from(format!("{err}")).into())
                    {
                        tracing::error!("Failed setting property: {err}");
                    }
                    tracing::warn!("Platform error: {err}");
                } else {
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
                    inner_window = main_ui
                        .create_with_existing_window(instance.window())
                        .unwrap_or_else(|_| main_ui.create().unwrap());
                    if let Err(err) = inner_window
                        .set_property("address", SharedString::from(local_ip_str.join("\n")).into())
                    {
                        tracing::error!("Failed setting property: {err}");
                    }
                    inner_window.show().unwrap();
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
