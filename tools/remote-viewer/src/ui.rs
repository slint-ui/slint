// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{net::SocketAddr, path::Path, rc::Rc, sync::Arc};

use futures_util::FutureExt;
use i_slint_compiler::{diagnostics::BuildDiagnostics, passes::ResourcePreloader};
use i_slint_core::InternalToken;
use i_slint_preview_protocol::PreviewToLspMessage;
use slint::{ComponentHandle as _, SharedString};
use slint_interpreter::ComponentInstance;
use tokio::sync;

use crate::{
    compilation,
    connection::{self, CacheEntry, Connection},
    util,
};

const MAIN_SLINT: &str = include_str!("../ui/main.slint");

pub fn run(address: Option<SocketAddr>, enable_mdns: bool) -> anyhow::Result<()> {
    let mut compiler = compilation::init_compiler(Rc::downgrade(&connection));
    let current_exe = std::env::current_exe().unwrap();
    let compilation_result = compiler
        .build_from_source(MAIN_SLINT.to_owned(), current_exe.parent().unwrap().to_owned(), ())
        .await;
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

    let (quit_sender, quit_receiver) = sync::oneshot::channel();

    let mut inner_window = window.clone_strong();
    let network_thread =
        std::thread::Builder::new().name("network".to_string()).spawn(move || {
            tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(
                async move {
                    #[cfg(not(target_vendor = "apple"))]
                    let mdns = enable_mdns.then(mdns_sd::ServiceDaemon::new).transpose()?;

                    let (message_sender, mut message_receiver) = sync::mpsc::unbounded_channel();

                    let connection = Rc::new(
                        connection::Connection::listen(address, move |msg| {
                            let _ = message_sender.send(msg);
                        })
                        .await?,
                    );

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
                                    i_slint_preview_protocol::SERVICE_TYPE_NAME,
                                    i_slint_preview_protocol::SERVICE_TYPE_PROTOCOL,
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

                    #[cfg(target_vendor = "apple")]
                    if let Some(mdns) = &mut mdns
                        && let Err(err) = mdns.start().await
                    {
                        tracing::error!("Failed to announce service: {err}");
                    }

                    let inner_local_ip_str = local_ip_str.clone();
                    slint::spawn_local(async move {
                        let mut last_connection = None;
                        let mut instance = None;
                        while let Some(msg) = message_receiver.recv().await {
                            match msg {
                                connection::ConnectionMessage::SetConfiguration { config } => {
                                    compiler.set_style(config.style);
                                    compiler
                                        .compiler_configuration(InternalToken)
                                        .enable_experimental = config.enable_experimental;
                                }
                                connection::ConnectionMessage::ShowPreview {
                                    preview_component,
                                    file_cache,
                                } => {
                                    tracing::debug!(
                                        "Cached files: {:#?}",
                                        file_cache
                                            .iter()
                                            .map(|entry| entry.key().to_string())
                                            .collect::<Vec<_>>()
                                    );
                                    let file_cache_preloader = FileCachePreloader {
                                        connection: &connection,
                                        window: &inner_window,
                                    };
                                    let compilation_result = if let Some(entry) = file_cache.get(
                                        preview_component
                                            .url
                                            .to_file_path()
                                            .unwrap()
                                            .as_os_str()
                                            .to_str()
                                            .unwrap(),
                                    ) && let CacheEntry::Ready(file) =
                                        &*entry
                                    {
                                        tracing::debug!(
                                            "Fetched file {} from cache.",
                                            preview_component.url
                                        );
                                        compiler
                                            .build_from_source(
                                                str::from_utf8(&file.contents).unwrap().to_owned(),
                                                preview_component.url.path().into(),
                                                file_cache_preloader,
                                            )
                                            .await
                                    } else {
                                        tracing::debug!(
                                            "Failed fetching file {} from cache.",
                                            preview_component.url
                                        );
                                        compiler
                                            .build_from_path(
                                                preview_component.url.path(),
                                                file_cache_preloader,
                                            )
                                            .await
                                    };
                                    if compilation_result.has_errors() {
                                        let mut build_diagnostics = BuildDiagnostics::default();
                                        for d in compilation_result.diagnostics() {
                                            tracing::warn!("Compiler error: {d}");
                                            build_diagnostics.push_compiler_error(d);
                                        }

                                        if let Err(err) = inner_window.set_property(
                                            "message",
                                            SharedString::from(
                                                build_diagnostics.to_string_vec().join("\n"),
                                            )
                                            .into(),
                                        ) {
                                            tracing::error!("Failed setting property: {err}");
                                        }

                                        let message = PreviewToLspMessage::Diagnostics {
                                            uri: preview_component.url,
                                            version: None,
                                            diagnostics: compilation_result
                                                .diagnostics()
                                                .map(|diagnostic| {
                                                    util::to_lsp_diag(
                                        &diagnostic,
                                        i_slint_compiler::diagnostics::ByteFormat::Utf8,
                                    )
                                                })
                                                .collect(),
                                        };

                                        connection.send(message).ok();

                                        continue;
                                    }
                                    if let Err(err) = inner_window
                                        .set_property("message", SharedString::new().into())
                                    {
                                        tracing::error!("Failed setting property: {err}");
                                    }

                                    let Some(component) = preview_component
                                        .component
                                        .as_deref()
                                        .or_else(|| compilation_result.component_names().next())
                                        .and_then(|name| compilation_result.component(name))
                                    else {
                                        if let Err(err) = inner_window.set_property(
                                            "message",
                                            SharedString::from("Component not found").into(),
                                        ) {
                                            tracing::error!("Failed setting property: {err}");
                                        }
                                        tracing::error!("Component not found");
                                        continue;
                                    };

                                    let Ok(inner_instance) = component
                                        .create_with_existing_window(inner_window.window())
                                        .inspect_err(|err| {
                                            if let Err(err) = inner_window.set_property(
                                                "message",
                                                SharedString::from(format!("{err}")).into(),
                                            ) {
                                                tracing::error!("Failed setting property: {err}");
                                            }
                                            tracing::warn!("Platform error: {err}");
                                        })
                                    else {
                                        return;
                                    };

                                    if let Err(err) = inner_instance.show() {
                                        if let Err(err) = inner_window.set_property(
                                            "message",
                                            SharedString::from(format!("{err}")).into(),
                                        ) {
                                            tracing::error!("Failed setting property: {err}");
                                        }
                                        tracing::warn!("Platform error: {err}");
                                    } else {
                                        instance = Some(inner_instance);
                                    }
                                }
                                connection::ConnectionMessage::HighlightFromEditor { .. } => {}
                                connection::ConnectionMessage::Connected { remote_addr } => {
                                    if let Err(err) = inner_window.set_property(
                                        "message",
                                        SharedString::from(format!("Connected to {remote_addr}"))
                                            .into(),
                                    ) {
                                        tracing::error!("Failed setting property: {err}");
                                    }
                                    last_connection = Some(remote_addr);
                                }
                                connection::ConnectionMessage::Disconnected { remote_addr } => {
                                    if last_connection == Some(remote_addr) {
                                        last_connection = None;
                                        inner_window = instance
                                            .as_ref()
                                            .map(|instance| {
                                                main_ui
                                                    .create_with_existing_window(instance.window())
                                                    .unwrap()
                                            })
                                            .unwrap_or_else(|| main_ui.create().unwrap());
                                        if let Err(err) = inner_window.set_property(
                                            "address",
                                            SharedString::from(inner_local_ip_str.join("\n"))
                                                .into(),
                                        ) {
                                            tracing::error!("Failed setting property: {err}");
                                        }
                                        inner_window.show().unwrap();
                                    }
                                }
                            }
                        }
                    })?;

                    if let Err(err) = window
                        .set_property("address", SharedString::from(local_ip_str.join("\n")).into())
                    {
                        tracing::error!("Failed setting property: {err}");
                    }

                    println!("{}", local_ip_str.join("\n"));

                    quit_receiver.await.ok();

                    #[cfg(not(target_vendor = "apple"))]
                    mdns.map(|mdns| mdns.shutdown())
                        .transpose()
                        .inspect_err(|err| tracing::error!("mdns shutdown: {err}"))?;

                    #[cfg(target_vendor = "apple")]
                    if let Some(mut mdns) = mdns.take() {
                        mdns.shutdown().await?;
                    }
                    Ok(())
                },
            )
        })?;

    window.show().inspect_err(|err| tracing::error!("window show: {err}"))?;

    slint::run_event_loop().inspect_err(|err| tracing::error!("slint event loop: {err}"))?;
    quit_sender.send(()).ok();
    network_thread.join()??;

    Ok(())
}

