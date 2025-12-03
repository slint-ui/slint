// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use mdns_sd::ServiceDaemon;

mod service_model;

slint::include_modules!();

const SERVICE_TYPE: &str = "_slint-preview._tcp.local.";

fn main() -> Result<(), slint::PlatformError> {
    let mdns = ServiceDaemon::new().unwrap();
    let receiver = mdns.browse(SERVICE_TYPE).unwrap();

    let main_window = MainWindow::new().unwrap();
    let controller = service_model::ServiceModelController::default();
    let adapter = main_window.global::<ServiceListAdapter>();
    let model = Rc::new(slint::MapModel::new(controller, |task| ServiceListViewItem {
        host: task.host.into(),
    }));
    adapter.set_services(model.clone().into());
    adapter.on_select(|_index| todo!());

    slint::spawn_local(async move {
        while let Ok(event) = receiver.recv_async().await {
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
