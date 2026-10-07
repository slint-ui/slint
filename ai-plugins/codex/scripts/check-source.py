# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import argparse
import asyncio
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

PLUGIN_ROOT = Path(__file__).resolve().parent.parent
RUNTIME = json.loads((PLUGIN_ROOT / "runtime/runtime.json").read_text())


class LspClient:
    def __init__(self, binary):
        self.binary = binary

    async def send(self, message):
        payload = json.dumps(message).encode()
        self.process.stdin.write(
            f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload
        )
        await self.process.stdin.drain()

    async def receive(self):
        while True:
            header = await self.process.stdout.readuntil(b"\r\n\r\n")
            length = next(
                int(line.split(b":", 1)[1])
                for line in header.split(b"\r\n")
                if line.lower().startswith(b"content-length:")
            )
            message = json.loads(await self.process.stdout.readexactly(length))
            if "method" in message and "id" in message:
                result = (
                    [{} for _ in message.get("params", {}).get("items", [])]
                    if message["method"] == "workspace/configuration"
                    else None
                )
                await self.send(
                    {"jsonrpc": "2.0", "id": message["id"], "result": result}
                )
            else:
                return message

    async def start(self):
        version = subprocess.check_output([self.binary, "--version"], text=True).strip()
        if version != f"slint-lsp {RUNTIME['version']}":
            raise ValueError(
                f"LSP differs from the built preview runtime. Found: {version}"
            )
        self.process = await asyncio.create_subprocess_exec(
            self.binary,
            "-I",
            str(Path(__file__).resolve().parent.parent / "components"),
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await self.send(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": os.getpid(),
                    "rootUri": Path.cwd().as_uri(),
                    "capabilities": {
                        "textDocument": {
                            "publishDiagnostics": {"versionSupport": True}
                        },
                        "workspace": {"configuration": True},
                    },
                },
            }
        )
        while (await asyncio.wait_for(self.receive(), 15)).get("id") != 1:
            pass
        await self.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})

    async def check(self, path, source, revision):
        uri = Path(path).resolve().as_uri()
        if type(revision) is not int or revision < 1:
            raise ValueError("Source revisions must be positive integers")
        await self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "slint",
                        "version": revision,
                        "text": source,
                    }
                },
            }
        )
        # Slint processes didOpen and publishes its complete diagnostic batch before
        # handling the next request; its per-file notification order is unspecified.
        await self.send(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/documentSymbol",
                "params": {"textDocument": {"uri": uri}},
            }
        )
        published_by_uri = {}
        entry_seen = False
        while True:
            message = await asyncio.wait_for(self.receive(), 15)
            if message.get("id") == 2:
                if "error" in message or not entry_seen:
                    raise ValueError("The LSP did not complete source diagnostics")
                break
            if message.get("method") != "textDocument/publishDiagnostics":
                continue
            published = message["params"]
            published_by_uri[published["uri"]] = [
                {**item, "uri": published["uri"]} for item in published["diagnostics"]
            ]
            entry_seen |= (
                published["uri"] == uri and published.get("version") == revision
            )
        diagnostics = [item for items in published_by_uri.values() for item in items]
        return {
            "runtimeVersion": RUNTIME["version"],
            "runtimeRevision": RUNTIME["revision"],
            "revision": revision,
            "sourceHash": hashlib.sha256(source.encode()).hexdigest(),
            "status": "error"
            if any(item.get("severity") == 1 for item in diagnostics)
            else "valid",
            "diagnostics": diagnostics,
        }

    async def close(self):
        if (
            getattr(self, "process", None) is not None
            and self.process.returncode is None
        ):
            self.process.terminate()
            await self.process.wait()


async def main(args):
    binary = str(
        PLUGIN_ROOT / "runtime" / ("slint-lsp.exe" if os.name == "nt" else "slint-lsp")
    )
    client = LspClient(binary)
    try:
        await client.start()
        result = await client.check(
            args.path, Path(args.path).read_bytes().decode("utf-8"), args.revision
        )
        print(json.dumps(result), flush=True)
        return int(result["status"] == "error")
    finally:
        await client.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Return revision-specific diagnostics from Slint's LSP"
    )
    parser.add_argument("path")
    parser.add_argument("--revision", type=int, default=1)
    try:
        sys.exit(asyncio.run(main(parser.parse_args())))
    except Exception as error:
        print(json.dumps({"status": "failure", "message": str(error)}), file=sys.stderr)
        sys.exit(2)
