# Navigation

A minimal multi-screen app that demonstrates the Slint navigation convention using
only stable language features.

The convention:

- A user-declared `enum Route`, one value per screen.
- A root component with `in-out property <Route> current-route`.
- One conditional child per route: `if current-route == Route.Home : HomeScreen { ... }`.
- Navigation by assigning the route property: `current-route = Route.Details;`.

The visual editor's flow map understands this shape. See the docs page
"Navigation" under Guide > App Development for the full description.

Run it with:

```sh
cargo run -p navigation
```
