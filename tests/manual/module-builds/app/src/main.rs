// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore blociga
use blogica;
use blogicb;
use random_word;
use std::error::Error;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let blociga_api = ui.global::<blogica::backend::BLogicAAPI>();
    blogica::backend::init(blociga_api);

    let blogicb_api = ui.global::<blogicb::BLogicBAPI>();
    blogicb::init(blogicb_api);

    ui.on_update_blogic_data({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.upgrade().unwrap();
            let blogica_api = ui.global::<blogica::backend::BLogicAAPI>();
            let mut bdata = blogica::backend::BData::default();

            bdata.colors = slint::ModelRc::new(slint::VecModel::from(
                (1..5)
                    .into_iter()
                    .map(|_| {
                        let red = rand::random::<u8>();
                        let green = rand::random::<u8>();
                        let blue = rand::random::<u8>();
                        slint::Color::from_rgb_u8(red, green, blue)
                    })
                    .collect::<Vec<_>>(),
            ));

            bdata.codes = slint::ModelRc::new(slint::VecModel::from(
                (1..5)
                    .into_iter()
                    .map(|_| slint::SharedString::from(random_word::get(random_word::Lang::En)))
                    .collect::<Vec<_>>(),
            ));

            blogica_api.invoke_update(bdata);
        }
    });

    ui.run()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the bug where the compiler treated properties of
    /// globals imported from another crate ("library globals") as constants,
    /// silently dropping bindings that read from them in the consumer.
    ///
    /// `AppWindow::test-status` is bound to `BLogicBAPI.status`, a property
    /// of the `BLogicBAPI` global which is exported by the `blogicb` library
    /// crate. With the bug present, that binding was inlined to the global's
    /// default value at compile time and writes made from the consumer's
    /// Rust code (`api.set_status(...)`) would no longer be observed.
    #[test]
    fn library_global_property_binding_observes_external_writes() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let api = ui.global::<blogicb::BLogicBAPI>();

        api.set_status(slint::SharedString::from("HelloFromRust"));

        assert_eq!(
            ui.get_test_status().as_str(),
            "HelloFromRust",
            "binding `test-status: BLogicBAPI.status` did not observe the \
             Rust-side write — the imported library global's property was \
             inlined as a constant"
        );
    }
}
