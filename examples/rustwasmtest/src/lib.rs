#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Using a macro for now.  But there could be others ways to do that
sixtyfps::sixtyfps! {
    SuperSimple := Rectangle {
        color: white;

        Rectangle {
            width: 100;
            height: 100;
            color: blue;
        }
        Rectangle {
            x: 100;
            y: 100;
            width: (100);
            height: {100}
            color: green;
        }
        Text {
            text: "Hello World";
            x: 100;
            y: 200;
            color: black;
            font_pixel_size: 48;
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn wasm_main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    SuperSimple::default().run();
}
