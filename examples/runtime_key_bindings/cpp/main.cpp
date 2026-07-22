// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "main_window.h"

#include <iostream>
#include <string_view>
#include <vector>

int main()
{
    auto window = MainWindow::create();

    window->on_shortcut_activated([&](const slint::SharedString &action) {
        if (action == "save") {
            std::cout << "Save" << std::endl;
        } else if (action == "undo") {
            std::cout << "Undo" << std::endl;
        } else if (action == "user") {
            std::cout << "User shortcut (" << window->get_user_shortcut().to_string() << ")"
                      << std::endl;
        } else if (action == "reassign-ctrl-p") {
            auto keys = slint::Keys::from_parts({ "Control", "P" });
            if (keys) {
                std::cout << "Reassigned to " << keys->to_string() << std::endl;
                window->set_user_shortcut(*keys);
            }
        }
    });

    // Capture a key event and turn it into a Keys value.
    // This enables graphical configuration of keyboard shortcuts.
    window->on_key_event([&](const slint::private_api::KeyEvent &event) {
        std::vector<std::string_view> parts;
        if (event.modifiers.control) {
            parts.push_back("Control");
        }
        if (event.modifiers.alt) {
            parts.push_back("Alt");
        }
        if (event.modifiers.shift) {
            parts.push_back("Shift");
        }
        if (event.modifiers.meta) {
            parts.push_back("Meta");
        }
        parts.push_back(std::string_view(event.text));
        auto keys = slint::Keys::from_parts(parts);
        if (keys.has_value()) {
            std::cout << "Captured shortcut: " << keys->to_string() << std::endl;
            window->set_user_shortcut(keys.value());
        } else {
            std::cerr << "Invalid shortcut" << std::endl;
        }
    });

    std::cout << "Press Ctrl+S, Ctrl+Z, or Ctrl+E (default user shortcut)" << std::endl;
    std::cout << "Click 'Capture shortcut' then press a key combo to reassign" << std::endl;

    window->run();
}
