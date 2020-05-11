pub mod graphics;

pub mod abi {
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
