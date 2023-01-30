## Legacy syntax

To keep compatibility with previous version of Slint, the old syntax that declared component with `:=` is still valid

```slint,no-preview
export MyApp := Window {
    //...
}
```

Element position and property lookup is different in the new syntax.
In the new syntax, only property declared within the component are in scope.
In the previous syntax, all properties of bases of `self` and `root` were also in scope.
In the new syntax, elements are centered by default and the constraints are applied to the parent.
