<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint Documentation MCP Server

This is a Model Context Protocol (MCP) server that provides search and retrieval capabilities for the Slint documentation. It allows AI assistants and other tools to search through Slint's comprehensive documentation to answer questions about the Slint language, API reference, tutorials, and guides.

## Features

- **Fuzzy Search**: Uses Fuse.js for intelligent fuzzy searching through all documentation
- **Category Filtering**: Filter results by documentation category (guide, reference, tutorial)
- **Content Retrieval**: Get full content of specific documentation pages
- **Structured Data**: Extracts metadata like titles, descriptions, and categories from markdown files
- **Fast Indexing**: Caches documentation content for quick access

## Available Tools

### 1. `search_slint_docs`
Search through Slint documentation for relevant information.

**Parameters:**
- `query` (required): The search query
- `category` (optional): Filter by category (`guide`, `reference`, `tutorial`, or `all`)
- `limit` (optional): Maximum number of results (default: 10)

**Example:**
```json
{
  "query": "how to create a button",
  "category": "guide",
  "limit": 5
}
```

### 2. `get_slint_doc_content`
Get the full content of a specific documentation page.

**Parameters:**
- `path` (required): The relative path to the documentation file

**Example:**
```json
{
  "path": "guide/language/concepts/slint-language"
}
```

### 3. `list_slint_doc_categories`
List all available documentation categories and their structure.

**Parameters:** None

## Installation

1. Install dependencies:
```bash
npm install
```

2. Set the `SLINT_DOCS_PATH` environment variable to point to your Slint documentation directory:
```bash
export SLINT_DOCS_PATH="/path/to/slint/docs/astro/src/content/docs"
```

Or configure it in your MCP client configuration (see Configuration section below).

## Usage

### As an MCP Server

Run the server:
```bash
npm start
```

### Configuration

Add to your MCP client configuration (e.g., `.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "slint-docs": {
      "command": "node",
      "args": ["slint-docs-mcp/src/index.js"],
      "env": {
        "SLINT_DOCS_PATH": "docs/astro/src/content/docs"
      }
    }
  }
}
```

**Environment Variables:**
- `SLINT_DOCS_PATH`: Path to the Slint documentation directory (default: `../docs/astro/src/content/docs`)
- `SLINT_DOCS_BASE_URL`: Base URL for documentation links (default: `https://slint.dev/docs`)

## Documentation Structure

The server indexes the following documentation categories:

- **Guide**: Language concepts, coding patterns, app development, platforms, backends
- **Reference**: API reference for elements, properties, functions, std-widgets
- **Tutorial**: Step-by-step tutorials including the memory game tutorial
- **Language Integrations**: Links to language-specific documentation

## Search Capabilities

The search is powered by Fuse.js and includes:

- **Title matching**: Prioritizes matches in document titles
- **Content matching**: Searches through full document content
- **Category filtering**: Allows filtering by documentation section
- **Fuzzy matching**: Handles typos and partial matches
- **Score-based ranking**: Results are ranked by relevance

## Example Queries

- "how to create buttons"
- "property binding"
- "animations"
- "grid layout"
- "memory game tutorial"
- "std widgets"
- "reactivity"
- "desktop platform"

## Development

For development with auto-reload:
```bash
npm run dev
```

## License

MIT License - See LICENSE file for details.
