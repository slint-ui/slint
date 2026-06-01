// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "main_window.h"

#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <string>
#include <string_view>
#include <vector>

namespace {

#ifndef USER_SHORTCUT_CONF_PATH
#    define USER_SHORTCUT_CONF_PATH "user_shortcut.conf"
#endif

constexpr const char *CONFIG_PATH = USER_SHORTCUT_CONF_PATH;

/// Load the first shortcut from `CONFIG_PATH`, if present. The file format is
/// one shortcut per line; parts are whitespace-separated and `#` introduces a
/// comment. Blank lines are ignored.
std::optional<slint::Keys> load_user_shortcut()
{
    std::ifstream in(CONFIG_PATH);
    if (!in) {
        return std::nullopt;
    }
    std::string line;
    while (std::getline(in, line)) {
        if (auto pos = line.find('#'); pos != std::string::npos) {
            line.erase(pos);
        }
        std::istringstream ss(line);
        std::vector<std::string> owned;
        std::string tok;
        while (ss >> tok) {
            owned.push_back(std::move(tok));
        }
        if (owned.empty()) {
            continue;
        }
        std::vector<std::string_view> parts;
        parts.reserve(owned.size());
        for (const auto &s : owned) {
            parts.emplace_back(s);
        }
        return slint::Keys::from_parts(parts);
    }
    return std::nullopt;
}

/// Save the shortcut to `CONFIG_PATH` as a single whitespace-separated line.
/// `to_parts` round-trips losslessly through `from_parts`, so the saved file
/// reloads into an equivalent `Keys` value on the next run.
void save_user_shortcut(const slint::Keys &keys)
{
    std::ofstream out(CONFIG_PATH);
    if (!out) {
        std::cerr << "Failed to open " << CONFIG_PATH << " for writing" << std::endl;
        return;
    }
    out << "# User shortcut for the runtime_key_bindings example.\n"
        << "# One shortcut per line; parts are whitespace-separated.\n";
    auto parts = keys.to_parts();
    for (std::size_t i = 0; i < parts.size(); ++i) {
        if (i > 0) {
            out << ' ';
        }
        out << static_cast<std::string_view>(parts[i]);
    }
    out << '\n';
    std::cout << "Saved shortcut to " << CONFIG_PATH << std::endl;
}

} // namespace

int main()
{
    auto window = MainWindow::create();

    // Restore the previously saved shortcut, if any. Falls back to the default
    // `@keys(Control + E)` baked into the .slint file when no config exists yet.
    if (auto saved = load_user_shortcut()) {
        std::cout << "Loaded shortcut from " << CONFIG_PATH << ": " << saved->to_string()
                  << std::endl;
        window->set_user_shortcut(*saved);
    }

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
                save_user_shortcut(*keys);
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
            save_user_shortcut(*keys);
        } else {
            std::cerr << "Invalid shortcut" << std::endl;
        }
    });

    std::cout << "Press Ctrl+S, Ctrl+Z, or Ctrl+E (default user shortcut)" << std::endl;
    std::cout << "Click 'Capture shortcut' then press a key combo to reassign" << std::endl;
    std::cout << "Reassigned shortcut is saved to " << CONFIG_PATH << " and restored on next launch"
              << std::endl;

    window->run();
}
