/*!
# The SixtyFPS compiler

The different modules tage the source code and transform into data structures
according to the following schema

```text
source code -> parser -> object_tree -> lower -> generator
```

*/

pub mod diagnostics;
pub mod generator;
pub mod lower;
pub mod object_tree;
pub mod parser;
pub mod typeregister;
