// simple examples - hello4
//
// This is progressing hello3 by getting data from rust, 
// and showing it inside of the slint ui. 
//

slint::include_modules!();

fn main () {
    let hello = Hello4::new().unwrap();
	let rustmsg = "Hello World 4! This string is from Rust!";

	// The below line is the key new thing for this example. It does several
	// things, as follows:
	//
	// hello.global - we access the global singletons from hello4.slint
	//
	// <HelloString> - we acces the individual Name given to the Singleton
	//
	// set_data - we access the property named data within HelloString
	// note that in the .slint file its called "data", so rust expects 
	// for this to be access by a function named "set_data", in other words
	// "set" followed by underscore followed by the name of the property
	// inside the singleton. 
	//
	// rustmsg.into() - this converts the rust &str into a sling SharedString. 
	//
	hello.global::<HelloString>().set_data( rustmsg.into() );

	hello.run().unwrap();
}

