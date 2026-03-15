#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
import { read, Image, ImageColorModel } from "image-js";

function fromImageData(bitmap: slint.ImageData): Image {
    return new Image(bitmap.width, bitmap.height, {
        data: bitmap.data,
        colorModel: ImageColorModel.RGBA,
    });
}

function toImageData(image: Image): slint.ImageData {
    const raw = image.getRawImage();
    return {
        width: raw.width,
        height: raw.height,
        data: raw.data as Uint8Array,
    };
}

/** Constant (r,g,b,0) image for add/subtract so only RGB change (alpha unchanged, like Rust brighten). */
function constantRgb(w: number, h: number, r: number, g: number, b: number): Image {
    return new Image(w, h, { colorModel: ImageColorModel.RGBA }).fill([r, g, b, 0]);
}

class Filter {
    name: string;
    applyFunction: (image: slint.ImageData) => slint.ImageData;

    constructor(
        name: string,
        applyFunction: (image: slint.ImageData) => slint.ImageData,
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
        return this.#filters[row]?.name;
    }

    setRowData(row: number, data: string): void {
        // not needed for this example
        throw new Error("Method not implemented.");
    }
}

const demo = slint.loadFile(
    new URL("../ui/main.slint", import.meta.url),
) as any;
const mainWindow = new demo.MainWindow();

const imagePath = new URL("../assets/cat.jpg", import.meta.url).pathname;
const image = await read(imagePath);
mainWindow.original_image = toImageData(image.convertColor(ImageColorModel.RGBA));

const filters = new Filters([
    new Filter("Blur", (bitmap) => {
        return toImageData(
            fromImageData(bitmap).gaussianBlur({ sigma: 4 }),
        );
    }),
    new Filter("Brighten", (bitmap) => {
        const img = fromImageData(bitmap);
        return toImageData(img.add(constantRgb(img.width, img.height, 30, 30, 30)));
    }),
    new Filter("Darken", (bitmap) => {
        const img = fromImageData(bitmap);
        return toImageData(img.subtract(constantRgb(img.width, img.height, 30, 30, 30)));
    }),
    new Filter("Increase Contrast", (bitmap) => {
        return toImageData(fromImageData(bitmap).increaseContrast());
    }),
    new Filter("Decrease Contrast", (bitmap) => {
        return toImageData(
            fromImageData(bitmap).level({ outputMin: 32, outputMax: 224 }),
        );
    }),
    new Filter("Invert", (bitmap) => {
        return toImageData(
            fromImageData(bitmap).invert(),
        );
    }),
]);

mainWindow.filters = filters;

mainWindow.filter_image = function (index: number) {
    const filterFunction = filters.at(index).applyFunction;
    return filterFunction(mainWindow.original_image);
};

await mainWindow.run();
