#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
import { Jimp } from "jimp";

class Filter {
    name: string;
    applyFunction: (image: slint.ImageData) => slint.ImageData;

    constructor(
        name: string,
        applyFunction: (image: slint.ImageData) => slint.ImageData
    ) {
        this.name = name;
        this.applyFunction = applyFunction;
    }
}

class Filters extends slint.Model<string> {
    #filters: Filter[];

    constructor(filters: Filter[]) {
        super();
        this.#filters = filters;
    }

    at(index: number): Filter {
        return this.#filters[index];
    }

    rowCount(): number {
        return this.#filters.length;
    }

    rowData(row: number): string | undefined {
        return this.#filters[row]?.name ?? undefined;
    }

    setRowData(row: number, data: string): void {
        // not needed for this example
        throw new Error("Method not implemented.");
    }
}

const demo = slint.loadFile("../ui/main.slint") as any;
const mainWindow = new demo.MainWindow();

const sourceImage = await Jimp.read("../assets/cat.jpg");

mainWindow.original_image = sourceImage.bitmap;

const filters = new Filters([
    new Filter("Blur", (image) => {
        return Jimp.fromBitmap(image).blur(4).bitmap;
    }),
    new Filter("Brighten", (image) => {
        return Jimp.fromBitmap(image).brightness(1.3).bitmap;
    }),
    new Filter("Darken", (image) => {
        return Jimp.fromBitmap(image).brightness(0.3).bitmap;
    }),
    new Filter("Increase Contrast", (image) => {
        return Jimp.fromBitmap(image).contrast(0.3).bitmap;
    }),
    new Filter("Decrease Contrast", (image) => {
        return Jimp.fromBitmap(image).contrast(-0.3).bitmap;
    }),
    new Filter("Invert", (image) => {
        return Jimp.fromBitmap(image).invert().bitmap;
    }),
]);

mainWindow.filters = filters;

// mainWindow.filter_image = function (index: number) {
//     const filterFunction = filters.at(index).applyFunction;
//     return filterFunction(mainWindow.original_image);
// };
mainWindow.filter_image = function (index: number) {
    const start = performance.now();

    const filterFunction = filters.at(index).applyFunction;
    const result = filterFunction(mainWindow.original_image);

    const end = performance.now();
    const executionTime = end - start;

    console.log(
        `Filter '${filters.at(index).name}' took ${executionTime.toFixed(2)} milliseconds to execute.`,
    );

    return result;
};

await mainWindow.run();
