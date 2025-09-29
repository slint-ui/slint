# Using the Slint Docs MCP Server

## Quick Start

1. **Install dependencies:**
   ```bash
   cd slint-docs-mcp
   pnpm install
   ```

2. **Start the MCP server:**
   ```bash
   pnpm start
   ```


3. **Configure Cursor:**
   Add the MCP server to your Cursor configuration by adding the details below to the MCP section.

   **Important:** Update the `<absolute path to slint repo>` in the configuration to point to your Slint directory:
   ```json
   {
     "mcpServers": {
       "slint-docs": {
          "command": "node",
          "args": ["<absolute path to slint repo>/slint-docs-mcp/src/index.js"],
          "env": {
            "SLINT_DOCS_PATH": "<absolute path to slint repo>/docs/astro/src/content/docs"
          }
        }
     }
   }
   ```

## Available Tools

Once configured, you can use these tools in Cursor:

### 🔍 Search Slint Documentation
- **Tool:** `search_slint_docs`
- **Use case:** Find relevant documentation pages
- **Example queries:**
  - "how to create buttons in Slint"
  - "property binding examples"
  - "animation tutorials"
  - "grid layout components"

### 📖 Get Full Documentation Content
- **Tool:** `get_slint_doc_content`
- **Use case:** Get complete content of a specific page
- **Example paths:**
  - `guide/language/concepts/slint-language`
  - `reference/std-widgets/basic-widgets/button`
  - `tutorial/memory_tile`

### 📚 List All Categories
- **Tool:** `list_slint_doc_categories`
- **Use case:** Browse the documentation structure

## Example Usage in Cursor

Once the MCP server is configured, you can ask questions like:

- "How do I create a button in Slint?"
- "Show me examples of property binding"
- "What are the available layout components?"
- "How do animations work in Slint?"
- "Find documentation about std-widgets"

The MCP server will search through all Slint documentation and provide relevant, accurate answers with links to the source documentation.

## Categories

The documentation is organized into these categories:

- **Guide**: Language concepts, coding patterns, development practices
- **Reference**: Complete API reference for all Slint components
- **Tutorial**: Step-by-step tutorials and examples
- **Language Integrations**: Language-specific documentation

## Troubleshooting

1. **Server not starting:** Check that the `SLINT_DOCS_PATH` environment variable points to the correct documentation directory
2. **No results found:** Try broader search terms or different categories
3. **Configuration issues:** Ensure the path in `cursor-mcp-config.json` is correct for your system

## Development

To modify or extend the MCP server:

1. Run in dev mode with `pnpm dev`
2. Edit `src/index.js` for core functionality
3. Update `package.json` for dependencies
4. Test with `node test-mcp.js`
