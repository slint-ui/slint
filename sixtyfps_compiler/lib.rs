/*!
# The SixtyFPS compiler

The different modules tage the source code and transform into data structures
according to the following schema

```text
source code -> parser -> object_tree -> lower -> generator
```

*/

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

pub mod diagnostics;
pub mod expression_tree;
pub mod generator;
pub mod lower;
pub mod object_tree;
pub mod parser;
pub mod typeregister;
