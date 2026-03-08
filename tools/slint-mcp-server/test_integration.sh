# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/bash
# Integration test: starts MCP server, connects gallery app, sends MCP requests
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SLINT_ROOT="$SCRIPT_DIR/../.."
MCP_SERVER="$SCRIPT_DIR/target/release/slint-mcp-server"
GALLERY="$SLINT_ROOT/target/release/gallery"
PORT=14242

echo "=== Starting MCP server on port $PORT ==="

# Create a named pipe for communication
FIFO=$(mktemp -u)
mkfifo "$FIFO"

# Start MCP server with stdin from fifo, capture stdout
$MCP_SERVER --port $PORT < "$FIFO" > /tmp/mcp_responses.txt 2>/tmp/mcp_stderr.txt &
MCP_PID=$!

# Open the fifo for writing (keep it open)
exec 3>"$FIFO"

# Give server a moment to start listening
sleep 1

echo "=== Starting gallery app ==="
SLINT_TEST_SERVER=127.0.0.1:$PORT $GALLERY &
GALLERY_PID=$!

# Wait for connection
sleep 3

echo "=== Sending MCP initialize ==="
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' >&3
sleep 0.5

echo "=== Sending tools/list ==="
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' >&3
sleep 0.5

echo "=== Sending list_windows ==="
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_windows","arguments":{}}}' >&3
sleep 0.5

echo "=== Sending get_window_properties ==="
echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_window_properties","arguments":{"window_handle":{"index":0,"generation":0}}}}' >&3
sleep 0.5

echo "=== Sending take_screenshot ==="
echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"take_screenshot","arguments":{"window_handle":{"index":0,"generation":0}}}}' >&3
sleep 1

echo "=== Sending get_element_tree (depth 2) ==="
# First we need the root element handle from the window properties response
echo '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"get_element_tree","arguments":{"element_handle":{"index":0,"generation":0},"max_depth":2}}}' >&3
sleep 2

echo ""
echo "=== MCP Server stderr ==="
cat /tmp/mcp_stderr.txt

echo ""
echo "=== MCP Responses ==="
# Pretty print each JSON response
while IFS= read -r line; do
    echo "$line" | python3 -m json.tool 2>/dev/null || echo "$line"
done < /tmp/mcp_responses.txt

# Cleanup
exec 3>&-
kill $GALLERY_PID 2>/dev/null || true
kill $MCP_PID 2>/dev/null || true
rm -f "$FIFO" /tmp/mcp_responses.txt /tmp/mcp_stderr.txt
wait 2>/dev/null

echo ""
echo "=== Done ==="
