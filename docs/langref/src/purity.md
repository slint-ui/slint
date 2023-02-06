# Purity

Slint's property evaluation is lazy and "reactive". Property
bindings are evaluated when used, dependencies between properties are
automatically discovered during property evaluation, the evaluation
result is cached. If a property changes, all dependent properties get
dirty, and their cached evaluation result discarded.

For this reactive system to work well, bindings evaluation should be "pure":
Evaluating any property shouldn't have side-effects. Side effects are
problematic because it's not always clear when they will happen: Lazy evaluation
may change their order or affect whether they happen at all. In addition, changes
to properties while their binding is getting evaluated due to some side-effect
in some of properties it depends on may result in unexpected behavior.

For this reason, bindings must be pure. The Slint compiler enforces
code in pure contexts to be free of side effects. Pure contexts include binding
expressions, bodies of pure functions, and bodies of pure callback handlers.
In such a context, it's not allowed to change a property, or call a non-pure
callback or function.

Callbacks and public functions may get annotated with the `pure` keyword.

Private functions may also get annotated with `pure`, otherwise their purity is
automatically inferred.

```slint,no-preview
export component Example {
    pure callback foo() -> int;
    public pure function bar(x: int) -> int
    { return x + foo(); }
}
```
