/*!
The module responsible for the code generation.

There is one sub module for every language
*/

mod cpp;

pub fn generate(component: &crate::lower::LoweredComponent) {
    println!("{}", cpp::generate(component));
}
