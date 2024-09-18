// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import { loadFile, loadSource, CompileError, MapModel, ArrayModel, private_api } from "../dist/index.js";

test("MapModel notify rowChanged", (t) => {
    const source = `
    export component App {

      in-out property <[string]> model;
      in-out property <string> changed-items;

      for item in root.model : Text {
          text: item;

          changed text => {
              root.changed-items += self.text;
          }
      }
    }`;
    
    const path = "api.spec.ts";

    private_api.initTesting();
    const demo = loadSource(source, path) as any;
    const instance = new demo.App();
    
    interface Name {
        first: string;
        last: string;
    }

    const nameModel: ArrayModel<Name> = new ArrayModel([
        { first: "Hans", last: "Emil" },
        { first: "Max", last: "Mustermann" },
        { first: "Roman", last: "Tisch" },
    ]);

    const mapModel = new MapModel(nameModel, (data) => {
        return data.last + ", " + data.first;
    });

    instance.model = mapModel;
    
    private_api.send_mouse_click(instance, 5., 5.);
    
    nameModel.setRowData(0, { first: "Simon", last: "Hausmann" });
    nameModel.setRowData(1, { first: "Olivier", last: "Goffart" });
    
    private_api.send_mouse_click(instance, 5., 5.);

    t.is(instance.changed_items, "Goffart, OlivierHausmann, Simon");
 });
