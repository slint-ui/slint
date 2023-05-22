// simple examples - hello5
//
// This is progressing hello5 by having a Click in Slint call a Rust function
//

use slint::SharedString;

slint::include_modules!();

fn lower_string( x: SharedString ) -> SharedString {
	format!("{}, lowercased by rust!", x.as_str().to_lowercase()).into()
}

fn main () {
    let hello = Hello9::new().unwrap();

	hello.global::<Logic>().on_lowercase( lower_string );

	hello.run().unwrap();
}

