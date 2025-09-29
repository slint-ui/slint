// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
    CallToolRequestSchema,
    ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, extname, relative, resolve } from "node:path";
import matter from "gray-matter";
import MarkdownIt from "markdown-it";
import Fuse from "fuse.js";

// Initialize markdown parser
const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true,
});

// Configuration - can be overridden with environment variables
const DOCS_PATH =
    process.env.SLINT_DOCS_PATH || "../docs/astro/src/content/docs";
const BASE_URL = process.env.SLINT_DOCS_BASE_URL || "https://slint.dev/docs";

// Cache for documentation content
let docsCache = null;
let fuseIndex = null;

class SlintDocsServer {
    constructor() {
        this.server = new Server(
            {
                name: "slint-mcp",
                version: "1.0.0",
            },
            {
                capabilities: {
                    tools: {},
                },
            },
        );

        this.setupToolHandlers();
    }

    setupToolHandlers() {
        this.server.setRequestHandler(ListToolsRequestSchema, () => {
            return {
                tools: [
                    {
                        name: "search_slint_docs",
                        description:
                            "Search through Slint documentation for information about the Slint language, API reference, tutorials, and guides",
                        inputSchema: {
                            type: "object",
                            properties: {
                                query: {
                                    type: "string",
                                    description:
                                        "The search query to find relevant documentation",
                                },
                                category: {
                                    type: "string",
                                    description:
                                        "Optional category to filter results (guide, reference, tutorial)",
                                    enum: [
                                        "guide",
                                        "reference",
                                        "tutorial",
                                        "all",
                                    ],
                                    default: "all",
                                },
                                limit: {
                                    type: "number",
                                    description:
                                        "Maximum number of results to return",
                                    default: 10,
                                },
                            },
                            required: ["query"],
                        },
                    },
                    {
                        name: "get_slint_doc_content",
                        description:
                            "Get the full content of a specific Slint documentation page",
                        inputSchema: {
                            type: "object",
                            properties: {
                                path: {
                                    type: "string",
                                    description:
                                        'The relative path to the documentation file (e.g., "guide/language/concepts/slint-language")',
                                },
                            },
                            required: ["path"],
                        },
                    },
                    {
                        name: "list_slint_doc_categories",
                        description:
                            "List all available documentation categories and their structure",
                        inputSchema: {
                            type: "object",
                            properties: {},
                        },
                    },
                ],
            };
        });

        this.server.setRequestHandler(
            CallToolRequestSchema,
            async (request) => {
                const { name, arguments: args } = request.params;

                try {
                    switch (name) {
                        case "search_slint_docs":
                            return await this.searchDocs(
                                args.query,
                                args.category || "all",
                                args.limit || 10,
                            );
                        case "get_slint_doc_content":
                            return await this.getDocContent(args.path);
                        case "list_slint_doc_categories":
                            return await this.listCategories();
                        default:
                            throw new Error(`Unknown tool: ${name}`);
                    }
                } catch (error) {
                    return {
                        content: [
                            {
                                type: "text",
                                text: `Error: ${error.message}`,
                            },
                        ],
                    };
                }
            },
        );
    }

    loadDocs() {
        if (docsCache && fuseIndex) {
            return { docsCache, fuseIndex };
        }

        console.log("Loading Slint documentation...");
        const docs = [];

        // Resolve the docs path relative to the current working directory
        const resolvedDocsPath = resolve(DOCS_PATH);

        const loadDirectory = (dir, category = "") => {
            const items = readdirSync(dir);

            for (const item of items) {
                const fullPath = join(dir, item);
                const stat = statSync(fullPath);

                if (stat.isDirectory()) {
                    loadDirectory(
                        fullPath,
                        category ? `${category}/${item}` : item,
                    );
                } else if (
                    extname(item) === ".mdx" ||
                    extname(item) === ".md"
                ) {
                    try {
                        const content = readFileSync(fullPath, "utf8");
                        const { data: frontmatter, content: body } =
                            matter(content);

                        const relativePath = relative(
                            resolvedDocsPath,
                            fullPath,
                        );
                        const pathWithoutExt = relativePath.replace(
                            /\.(mdx|md)$/,
                            "",
                        );

                        // Convert markdown to plain text for searching
                        const html = md.render(body);
                        const textContent = this.stripHtml(html);

                        const doc = {
                            title:
                                frontmatter.title ||
                                this.extractTitleFromContent(body),
                            description:
                                frontmatter.description ||
                                this.extractDescription(textContent),
                            category: this.determineCategory(pathWithoutExt),
                            path: pathWithoutExt,
                            fullPath,
                            content: body,
                            textContent,
                            frontmatter,
                            url: `${BASE_URL}/${pathWithoutExt}`,
                        };

                        docs.push(doc);
                    } catch (error) {
                        console.warn(
                            `Failed to load ${fullPath}:`,
                            error.message,
                        );
                    }
                }
            }
        };

        loadDirectory(resolvedDocsPath);

        // Create Fuse.js index for fuzzy search
        const fuseOptions = {
            keys: [
                { name: "title", weight: 0.3 },
                { name: "description", weight: 0.2 },
                { name: "textContent", weight: 0.3 },
                { name: "category", weight: 0.1 },
                { name: "path", weight: 0.1 },
            ],
            threshold: 0.4,
            includeScore: true,
            includeMatches: true,
        };

        fuseIndex = new Fuse(docs, fuseOptions);
        docsCache = docs;

        console.log(`Loaded ${docs.length} documentation pages`);
        return { docsCache, fuseIndex };
    }

