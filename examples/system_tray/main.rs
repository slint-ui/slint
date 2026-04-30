// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    export component ExampleTray inherits SystemTray {
        icon: @image-url("favicon-white.png");

        // Flip-flops when the user clicks "Enable submenu" / "Disable submenu";
        // since the `enabled` binding on the sub-menu reads this property, the menu
        // rebuilds to reflect the new state on every toggle.
        in-out property <bool> more-enabled: true;

        activated => {
            more-enabled = !more-enabled;
        }

        callback quit();

        Menu {
            MenuItem {
                title: more-enabled ? "Disable submenu" : "Enable submenu";
                activated => { more-enabled = !more-enabled; }
            }
            Menu {
                title: "More";
                enabled: more-enabled;
                MenuItem { title: "Nested A"; }
                MenuItem { title: "Nested B"; }
                Menu {
                    title: "Deeper";
                    MenuItem { title: "Way down"; }
                }
            }
            MenuSeparator {}
            MenuItem {
                title: "Quit";
                activated => { quit(); }
            }
        }
    }
}

fn main() {
    let tray = ExampleTray::new().unwrap();
    tray.on_quit(|| slint::quit_event_loop().unwrap());
    slint::run_event_loop().unwrap();
}
