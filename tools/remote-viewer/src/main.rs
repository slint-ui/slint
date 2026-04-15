// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_core::InternalToken;
use mdns_sd::ServiceDaemon;
use slint::ComponentHandle;
use tokio::sync;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _};
#[cfg(target_vendor = "apple")]
use zeroconf_tokio::txt_record::TTxtRecord as _;

mod compilation;
mod connection;

slint::include_modules!();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    let mdns = ServiceDaemon::new()?;

    let (message_sender, mut message_receiver) = sync::mpsc::unbounded_channel();

    let connection = Rc::new(
        connection::Connection::listen(move |msg| {
            let _ = message_sender.send(msg);
        })
        .await?,
    );

    let service = connection.service()?;
    mdns.register(service)?;

    let window = EmptyWindow::new()?;
    let mut compiler = compilation::init_compiler(Rc::downgrade(&connection));

    let inner_window = window.clone_strong();
    slint::spawn_local(async move {
        let mut last_connection = None;
        while let Some(msg) = message_receiver.recv().await {
            match msg {
                connection::ConnectionMessage::SetConfiguration { config } => {
                    compiler.set_style(config.style);
                    compiler.compiler_configuration(InternalToken).enable_experimental =
                        config.enable_experimental;
                }
                connection::ConnectionMessage::ShowPreview { preview_component } => {
                    let compilation_result =
                        compiler.build_from_path(preview_component.url.path()).await;
                    if compilation_result.has_errors() {
                        let mut build_diagnostics = BuildDiagnostics::default();
                        for d in compilation_result.diagnostics() {
                            tracing::warn!("Compiler error: {d}");
                            build_diagnostics.push_compiler_error(d);
                        }

                        inner_window.set_message(build_diagnostics.diagnostics_as_string().into());
                        continue;
                    }
                    inner_window.set_message("".into());

                    let Some(component) = preview_component
                        .component
                        .as_deref()
                        .or_else(|| compilation_result.component_names().next())
                        .and_then(|name| compilation_result.component(name))
                    else {
                        inner_window.set_message("Component not found".into());
                        tracing::error!("Component not found");
                        continue;
                    };

                    let Ok(instance) = component
                        .create_with_existing_window(inner_window.window())
                        .inspect_err(|err| {
                            inner_window.set_message(format!("{err}").into());
                            tracing::warn!("Platform error: {err}");
                        })
                    else {
                        return;
                    };

                    if let Err(err) = instance.show() {
                        inner_window.set_message(format!("{err}").into());
                        tracing::warn!("Platform error: {err}");
                    }
                }
                connection::ConnectionMessage::HighlightFromEditor { url, offset } => {}
                connection::ConnectionMessage::Connected { remote_addr } => {
                    inner_window.set_message(format!("Connected to {remote_addr}").into());
                    last_connection = Some(remote_addr);
                }
                connection::ConnectionMessage::Disconnected { remote_addr } => {
                    if last_connection == Some(remote_addr) {
                        // tracing::error!("Platform error: {err}");
                    }
                }
            }
        }
    })?;

    let local_port = connection.local_port();
    let local_ip_str: Vec<String> =
        connection.local_ips().into_iter().map(|ip| format!("{ip}:{local_port}")).collect();
    window.set_address(local_ip_str.join("\n").into());

    window.show().inspect_err(|err| tracing::error!("window show: {err}"))?;

    slint::run_event_loop().inspect_err(|err| tracing::error!("slint event loop: {err}"))?;

    mdns.shutdown().inspect_err(|err| tracing::error!("mdns shutdown: {err}"))?;

    // #[allow(unused_mut)] // for non-apple platforms
    // let mut mdns_browser = {
    //     #[cfg(target_vendor = "apple")]
    //     {
    //         use zeroconf_tokio::{ServiceType, prelude::*};

    //         let browser = zeroconf_tokio::MdnsBrowser::new(
    //             ServiceType::new("slint-preview", "tcp").map_err(Box::new).unwrap(),
    //         );
    //         let mut mdns_browser = zeroconf_tokio::MdnsBrowserAsync::new(browser).unwrap();
    //         mdns_browser.start().await.unwrap();
    //         mdns_browser
    //     }
    //     #[cfg(not(target_vendor = "apple"))]
    //     {
    //         let mdns = mdns_sd::ServiceDaemon::new().unwrap();
    //         mdns.browse(SERVICE_TYPE).unwrap()
    //     }
    // };

    // slint::spawn_local(async move {
    //     while let Ok(event) = {
    //         #[cfg(target_vendor = "apple")]
    //         {
    //             mdns_browser.next().await.unwrap()
    //         }
    //         #[cfg(not(target_vendor = "apple"))]
    //         {
    //             mdns_browser.recv_async().await
    //         }
    //     } {
    //         eprintln!("MDNS Event: {event:?}");
    //         #[cfg(target_vendor = "apple")]
    //         match event {
    //             zeroconf_tokio::BrowserEvent::Add(added) => {
    //                 model.source_model().insert(added);
    //             }
    //             zeroconf_tokio::BrowserEvent::Remove(removal) => {
    //                 model.source_model().remove(&removal);
    //             }
    //         }
    //         #[cfg(not(target_vendor = "apple"))]
    //         match event {
    //             mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
    //                 model.source_model().insert(*resolved);
    //             }
    //             mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
    //                 model.source_model().remove(&fullname);
    //             }
    //             _ => {}
    //         }
    //     }
    // })
    // .unwrap();

    Ok(())
}
