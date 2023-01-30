## Bindings

The expression on the right of a binding is automatically re-evaluated when the expression changes.

In the following example, the text of the button is automatically changed when the button is pressed, because
changing the `counter` property automatically changes the text.

```slint
import { Button } from "std-widgets.slint";
export component Example inherits Window {
    preferred-width: 50px;
    preferred-height: 50px;
    Button {
        property <int> counter: 3;
        clicked => { self.counter += 3 }
        text: self.counter * 2;
    }
}
```

The re-evaluation happens when the property is queried. Internally, a dependency will be registered
for any property accessed while evaluating this binding. When the dependent properties are changed,
all the dependent bindings are marked dirty. Callbacks in native code by default do not depend on
any properties unless they query a property in the native code.
