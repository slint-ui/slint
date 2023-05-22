// simple examples - hello5

slint::include_modules!();

fn main () {
    let hello = Hello5::new().unwrap();

	// We can get the "data" property of Hello5 Slint widget by 
	// referring to it with get_data() (the word "get" then underscore 
	// then the property name)
	//
	// into() is used to convert from the Slint::SharedString into a rust String
	//
	let rustmsg:String = hello.get_data().into();

	println!("{}, but being printed from inside of Rust!",rustmsg);

	hello.run().unwrap();
}

