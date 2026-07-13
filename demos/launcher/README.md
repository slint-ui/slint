# Demo Launcher

A little helper application for embedded devices that lists the Slint demos
and examples installed on the system and launches them, along with an entry to
start `slint-viewer --remote` to turn the device into a live-preview target
for the Slint IDE integrations.

At startup, the launcher checks a built-in catalog of known demo and example
binary names against the directories in the `PATH` environment variable, and
only shows the ones that are installed. The launcher's own directory is
searched, too: in a development build the workspace's demos and examples land
in the same cargo target directory, so `cargo run -p launcher` shows the ones
that were built.
