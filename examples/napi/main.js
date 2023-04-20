import { Instance } from 'slint-ui';

import * as fs from 'fs';
const fileName = "window.slint";
const fileData = fs.readFileSync(fileName, "utf8");

console.log(fileData);

// run(fileData).run();

var instance = new Instance(fileData);
instance.run();

// console.log(run(fileData))