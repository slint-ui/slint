// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { DefinitionLike, InstanceLike } from "./backend";
import type { Window } from "./types";
import { translateName } from "./util";

/**
 * Callback type for running the event loop.
 * Each platform provides its own implementation.
 */
export type RunEventLoopFn = () => Promise<unknown>;

let _runEventLoop: RunEventLoopFn | null = null;

/**
 * Sets the event loop runner. Must be called before using Component.run().
 */
export function setRunEventLoop(fn: RunEventLoopFn): void {
    _runEventLoop = fn;
}

/**
 * Component wraps a native component instance, providing show/hide/run.
 *
 * @hidden
 */
export class Component {
    [key: string]: unknown;
    #instance: InstanceLike;

    constructor(instance: InstanceLike) {
        this.#instance = instance;
    }

    get window(): Window {
        return this.#instance.window() as Window;
    }

    /**
     * @hidden
     */
    get component_instance(): InstanceLike {
        return this.#instance;
    }

    async run() {
        this.show();
        try {
            if (_runEventLoop) {
                await _runEventLoop();
            }
        } finally {
            this.hide();
        }
    }

    show(): void {
        this.window.show();
    }

    hide(): void {
        this.window.hide();
    }
}

/**
 * Wraps a component instance with dynamic property getters/setters,
 * callback bindings, function bindings, and global sub-objects.
 *
 * This is the core shared logic extracted from the Node.js API.
 */
function wrapInstance(instance: InstanceLike): Component {
    const componentHandle = new Component(instance);
    const definition = instance.definition();

    // Properties
    definition.properties.forEach((prop) => {
        const propName = translateName(prop.name);

        if (componentHandle[propName] !== undefined) {
            console.warn("Duplicated property name " + propName);
        } else {
            Object.defineProperty(componentHandle, propName, {
                get() {
                    return instance.getProperty(prop.name);
                },
                set(value) {
                    instance.setProperty(prop.name, value);
                },
                enumerable: true,
            });
        }
    });

    // Callbacks
    definition.callbacks.forEach((cb) => {
        const callbackName = translateName(cb);

        if (componentHandle[callbackName] !== undefined) {
            console.warn("Duplicated callback name " + callbackName);
        } else {
            Object.defineProperty(componentHandle, callbackName, {
                get() {
                    return function () {
                        return instance.invoke(
                            cb,
                            Array.from(arguments),
                        );
                    };
                },
                set(callback) {
                    instance.setCallback(cb, callback);
                },
                enumerable: true,
            });
        }
    });

    // Functions
    definition.functions.forEach((cb) => {
        const functionName = translateName(cb);

        if (componentHandle[functionName] !== undefined) {
            console.warn("Duplicated function name " + functionName);
        } else {
            Object.defineProperty(componentHandle, functionName, {
                get() {
                    return function () {
                        return instance.invoke(
                            cb,
                            Array.from(arguments),
                        );
                    };
                },
                enumerable: true,
            });
        }
    });

    // Globals
    definition.globals.forEach((globalName) => {
        const jsName = translateName(globalName);
        if (componentHandle[jsName] !== undefined) {
            console.warn(
                "Duplicated property name " +
                    globalName +
                    " (In JS: " +
                    jsName +
                    ")",
            );
        } else {
            const globalObject = Object.create({});

            definition
                .globalProperties(globalName)
                ?.forEach((prop) => {
                    const propName = translateName(prop.name);

                    if (globalObject[propName] !== undefined) {
                        console.warn(
                            "Duplicated property name " +
                                propName +
                                " on global " +
                                globalName,
                        );
                    } else {
                        Object.defineProperty(globalObject, propName, {
                            get() {
                                return instance.getGlobalProperty(
                                    globalName,
                                    prop.name,
                                );
                            },
                            set(value) {
                                instance.setGlobalProperty(
                                    globalName,
                                    prop.name,
                                    value,
                                );
                            },
                            enumerable: true,
                        });
                    }
                });

            definition
                .globalCallbacks(globalName)
                ?.forEach((cb) => {
                    const callbackName = translateName(cb);

                    if (globalObject[callbackName] !== undefined) {
                        console.warn(
                            "Duplicated property name " +
                                cb +
                                " on global " +
                                globalName,
                        );
                    } else {
                        Object.defineProperty(
                            globalObject,
                            callbackName,
                            {
                                get() {
                                    return function () {
                                        return instance.invokeGlobal(
                                            globalName,
                                            cb,
                                            Array.from(arguments),
                                        );
                                    };
                                },
                                set(callback) {
                                    instance.setGlobalCallback(
                                        globalName,
                                        cb,
                                        callback,
                                    );
                                },
                                enumerable: true,
                            },
                        );
                    }
                });

            definition
                .globalFunctions(globalName)
                ?.forEach((cb) => {
                    const functionName = translateName(cb);

                    if (globalObject[functionName] !== undefined) {
                        console.warn(
                            "Duplicated function name " +
                                cb +
                                " on global " +
                                globalName,
                        );
                    } else {
                        Object.defineProperty(
                            globalObject,
                            functionName,
                            {
                                get() {
                                    return function () {
                                        return instance.invokeGlobal(
                                            globalName,
                                            cb,
                                            Array.from(arguments),
                                        );
                                    };
                                },
                                enumerable: true,
                            },
                        );
                    }
                });

            Object.defineProperty(componentHandle, jsName, {
                get() {
                    return globalObject;
                },
                enumerable: true,
            });
        }
    });

    return Object.seal(componentHandle) as Component;
}

/**
 * Takes compiler output (definitions, structs, enums) and builds the
 * sealed module object with constructor functions, struct factories,
 * and enum values.
 *
 * This is the platform-independent core of loadFile/loadSource.
 */
export function wrapModule(
    definitions: Record<string, DefinitionLike>,
    structs: Record<string, unknown>,
    enums: Record<string, unknown>,
): Object {
    const slint_module = Object.create({});

    // Generate structs
    for (const key in structs) {
        Object.defineProperty(slint_module, translateName(key), {
            value: function (properties: Record<string, unknown>) {
                const defaultObject = structs[key] as Record<string, unknown>;
                const newObject = Object.create({});

                for (const propertyKey in defaultObject) {
                    const propertyName = translateName(propertyKey);
                    const propertyValue =
                        properties !== undefined &&
                        Object.hasOwn(properties, propertyName)
                            ? properties[propertyName]
                            : defaultObject[propertyKey];

                    Object.defineProperty(newObject, propertyName, {
                        value: propertyValue,
                        writable: true,
                        enumerable: true,
                    });
                }

                return Object.seal(newObject);
            },
        });
    }

    // Generate enums
    for (const key in enums) {
        Object.defineProperty(slint_module, translateName(key), {
            value: Object.seal(enums[key]),
            enumerable: true,
        });
    }

    // Generate component constructors
    Object.keys(definitions).forEach((key) => {
        const definition = definitions[key];

        Object.defineProperty(slint_module, translateName(definition.name), {
            value: function (properties: Record<string, unknown>) {
                const instance = definition.create();

                if (instance == null) {
                    throw Error(
                        "Could not create a component handle for " + key,
                    );
                }

                for (const propKey in properties) {
                    const value = properties[propKey];

                    if (value instanceof Function) {
                        instance.setCallback(propKey, value);
                    } else {
                        instance.setProperty(propKey, value);
                    }
                }

                return wrapInstance(instance);
            },
        });
    });

    return Object.seal(slint_module);
}
