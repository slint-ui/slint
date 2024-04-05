// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "app.h"

void init_virtual_keyboard(slint::ComponentHandle<App> app)
{
    app->global<VirtualKeyboardHandler>().on_key_pressed([=](auto key) {
        app->window().dispatch_key_press_event(key);
        app->window().dispatch_key_release_event(key);
    });
}

int main()
{
    auto app = App::create();

    init_virtual_keyboard(app);

    auto mails = std::make_shared<slint::VectorModel<CardListViewItem>>(std::vector {
            CardListViewItem { "Simon Hausmann", "1 hour ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Tobias Hunger", "1 day ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Olivier Goffart", "2 hour ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Aurindam Jana", "5 hour ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Simon Hausmann", "7 hour ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Tobias Hunger", "1 day ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Olivier Goffart", "8 hour ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
            CardListViewItem { "Aurindam Jana", "9 hour ago", "Meeting tomorrow",
                               "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do "
                               "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut "
                               "enim ad minim veniam, quis nostrud exercitation ullamco laboris "
                               "nisi ut aliquip ex ea commodo consequat." },
    });

    app->global<MailBoxViewAdapter>().set_mails(mails);

    app->run();
}
