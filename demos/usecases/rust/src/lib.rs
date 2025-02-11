// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = App::new().unwrap();

    virtual_keyboard::init(&app);
    data::init(&app);

    app.run().unwrap();
}

mod virtual_keyboard {
    use super::*;
    use slint::*;

    pub fn init(app: &App) {
        let weak = app.as_weak();
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
    use super::*;
    use slint::*;

    pub fn init(app: &App) {
        let mail_box_adapter = MailBoxViewAdapter::get(app);

        let mails = VecModel::from_slice(&[

         CardListViewItem{
            title: "Simon Hausmann".into(),
            note: "1 hour ago".into(),
            sub_title: "Meeting tomorrow".into(),
            caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into()
        },
//       CardListViewItem  { title: "Tobias Hunger".into(), note: "1 day ago".into(), sub_title: "Meeting tomorrow".into(),  caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into() },
//       CardListViewItem  {
//             title: "Olivier Goffart".into(),
//             note: "2 hour ago".into(),
//             sub_title: "Meeting tomorrow".into(),
//             caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into()
//         },
//      CardListViewItem   {
//             title: "Aurindam Jana".into(),
//             note: "5 hour ago".into(),
//             sub_title: "Meeting tomorrow".into(),
//             caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."
// .into()
//         },
//      CardListViewItem   {
//             title: "Simon Hausmann".into(),
//             note: "7 hour ago".into(),
//             sub_title: "Meeting tomorrow".into(),
//             caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."
// .into()
//         },
//       CardListViewItem  { title: "Tobias Hunger".into(), note: "1 day ago".into(), sub_title: "Meeting tomorrow".into(),  caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."into() },
//       CardListViewItem  {
//             title: "Olivier Goffart".into(),
//             note: "8 hour ago".into(),
//             sub_title: "Meeting tomorrow".into(),
//             caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."
// .into()
//         },
//        CardListViewItem {
//             title: "Aurindam Jana".into(),
//             note: "9 hour ago".into(),
//             sub_title: "Meeting tomorrow".into(),
//             caption: "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."
// .into()
//         }
        ]);

        mail_box_adapter.set_mails(mails.into());
    }
}
