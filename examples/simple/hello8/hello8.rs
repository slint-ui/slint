
use slint::SharedString;

slint::include_modules!();

fn lower_string( x: SharedString ) -> SharedString {
	format!("{}, lowercased by rust!", x.as_str().to_lowercase()).into()
}

fn main () {
    let hello = Hello8::new().unwrap();

	// here we connect the Slint Global function "lowercase" to our own
	// function lower_sring. this uses the 'on_x' feature of Slint where
	// x can be any callback listed in the Slint code.
	hello.global::<Logic>().on_lowercase( lower_string );

	hello.run().unwrap();
}

