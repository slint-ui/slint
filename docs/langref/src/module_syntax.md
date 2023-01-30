## Module Syntax

The following syntax is supported for importing types:

```slint,ignore
import { export1 } from "module.slint";
import { export1, export2 } from "module.slint";
import { export1 as alias1 } from "module.slint";
import { export1, export2 as alias2, /* ... */ } from "module.slint";
```

The following syntax is supported for exporting types:

```slint,ignore
// Export declarations
export component MyButton inherits Rectangle { /* ... */ }

// Export lists
component MySwitch inherits Rectangle { /* ... */ }
export { MySwitch };
export { MySwitch as Alias1, MyButton as Alias2 };

// Re-export all types from other module
export * from "other_module.slint";
```
