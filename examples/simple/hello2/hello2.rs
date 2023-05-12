// simple examples - hello2
//
// This displays Hello World, but this time the slint code is 
// incorporated into rust code using the slint::slint! macro.
//
//   cargo run
//
slint::slint!{
	export component Hello2 inherits Text {
    	text: "Hello World 2!";
	}
}

fn main () {
    let hello = Hello2::new().unwrap();
	hello.run().unwrap();
}

