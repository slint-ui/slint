#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
import { read, Image, ImageColorModel } from "image-js";

function fromBitmap(bitmap: slint.ImageData): Image {
    return new Image(bitmap.width, bitmap.height, {
        data: bitmap.data,
        colorModel: ImageColorModel.RGBA,
    });
}

function toBitmap(image: Image): slint.ImageData {
    const raw = image.getRawImage();
    return {
        width: raw.width,
        height: raw.height,
        data: raw.data as Uint8Array,
    };
}

/** Apply Rust image crate contrast formula: ((v/255 - 0.5) * percent + 0.5) * 255. */
function applyContrast(image: Image, contrast: number): Image {
    const p = Math.pow((100 + contrast) / 100, 2);
    const clone = image.clone();
    clone.changeEach((v) =>
        Math.round(Math.min(255, Math.max(0, ((v / 255 - 0.5) * p + 0.5) * 255))),
    );
    return clone;
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
const rgbaImage =
    image.colorModel === ImageColorModel.RGBA
        ? image
        : image.convertColor(ImageColorModel.RGBA);
const raw = rgbaImage.getRawImage();
mainWindow.original_image = raw as slint.ImageData;

const filters = new Filters([
    new Filter("Blur", (bitmap) => {
        return toBitmap(
            fromBitmap(bitmap).gaussianBlur({ sigma: 4 }),
        );
    }),
    new Filter("Brighten", (bitmap) => {
        const img = fromBitmap(bitmap).clone();
        img.changeEach((v) => Math.min(255, v + 30));
        return toBitmap(img);
    }),
    new Filter("Darken", (bitmap) => {
        const img = fromBitmap(bitmap).clone();
        img.changeEach((v) => Math.max(0, v - 30));
        return toBitmap(img);
    }),
    new Filter("Increase Contrast", (bitmap) => {
        return toBitmap(
            applyContrast(fromBitmap(bitmap), 30),
        );
    }),
    new Filter("Decrease Contrast", (bitmap) => {
        return toBitmap(
            applyContrast(fromBitmap(bitmap), -30),
        );
    }),
    new Filter("Invert", (bitmap) => {
        return toBitmap(
            fromBitmap(bitmap).invert(),
        );
    }),
]);

mainWindow.filters = filters;

mainWindow.filter_image = function (index: number) {
    const filterFunction = filters.at(index).applyFunction;
    return filterFunction(mainWindow.original_image);
};

await mainWindow.run();
