// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { restProperty, REST_UNAVAILABLE } from "./rest-properties";
import {
    encodeValue,
    type SourceBytes,
    type SourceCapture,
    type SourceNode,
} from "./source";

export function indexBulkDocument(
    document: Record<string, unknown>,
    nodes: Map<string, Record<string, unknown>>,
): void {
    const collect = (node: Record<string, unknown>) => {
        if (typeof node.id !== "string" || typeof node.type !== "string")
            throw Error("Invalid bulk node identity");
        nodes.set(node.id, node);
        if (node.children !== undefined) {
            if (!Array.isArray(node.children))
                throw Error("Invalid bulk children");
            for (const child of node.children) {
                if (!child || typeof child !== "object" || Array.isArray(child))
                    throw Error("Invalid bulk child");
                collect(child);
            }
        }
    };
    collect(document);
}

export function hydrateBulkSource<Bytes extends SourceBytes>(
    source: SourceCapture<Bytes>,
    nodes = new Map<string, Record<string, unknown>>(),
    documents = new Set<string>(),
): SourceCapture<Bytes> {
    for (const document of source.bulkDocuments ?? []) {
        const id = String(document.id);
        if (!documents.has(id)) {
            indexBulkDocument(document, nodes);
            documents.add(id);
        }
    }
    const hydrate = (node: SourceNode<Bytes>): SourceNode<Bytes> => {
        const properties = { ...node.properties };
        const bulk = nodes.get(node.id);
        const keys =
            node.bulkPropertySet === undefined
                ? (node.bulkProperties ?? [])
                : source.bulkPropertySets?.[node.bulkPropertySet];
        if (
            !keys ||
            !Array.isArray(keys) ||
            keys.some((key) => typeof key !== "string")
        )
            throw Error("Invalid bulk property set");
        for (const key of keys) {
            if (
                !bulk ||
                !(
                    bulk.type === node.type ||
                    (node.type === "INSTANCE" && bulk.type === "FRAME")
                )
            )
                throw Error(`Missing bulk metadata for ${node.id}`);
            const value = restProperty(bulk, key);
            if (value === REST_UNAVAILABLE)
                throw Error(`Missing bulk property ${node.id}.${key}`);
            if (!(key in properties)) properties[key] = encodeValue(value);
        }
        const { bulkProperties: _keys, bulkPropertySet: _set, ...rest } = node;
        return {
            ...rest,
            properties,
            ...(node.children ? { children: node.children.map(hydrate) } : {}),
        };
    };
    const {
        bulkDocuments: _documents,
        bulkPropertySets: _sets,
        ...rest
    } = source;
    return {
        ...rest,
        root: hydrate(source.root),
        ...(source.components
            ? {
                  components: {
                      ...source.components,
                      definitions: source.components.definitions.map(
                          (definition) => ({
                              ...definition,
                              variants: definition.variants.map((variant) => ({
                                  ...variant,
                                  root: hydrate(variant.root),
                              })),
                          }),
                      ),
                  },
              }
            : {}),
    };
}
