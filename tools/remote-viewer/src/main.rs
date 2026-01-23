// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::Model as _;
#[cfg(target_vendor = "apple")]
use zeroconf_tokio::txt_record::TTxtRecord as _;

mod connection;
mod service_model;

slint::include_modules!();

#[allow(unused)]
const SERVICE_TYPE: &str = "_slint-preview._tcp.local.";

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new().unwrap();
    let api = main_window.global::<Api>();

    let controller = service_model::ServiceModelController::default();
    let adapter = main_window.global::<ServiceListAdapter>();
    let model = Rc::new(slint::MapModel::new(controller, |task| {
        #[cfg(target_vendor = "apple")]
        return ServiceListViewItem {
            filename: task
                .txt()
                .as_ref()
                .and_then(|txt| txt.get("slint_filename").map(|record| record.into()))
                .unwrap_or_default(),
            host: task.host_name().into(),
            port: (*task.port()) as i32,
        };
        #[cfg(not(target_vendor = "apple"))]
        ServiceListViewItem {
            filename: task.get_property_val_str("slint_filename").unwrap_or_default().into(),
            host: (&task.host).into(),
            port: task.get_port() as i32,
        }
    }));
    adapter.set_services(model.clone().into());
    let inner_model = Rc::downgrade(&model);
    adapter.on_select(move |index| {
        if let Some(model) = inner_model.upgrade()
            && let Some(item) = model.source_model().row_data(index as usize)
        {
            todo!();
        }
    });

    #[allow(unused_mut)] // for non-apple platforms
    let mut mdns_browser = {
        #[cfg(target_vendor = "apple")]
        {
            use zeroconf_tokio::{ServiceType, prelude::*};

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
