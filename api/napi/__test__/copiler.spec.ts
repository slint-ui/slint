// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'
var Jimp = require("jimp");

import { ComponentCompiler, ComponentDefinition, ComponentInstance } from '../index'

test('get/set include paths', (t) => {
  let compiler = new ComponentCompiler;

  t.is(compiler.includePaths.length, 0);

  compiler.includePaths = ["path/one/", "path/two/", "path/three/"];

  t.deepEqual(compiler.includePaths, ["path/one/", "path/two/", "path/three/"]);
})

test('get/set style', (t) => {
  let compiler = new ComponentCompiler;

  t.is(compiler.style, null);

  compiler.style = "fluent";
  t.is(compiler.style, "fluent");
})

test('get/set build from source', (t) => {
  let compiler = new ComponentCompiler;

  let definition = compiler.buildFromSource(`export component App {}`, "");
  t.not(definition, null);
  t.is(definition!.name, "App");
})

test('constructor error ComponentDefinition and ComponentInstance', (t) => {
  const componentDefinitionError = t.throws(() => { new ComponentDefinition });
  t.is(componentDefinitionError?.message, "ComponentDefinition can only be created by using ComponentCompiler.");

  const componentInstanceError = t.throws(() => { new ComponentInstance });
  t.is(componentInstanceError?.message, "ComponentInstance can only be created by using ComponentCompiler.");
})