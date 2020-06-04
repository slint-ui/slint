// Run with
// npm install ../../api/sixtyfps-node && node main.js

// import "sixtyfps";
require("sixtyfps");
// import * as myModule from "../cpptest/hello.60";
let hello = require("../cpptest/hello.60");

let x = new hello.Hello({
    counter : 55,
    minus_clicked: (function () { console.log("Clicked!"); x.counter--; }),
});
x.show();
