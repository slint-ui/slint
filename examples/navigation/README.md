# Navigation

A minimal multi-screen app built with the experimental `navigator` construct.

```slint
navigator (current-route) {
    Route.Home: HomeScreen { }
    Route.Details: DetailsScreen { }
    Route.Settings: SettingsScreen { }
}
```

- The set of screens is a compiler-resolved route table; the active screen is the
  `current-route` property you drive at runtime.
- The navigator adds a history API to the declaring component: `navigate(route)`,
  `back()`, and `can-go-back`. This example uses them for the back buttons.

For the std-widgets and Material presentations of the same route model, see the
`navigation-std` example. The full description is on the docs page "Navigation"
under Guide > App Development.

## Experimental features

`navigator` is experimental. `build.rs` enables it for the `slint!` macro that
compiles `navigation.slint`:

```rust
println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
```

Run it with:

```sh
cargo run -p navigation
```