struct FileCachePreloader<'a> {
    connection: &'a Connection,
    window: &'a ComponentInstance,
}

impl<'p> ResourcePreloader for FileCachePreloader<'p> {
    fn load<'a>(
        &self,
        paths: impl Iterator<Item = &'a str>,
        push: Rc<impl Fn(/* url */ &'a str, /* extension */ String, /* data */ Arc<[u8]>)>,
    ) -> impl Future<Output = ()> {
        if let Err(err) =
            self.window.set_property("message", SharedString::from("Loading resources...").into())
        {
            tracing::error!("Failed setting property: {err}");
        }

        futures_util::future::join_all(paths.map(|path| {
            let push = push.clone();
            async move {
                if path.starts_with("builtin:/") {
                    tracing::debug!("Skipping builtin resource {path}");
                    return;
                }
                tracing::debug!("Preloading file {path}");
                match self.connection.request_file(path.to_owned()).await {
                    Ok(file) => {
                        tracing::debug!("Got file {path} with {} bytes", file.contents.len());
                        let extension = Path::new(&path)
                            .extension()
                            .and_then(std::ffi::OsStr::to_str)
                            .map(std::string::ToString::to_string)
                            .unwrap_or_default();

                        push(path, extension, file.contents.clone());
                    }
                    Err(err) => {
                        tracing::error!("Failed requesting file {path}: {err}");
                    }
                }
            }
        }))
        .map(|_| ())
    }
}
