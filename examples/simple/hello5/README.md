## simple examples - hello5
#
# This builds on hello4, but instead of transmitting data from Rust to
# Slint, we are instead transmitting data from Slint to Rust.
# 
# This is accomplished again with a Global Singleton, but instead of using
# an "in" property we are using an "out" property in the .slint code
#
# The example will show a window with Hello World 5!, however it will also
# display on the Standard Output (command line) a text similar to the following:
#
#   Hello World 5! This text is from Slint, but being displayed from Rust!
#
# see also https://slint-ui.com/releases/1.0.2/docs/slint/src/reference/globals.html#
#