    determineCategory(path) {
        if (path.startsWith("guide/")) {
            return "guide";
        }
        if (path.startsWith("reference/")) {
            return "reference";
        }
        if (path.startsWith("tutorial/")) {
            return "tutorial";
        }
        return "other";
    }

    extractTitleFromContent(content) {
        const lines = content.split("\n");
        for (const line of lines) {
            const trimmed = line.trim();
            if (trimmed.startsWith("# ")) {
                return trimmed.substring(2).trim();
            }
        }
        return "Untitled";
    }

    extractDescription(content) {
        // Extract first meaningful paragraph
        const sentences = content
            .split(/[.!?]+/)
            .filter((s) => s.trim().length > 20);
        return (
            sentences[0]?.trim().substring(0, 200) + "..." ||
            "No description available"
        );
    }

    stripHtml(html) {
        return html
            .replace(/<script[^>]*>.*?<\/script>/gi, "")
            .replace(/<style[^>]*>.*?<\/style>/gi, "")
            .replace(/<[^>]*>/g, " ")
            .replace(/\s+/g, " ")
            .trim();
    }

    async searchDocs(query, category, limit) {
        const { docsCache, fuseIndex } = await this.loadDocs();

        let results = fuseIndex.search(query);

        // Filter by category if specified
        if (category !== "all") {
            results = results.filter(
                (result) => result.item.category === category,
            );
        }

        // Limit results
        results = results.slice(0, limit);

        if (results.length === 0) {
            return {
                content: [
                    {
                        type: "text",
                        text: `No documentation found for query: "${query}"${category !== "all" ? ` in category: ${category}` : ""}`,
                    },
                ],
            };
        }

        const formattedResults = results
            .map((result, index) => {
                const doc = result.item;
                const score = result.score
                    ? (1 - result.score).toFixed(2)
                    : "1.00";

                return `${index + 1}. **${doc.title}** (Score: ${score})
   - Category: ${doc.category}
   - Path: \`${doc.path}\`
   - URL: ${doc.url}
   - Description: ${doc.description}
   - Matches: ${result.matches?.map((m) => `"${m.value}"`).join(", ") || "N/A"}`;
            })
            .join("\n\n");

        return {
            content: [
                {
                    type: "text",
                    text: `Found ${results.length} documentation page(s) for "${query}":\n\n${formattedResults}`,
                },
            ],
        };
    }

    async getDocContent(path) {
        const { docsCache } = await this.loadDocs();

        const doc = docsCache.find((d) => d.path === path);
        if (!doc) {
            return {
                content: [
                    {
                        type: "text",
                        text: `Documentation page not found: ${path}`,
                    },
                ],
            };
        }

        return {
            content: [
                {
                    type: "text",
                    text: `# ${doc.title}

**Category:** ${doc.category}
**URL:** ${doc.url}

## Content

${doc.content}`,
                },
            ],
        };
    }

    async listCategories() {
        const { docsCache } = await this.loadDocs();

        const categories = {};
        docsCache.forEach((doc) => {
            if (!categories[doc.category]) {
                categories[doc.category] = [];
            }
            categories[doc.category].push({
                title: doc.title,
                path: doc.path,
                url: doc.url,
            });
        });

        const formattedCategories = Object.entries(categories)
            .map(([category, docs]) => {
                const docList = docs
                    .map(
                        (doc) =>
                            `  - [${doc.title}](${doc.url}) (\`${doc.path}\`)`,
                    )
                    .join("\n");
                return `## ${category.charAt(0).toUpperCase() + category.slice(1)}\n${docList}`;
            })
            .join("\n\n");

        return {
            content: [
                {
                    type: "text",
                    text: `# Slint Documentation Categories\n\n${formattedCategories}\n\nTotal pages: ${docsCache.length}`,
                },
            ],
        };
    }

    async run() {
        const transport = new StdioServerTransport();
        await this.server.connect(transport);
        console.error("Slint Docs MCP server running on stdio");
    }
}

const server = new SlintDocsServer();
server.run().catch(console.error);
