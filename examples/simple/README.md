# Simple Examples

These examples are bare bones, basic minimum samples for the absolute noob

## hello1 - A simple demonstration of pure slint, run by slint-viewer alone

    cargo install slint-viewer
    slint-viewer hello1.slint

## hello2 - Slint code inline within rust source code. 
#           This shows the basic capability of creating a stand alone binary 
#           executable that runs a UI.


    cargo run --example hello2

## hello3 - Shows the use of a separate slint file for the user interface
#           and a rust source file for the main binary code. This produces
#           a single self contained binary that runs a GUI of the slint file.

    cargo run --example hello3

