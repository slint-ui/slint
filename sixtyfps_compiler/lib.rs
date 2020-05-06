pub mod diagnostics;
pub mod generator;
pub mod lower;
pub mod object_tree;
pub mod parser;
pub mod typeregister;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
