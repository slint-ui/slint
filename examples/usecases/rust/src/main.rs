// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use slint::*;

slint::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = App::new().unwrap();

    // mails
    let _mail_controller = MailController::new(&app);

    app.run().unwrap();
}

#[derive(Clone)]
struct MailController {
    app_weak: Weak<App>,
    accounts: ModelRc<SharedString>,
}

impl MailController {
    pub fn new(app: &App) -> Self {
        let controller = Self {
            app_weak: app.as_weak(),
            accounts: Rc::new(VecModel::from_slice(&[
                SharedString::from("simon.hausmann@slint.dev"),
                SharedString::from("olivier.goffart@slint.dev"),
            ]))
            .into(),
        };

        app.global::<MailSideBarViewAdapter>().set_accounts(controller.accounts.clone().into());

        app.global::<MailSideBarViewAdapter>().on_account_selected({
            let controller = controller.clone();
            move |index| {
                controller.load_mail_boxes(index as usize);
                controller.load_custom_mail_boxes(index as usize);
                controller.load_mails(0);
            }
        });

        app.global::<MailSideBarViewAdapter>().on_box_selected({
            let controller = controller.clone();
            move |_, mail_box| {
                controller.load_mails(mail_box as usize);
            }
        });

        app.global::<MailSideBarViewAdapter>().on_custom_box_selected({
            let controller = controller.clone();
            move |_, mail_box| {
                controller.load_custom_mails(mail_box as usize);
            }
        });

        app.global::<MailSideBarViewAdapter>().invoke_account_selected(0);

        controller
    }

    fn load_mail_boxes(&self, index: usize) {
        let boxes = Rc::new(VecModel::default());
        self.app_weak.unwrap().global::<MailSideBarViewAdapter>().set_current_box(0);
        self.app_weak.unwrap().global::<MailSideBarViewAdapter>().set_current_custom_box(-1);

        match index {
            0 => {
                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Inbox"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_inbox(),
                    message: SharedString::from("5"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Drafts"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_document(),
                    message: SharedString::from("3"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Sent"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_send(),
                    message: SharedString::from("2"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Junk"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_junk(),
                    message: SharedString::from("1"),
                });
            }
            _ => {
                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Inbox"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_inbox(),
                    message: SharedString::from("3"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Drafts"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_document(),
                    message: SharedString::from("4"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Sent"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_send(),
                    message: SharedString::from("3"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Junk"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_junk(),
                    message: SharedString::from("2"),
                });
            }
        }

        boxes.push(NavigationListViewItem {
            text: SharedString::from("Trash"),
            icon: self.app_weak.unwrap().global::<Icons>().get_trash(),
            message: SharedString::default(),
        });

        boxes.push(NavigationListViewItem {
            text: SharedString::from("Archive"),
            icon: self.app_weak.unwrap().global::<Icons>().get_archive(),
            message: SharedString::default(),
        });

        self.app_weak.unwrap().global::<MailSideBarViewAdapter>().set_boxes(boxes.into());
    }

    fn load_custom_mail_boxes(&self, index: usize) {
        let boxes = Rc::new(VecModel::default());
        match index {
            0 => {
                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Social"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_useres(),
                    message: SharedString::from("5"),
                });
            }
            _ => {
                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Updates"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_updates(),
                    message: SharedString::from("2"),
                });

                boxes.push(NavigationListViewItem {
                    text: SharedString::from("Forums"),
                    icon: self.app_weak.unwrap().global::<Icons>().get_message(),
                    message: SharedString::from("4"),
                });
            }
        }

        self.app_weak.unwrap().global::<MailSideBarViewAdapter>().set_custom_boxes(boxes.into());
    }

    fn load_mails(&self, mail_box: usize) {
        let mails = Rc::new(VecModel::default());

        // FIXME: use internal struct with count as usize
        if let Some(mail_box) =
            self.app_weak.unwrap().global::<MailSideBarViewAdapter>().get_boxes().row_data(mail_box)
        {
            if let Ok(count) = mail_box.message.as_str().parse::<usize>() {
                if count > 0 {
                    for i in 0..count {
                        mails.push(CardListViewItem {
                    title: SharedString::from("Tobias Hunger"),
                    note: SharedString::from(std::format!("{} hour ago", i + 1)),
                    sub_title: SharedString::from("Meeting"),
                    caption: SharedString::from("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat")
                });
                    }
                }
            }
        }

        self.app_weak.unwrap().global::<MailBoxViewAdapter>().set_mails(mails.into());
    }

    fn load_custom_mails(&self, mail_box: usize) {
        let mails = Rc::new(VecModel::default());

        // FIXME: use internal struct with count as usize
        if let Some(mail_box) = self
            .app_weak
            .unwrap()
            .global::<MailSideBarViewAdapter>()
            .get_custom_boxes()
            .row_data(mail_box)
        {
            if let Ok(count) = mail_box.message.as_str().parse::<usize>() {
                if count > 0 {
                    for i in 0..count {
                        mails.push(CardListViewItem {
                    title: SharedString::from("Tobias Hunger"),
                    note: SharedString::from(std::format!("{} hour ago", i + 1)),
                    sub_title: SharedString::from("Meeting"),
                    caption: SharedString::from("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat")
                });
                    }
                }
            }
        }

        self.app_weak.unwrap().global::<MailBoxViewAdapter>().set_mails(mails.into());
    }
}
