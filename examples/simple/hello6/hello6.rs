// simple examples - hello6
//
// This is progressing hello4 by retrieveing data from slint
// and using it inside of rust
//

slint::include_modules!();

fn printmsg() {
	println!("Hello World 6! Printed from inside of Rust!");
}

fn main () {
    let hello = Hello6::new().unwrap();

	// This is the key thing for this example versus the last. 
	// We are asking Slint to tell Rust when an element has been clicked,
	// and then do something when that happens.
	//
	hello.global::<HelloString>().on_clicked(printmsg)

	hello.run().unwrap();
}

