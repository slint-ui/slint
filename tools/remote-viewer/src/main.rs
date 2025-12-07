// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

mod service_model;

slint::include_modules!();

#[allow(unused)]
const SERVICE_TYPE: &str = "_slint-preview._tcp.local.";

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new().unwrap();
    let controller = service_model::ServiceModelController::default();
    let adapter = main_window.global::<ServiceListAdapter>();
    let model = Rc::new(slint::MapModel::new(controller, |task| {
        #[cfg(target_vendor = "apple")]
        return ServiceListViewItem { host: task.host_name().into() };
        #[cfg(not(target_vendor = "apple"))]
        ServiceListViewItem { host: task.host.into() }
    }));
    adapter.set_services(model.clone().into());
    adapter.on_select(|_index| todo!());

    let mut mdns_browser = {
        #[cfg(target_vendor = "apple")]
        {
            use zeroconf_tokio::{prelude::*, ServiceType};

            let browser = zeroconf_tokio::MdnsBrowser::new(
                ServiceType::new("slint-preview", "tcp").map_err(Box::new).unwrap(),
            );
            let mut mdns_browser = zeroconf_tokio::MdnsBrowserAsync::new(browser).unwrap();
            mdns_browser.start().await.unwrap();
            mdns_browser
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            let mdns = mdns_sd::ServiceDaemon::new().unwrap();
            mdns.browse(SERVICE_TYPE).unwrap()
        }
    };

    slint::spawn_local(async move {
        while let Ok(event) = {
            #[cfg(target_vendor = "apple")]
            {
                mdns_browser.next().await.unwrap()
            }
            #[cfg(not(target_vendor = "apple"))]
            {
                mdns_browser.recv_async().await
            }
        } {
            eprintln!("MDNS Event: {event:?}");
            #[cfg(target_vendor = "apple")]
            match event {
                zeroconf_tokio::BrowserEvent::Add(added) => {
                    model.source_model().insert(added);
                }
                zeroconf_tokio::BrowserEvent::Remove(removal) => {
                    model.source_model().remove(&removal);
                }
            }
            #[cfg(not(target_vendor = "apple"))]
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
                    model.source_model().insert(*resolved);
                }
                mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                    model.source_model().remove(&fullname);
                }
                _ => {}
            }
        }
    })
    .unwrap();

    main_window.run()
}
