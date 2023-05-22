## simple examples - hello7

Button clicked example. 

This shows how to do something when a Button is clicked. 

The way this happens in slint is via something called "callbacks".
Callbacks are basically very similar to Functions, but can also be
thought of as similar to Methods in object oriented languages.
They are not exactly like either though. Callbacks are callbacks. 

Button has a built-in callback called "clicked" which is run whenever
the button is clicked. 

This example first displays the text Hello Button in a button, but changes
it to "Hello Click!" when the button is clicked, by having code inside
of a callback called 'clicked'.

This can be run with slint-viewer:
 
     slint-viewer hello7.slint

To use with a keyboard, first hit 'tab', then press 'spacebar' to click button
