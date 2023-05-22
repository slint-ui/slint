// simple examples - hello4

slint::include_modules!();

fn main () {
    let hello = Hello4::new().unwrap();

	let rustmsg = "Hello World 4! This string is from Rust!";

	// We can set the value of a Property in Slint code by using the
	// set_xxxx function, where set_ is followed by the name of the property.
	// in this case our property is named "data", inside the Hello4 widget.
	//
	// rustmsg.into() - this converts the rust &str into a sling SharedString. 
	//
	hello.set_data( rustmsg.into() );

	hello.run().unwrap();
}

