# Purity

Slint's property evaluation is lazy and "reactive". Property
bindings are evaluated when reading the property value. Dependencies between properties are
automatically discovered during property evaluation. The property stores the
result of the evaluation. When a property changes, all dependent properties get
notified, so that the next time their value is read, their binding is re-evaluated.

For any reactive system to work well, bindings evaluation should be "pure":
Evaluating a property shouldn't have side-effects on other properties or change
any observable state but the property itself. Side effects are problematic
because it's not always clear when they will happen: Lazy evaluation may change
their order or affect whether they happen at all. In addition, changes to
properties during their binding evaluation due to a side-effect may result in
unexpected behavior.

For this reason, bindings in Slint _must_ be pure. The Slint compiler enforces
code in pure contexts to be free of side effects. Pure contexts include binding
expressions, bodies of pure functions, and bodies of pure callback handlers.
In such a context, it's not allowed to change a property, or call a non-pure
callback or function.

Annotate callbacks and public functions with the `pure` keyword to make them
accessible from property bindings and other pure callbacks and functions.

The purity of private functions is automatically inferred. Annotated them
explicitly with `pure` to enforce their purity.

```slint,no-preview
export component Example {
    pure callback foo() -> int;
    public pure function bar(x: int) -> int
    { return x + foo(); }
}
```
