/*!
The module responsible for the code generation.

There is one sub module for every language
*/

#[cfg(feature = "cpp")]
mod cpp;

pub fn generate(component: &crate::lower::LoweredComponent) {
    #![allow(unused_variables)]
    #[cfg(feature = "cpp")]
    println!("{}", cpp::generate(component));
}
