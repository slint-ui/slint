// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

fn app() -> Result<App, slint::PlatformError> {
    let app = App::new()?;

    data::init(&app);

    Ok(app)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = app().unwrap();
    virtual_keyboard::init(&app);

    if let Err(slint::SelectBundledTranslationError::LanguageNotFound { .. }) =
        slint::select_bundled_translation(option_env!("LANG").unwrap_or("en"))
    {
        slint::select_bundled_translation("en").unwrap();
    }

    app.run().unwrap();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    let app = app().unwrap();
    app.global::<UsecasesPalette>().set_use_material(true);
    app.run().unwrap();
}

mod virtual_keyboard {
    use super::*;
    use slint::*;

    pub fn init(app: &App) {
        let weak = app.as_weak();

        app.global::<VirtualKeyboardHandler>().set_enabled(true);
        app.global::<VirtualKeyboardHandler>().on_key_pressed({
            move |key| {
                weak.unwrap()
                    .window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.clone() });
                weak.unwrap()
                    .window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key });
            }
        });
    }
}

mod data {
    use std::rc::Rc;

    use super::*;
    use slint::*;

    pub fn init(app: &App) {
        let mail_box_adapter = MailBoxViewAdapter::get(app);

        let message = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".to_string();

        let mails = VecModel::from_slice(&[
            CardListViewItem {
                title: "Simon Hausmann".into(),
                note: "1 hour ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Tobias Hunger".into(),
                note: "1 day ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Olivier Goffart".into(),
                note: "1 day".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Aurindam Jana".into(),
                note: "2 hour ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Simon Hausmann".into(),
                note: "5 hour ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Tobias Hunger".into(),
                note: "7 hours ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Olivier Goffart".into(),
                note: "8 hour ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.clone().into(),
            },
            CardListViewItem {
                title: "Aurindam Jana".into(),
                note: "9 hour ago".into(),
                sub_title: "Meeting tomorrow".into(),
                caption: message.into(),
            },
        ]);

        mail_box_adapter.on_search_text_changed({
            let app_weak = app.as_weak();
            let mails = mails.clone();

            move |text| {
                let mails = mails
                    .clone()
                    .filter(move |e| e.title.to_lowercase().contains(text.to_lowercase().as_str()));
                MailBoxViewAdapter::get(&app_weak.unwrap()).set_mails(Rc::new(mails).into());
            }
        });

        mail_box_adapter.set_mails(mails.into());
    }
}
