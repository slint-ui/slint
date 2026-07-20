# Navigation (federated, multi-team)

A multi-screen app composed from **independently-authored feature modules** using
the experimental `navigator` federation seams. It mirrors how separate teams,
each on their own release cycle, plug features into one app that an integration
team assembles.

## Who owns what

| File | Owner | Role |
| --- | --- | --- |
| `ui/contract.slint` | platform team | the `FeatureNav` navigation contract + `HostServices` capabilities |
| `ui/media.slint` | Media team | `MediaFeature implements FeatureNav`, `needs HostServices`, its own navigator |
| `ui/settings.slint` | Settings team | `SettingsFeature`, same contract |
| `ui/app.slint` | integration team | mounts both features at shell routes and binds the capabilities |

The features conform to a shared contract and declare what they `needs`; the
shell `mount`s each `via FeatureNav` and binds the host services. No team edits
another team's file.

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
