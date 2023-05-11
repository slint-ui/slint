// simple examples - hello2
//
// This displays Hello World, but this time the slint code is 
// incorporated into rust code using the slint::slint! macro.
//
//   cargo run
//
slint::slint!{
	export component Hello inherits Text {
    	text: "Hello World!";
	}
}

fn main () {
    let hello = Hello::new().unwrap();
	hello.run().unwrap();
}

