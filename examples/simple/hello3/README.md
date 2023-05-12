## simple examples - hello3
#
# This displays Hello World, but this time the slint code is loaded 
# from an external .slint file, built by build.rs at build time.
#
#  cargo run --example hello3
#
# This allows the concept of separation between user-interface and core 
# logic code of a program. It allows the interface to be rapidly 
# developed and changed hundreds of times in a WYSIWYG editor, while 
# the code itself can be written as if it were a headless program, with 
# all the automated testing, strong typing, and other features of a 
# compiled language. Of course this particular program has no logic 
# other than to run and stop. But we are introducing one concept at a 
# time.

# how does it know which slint file to include? 
# it actually depends on the contents of build.rs
#
# see also
# https:#docs.rs/slint-build/latest/slint_build/
# https://slint-ui.com/releases/1.0.0/docs/rust/slint/macro.include_modules.html
#
