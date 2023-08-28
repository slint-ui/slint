// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod tao_platform;

slint::slint! {

export component App {
    Text {
        text: "Hello World";
    }
}

}

fn main() {
    slint::platform::set_platform(Box::new(tao_platform::TaoPlatform::new())).unwrap();

    let app = App::new().unwrap();
    app.show().unwrap();

    slint::run_event_loop().unwrap()
}
