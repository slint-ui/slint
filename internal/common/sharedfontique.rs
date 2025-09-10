pub use fontique;

thread_local! {
    pub static COLLECTION: std::cell::RefCell<fontique::Collection> = Default::default()
}
