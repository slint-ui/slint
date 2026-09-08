// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { VariantAxis } from "./component-variants";
import { optionValue } from "./component-variants";

/** Explicit host configuration; no behavior is inferred from Figma layer names. */
export type ButtonBehavior = {
    kind: "button";
    stateAxis: string;
    states: {
        enabled: string;
        pressed: string;
        disabled: string;
        hovered?: string;
        focused?: string;
    };
    /** Authored Figma text-property key used for the accessible label. */
    labelProperty: string;
};
export type ComponentGenerationOptions = {
    behaviors?: Record<string, ButtonBehavior>;
};
export function buttonBehavior(
    profile: ButtonBehavior | undefined,
    axes: VariantAxis[],
    properties: Map<string, { name: string; type: string }>,
) {
    if (!profile) return undefined;
    if (profile.kind !== "button")
        throw Error("Unsupported component behavior mapping");
    const axis = axes.find((a) => a.key === profile.stateAxis);
    const label = properties.get(profile.labelProperty);
    if (
        !axis ||
        !label ||
        label.type !== "string" ||
        Object.values(profile.states).some((v) => !axis.options.has(v))
    )
        throw Error(
            "Button behavior references an unavailable state or label property",
        );
    if (
        [...properties.values()].some((p) =>
            ["enabled", "keyboard-pressed", "clicked"].includes(p.name),
        )
    )
        throw Error("Button behavior conflicts with an authored property");
    const state = (value: string) => optionValue(axis, value);
    const idle = profile.states.focused
        ? `interaction-focus.has-focus ? ${state(profile.states.focused)} : ${state(profile.states.enabled)}`
        : state(profile.states.enabled);
    const hover = profile.states.hovered
        ? `interaction-touch.has-hover ? ${state(profile.states.hovered)} : (${idle})`
        : idle;
    const expression = `!root.enabled ? ${state(profile.states.disabled)} : ((interaction-touch.pressed || root.keyboard-pressed) ? ${state(profile.states.pressed)} : (${hover}))`;
    const declarations = [
        "    in property <bool> enabled: true;",
        "    callback clicked();",
        "    private property <bool> keyboard-pressed: false;",
        "    forward-focus: interaction-focus;",
        "    accessible-role: button;",
        "    accessible-enabled: root.enabled;",
        `    accessible-label: root.${label.name};`,
        "    accessible-action-default => { if root.enabled { root.clicked(); } }",
    ];
    const body = [
        "    interaction-focus := FocusScope {",
        "        enabled: root.enabled;",
        "        focus-lost => { root.keyboard-pressed = false; }",
        "        key-pressed(event) => {",
        "            if root.enabled && (event.text == Key.Space || event.text == Key.Return) {",
        "                root.keyboard-pressed = true; accept",
        "            } else { reject }",
        "        }",
        "        key-released(event) => {",
        "            if event.text == Key.Space || event.text == Key.Return {",
        "                if root.enabled && root.keyboard-pressed { root.clicked(); }",
        "                root.keyboard-pressed = false; accept",
        "            } else { reject }",
        "        }",
        "        interaction-touch := TouchArea {",
        "            enabled: root.enabled;",
        "            clicked => { if root.enabled { root.clicked(); } }",
        "        }",
        "    }",
    ];
    return { axis: axis.key, expression, declarations, body };
}
