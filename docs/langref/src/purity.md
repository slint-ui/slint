# Purity

Slint's property evaluation is lazy and "reactive", which means that property bindings are only evaluated when they are used, and the value is cached.
If any of the dependent properties change, the binding will be re-evaluated the next time the property is queried.
Binding evaluation should be "pure", meaning that it should not have any side effects.
For example, evaluating a binding should not change any other properties.
Side effects are problematic because it is not always clear when they will happen.
Lazy evaluation may change their order or affect whether they happen at all.
In addition, changes to properties while their binding is being evaluated may result in unexpected behavior.

For this reason, bindings are required to be pure. The Slint compiler checks that code in pure contexts has no side effects.
Pure contexts includes binding expressions, bodies of pure functions, and bodies of pure callback handlers.
In such a context, it is not allowed to change a property, or call a non-pure callback or function.

Callbacks and public functions can be annotated with the `pure` keyword.
Private functions can also be annotated with `pure`, otherwise their purity is automatically inferred.

```slint,no-preview
export component Example {
    pure callback foo() -> int;
    public pure function bar(x: int) -> int
    { return x + foo(); }
}
```
