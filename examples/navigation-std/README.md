# Navigation (std-widgets)

A multi-screen app whose navigation chrome is built from **std-widgets**, driving
the experimental `navigator` construct. It pairs with the Material navigation
example to show one route model behind different widget libraries.

The chrome is a segmented tab bar assembled from a row of std `Button`s (std-widgets
has no public tab-bar/segmented control). It binds to the navigator's int-index
adapter:

- the highlighted segment follows `current-route-index`,
- clicking a segment navigates by ordinal via `navigate-index(index)`.

## Why the wiring lives in `main.rs`

The navigator's int-index adapter (`current-route-index` / `navigate-index`) is
synthesized *after* expression resolution, so it cannot be referenced from
`.slint`. The navigator therefore sits on the root `Window`, where its adapter
members land on the component's public API, and `main.rs` bridges them to the tab
bar's `current-index` / `selected(int)`.

## Experimental features

`navigator` is experimental. `build.rs` enables it for the `slint!` macro that
compiles `navigation.slint`:

```rust
println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
```

Run it with:

```sh
cargo run -p navigation-std
```
