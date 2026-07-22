
# Slint Documentation

```
docs/
├── astro/                    # The Astro project that builds the Slint language (DSL) docs
├── building.md               # How to build Slint
├── development.md            # How to develop Slint
├── development/               # Deep dives into specific subsystems (compiler, renderers, layout, ...)
├── testing.md                 # The testing infrastructure
├── internal/                  # Contributor process docs (triage, release, writing style guide)
├── common/                    # Shared Astro components/config for the per-language doc sites
├── cpp/                       # The C++ API doc site (Doxygen -> Markdown -> Astro/Starlight)
├── nodejs/                    # The Node.js API doc site
├── python/                    # The Python API doc site
├── safety/                    # The safety/certification doc site
├── site/                      # The docs.slint.dev landing page
├── slint-doc-generator/       # Rust tool that extracts DSL doc comments for the astro site
├── embedded-tutorials.md      # Embedded tutorials template
├── ios.md                     # How to build slint-viewer for iOS / TestFlight
├── install_qt.md              # How to install Qt
├── torizon.md                 # Deploying to Torizon
├── nightly-release-notes.md   # Release note template
├── release-notes.md           # Release note template
└── release-artifacts.md       # Release artifact listing
```

Start with [building.md](building.md) to get building, and [development.md](development.md)
for the repository structure and contribution workflow. [testing.md](testing.md) documents
the test drivers, and [development/](development/) has one deep-dive file per subsystem
(compiler internals, layout, rendering, the item tree, and more) for agents and contributors
working in that area.
