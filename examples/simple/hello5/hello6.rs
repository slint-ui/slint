// simple examples - hello5
//
// This is progressing hello5 by having a Click in Slint call a Rust function
//

slint::include_modules!();

fn main () {
    let hello = Hello5::new().unwrap();

	// This is the key thing for this example versus the last. 
	// instead of setting data in the Slint global singleton, we are
	// getting data out of slint, and saving it as a Rust String.
	// 
	// same as with hello4, the hello.global::<HelloString> is accessing
	// the global singleton by name.
	//
	// then we use get underscore propertyname , in this case the property
	// name is data, so get_data, to retrieve the text from slint
	//
	// into() is used to convert from the Slint::SharedString into a rust String
	//
	let rustmsg:String = hello.global::<HelloString>().get_data().into();

	println!("{}, but being printed from inside of Rust!",rustmsg);

	hello.run().unwrap();
}

