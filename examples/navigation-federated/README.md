# Navigation (federated, multi-team)

A multi-screen app composed from **independently-authored feature modules** using
the experimental `navigator` federation seams. It mirrors how separate teams,
each on their own release cycle, plug features into one app that an integration
team assembles.

## Who owns what

| File | Owner | Role |
| --- | --- | --- |
| `ui/contract.slint` | platform team | the `FeatureNav` contract (`@version` / `@uri`) + `HostServices` capabilities |
| `ui/media.slint` | Media team | `MediaFeature` with `implement FeatureNav <=> self`, `needs HostServices`, its own navigator |
| `ui/settings.slint` | Settings team | `SettingsFeature`, same contract |
| `ui/app.slint` | integration team | mounts each feature at a shell route and binds the capabilities |
| `main.rs` | integration team | supplies the external plugin as a `ComponentFactory` |

The example shows every federation seam:

- **build-time mount** — `mount MediaFeature via FeatureNav { ... }` instantiates a
  compile-time feature and binds the capabilities it `needs`.
- **external mount** — `mount extern via FeatureNav { component-factory: ... }` mounts
  a plugin delivered at runtime. `main.rs` builds it via `slint_interpreter` and passes
  it in as a `slint::ComponentFactory`; it could equally be shipped as its own binary.
- **versioned, deep-linkable contract** — `@version(1)` and `@uri("app://feature")` on
  the contract's routes.

No team edits another team's file.

## Experimental features

`navigator` and its federation seams are experimental. `build.rs` enables them
for the `slint!` macro:

```rust
println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
```

Run it with:

```sh
cargo run -p navigation-federated
```
