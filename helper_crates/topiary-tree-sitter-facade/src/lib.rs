mod error;
mod input_edit;
mod language;
mod logger;
mod node;
mod parser;
mod point;
mod query;
mod query_capture;
mod query_cursor;
mod query_match;
mod query_predicate;
mod range;
mod tree;
mod tree_cursor;

pub use error::*;
pub use input_edit::*;
pub use language::*;
pub use logger::*;
pub use node::*;
pub use parser::*;
pub use point::*;
pub use query::*;
pub use query_capture::*;
pub use query_cursor::*;
pub use query_match::*;
pub use query_predicate::*;
pub use range::*;
pub use tree::*;
pub use tree_cursor::*;

pub struct TreeSitter;

impl TreeSitter {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn init() -> Result<(), wasm_bindgen::JsError> {
        topiary_web_tree_sitter_sys::TreeSitter::init().await
    }
}
