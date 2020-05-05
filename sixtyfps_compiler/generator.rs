mod cpp;

pub fn generate(component: &crate::lower::LoweredComponent) {
    println!("{}", cpp::generate(component));
}
