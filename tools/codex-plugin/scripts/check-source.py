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
        self.documents = {}

    async def send(self, message):
        payload = json.dumps(message).encode()
        self.process.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
        await self.process.stdin.drain()

    async def receive(self):
        while True:
            header = await self.process.stdout.readuntil(b"\r\n\r\n")
            length = next(int(line.split(b":", 1)[1]) for line in header.split(b"\r\n") if line.lower().startswith(b"content-length:"))
            message = json.loads(await self.process.stdout.readexactly(length))
            if "method" in message and "id" in message:
                result = [{} for _ in message.get("params", {}).get("items", [])] if message["method"] == "workspace/configuration" else None
                await self.send({"jsonrpc": "2.0", "id": message["id"], "result": result})
            else:
                return message

    async def start(self):
        version = subprocess.check_output([self.binary, "--version"], text=True).strip()
        if version != f"slint-lsp {RUNTIME['version']}":
            raise ValueError(f"LSP differs from the built preview runtime. Found: {version}")
        self.process = await asyncio.create_subprocess_exec(self.binary, "-I", str(Path(__file__).resolve().parent.parent / "components"), stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        await self.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"processId": os.getpid(), "rootUri": Path.cwd().as_uri(), "capabilities": {"textDocument": {"publishDiagnostics": {"versionSupport": True}}, "workspace": {"configuration": True}}}})
        while (await asyncio.wait_for(self.receive(), 15)).get("id") != 1:
            pass
        await self.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})

    async def check(self, path, source, revision):
        uri = Path(path).resolve().as_uri()
        if type(revision) is not int or revision <= self.documents.get(uri, 0):
            raise ValueError("Source revisions must increase for each document")
        if uri not in self.documents:
            method = "textDocument/didOpen"
            params = {"textDocument": {"uri": uri, "languageId": "slint", "version": revision, "text": source}}
        else:
            method = "textDocument/didChange"
            params = {"textDocument": {"uri": uri, "version": revision}, "contentChanges": [{"text": source}]}
        self.documents[uri] = revision
        await self.send({"jsonrpc": "2.0", "method": method, "params": params})
        diagnostics = []
        while True:
            message = await asyncio.wait_for(self.receive(), 15)
            if message.get("method") != "textDocument/publishDiagnostics":
                continue
            published = message["params"]
            diagnostics.extend({**item, "uri": published["uri"]} for item in published["diagnostics"])
            if published["uri"] == uri and published.get("version") == revision:
                break
        return {"runtimeVersion": RUNTIME["version"], "runtimeRevision": RUNTIME["revision"], "revision": revision, "sourceHash": hashlib.sha256(source.encode()).hexdigest(), "status": "error" if any(item.get("severity") == 1 for item in diagnostics) else "valid", "diagnostics": diagnostics}

    async def close(self):
        if getattr(self, "process", None) is not None and self.process.returncode is None:
            self.process.terminate()
            await self.process.wait()


async def main(args):
    binary = str(PLUGIN_ROOT / "runtime" / ("slint-lsp.exe" if os.name == "nt" else "slint-lsp"))
    client = LspClient(binary)
    try:
        await client.start()
        if args.stdio:
            while line := await asyncio.to_thread(sys.stdin.readline):
                request = json.loads(line)
                result = await client.check(request["path"], request["source"], request["revision"])
                print(json.dumps(result), flush=True)
        else:
            if not args.path:
                raise ValueError("Provide a .slint file or --stdio")
            result = await client.check(args.path, Path(args.path).read_text(), args.revision)
            print(json.dumps(result), flush=True)
            return int(result["status"] == "error")
        return 0
    finally:
        await client.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Return revision-specific diagnostics from Slint's LSP")
    parser.add_argument("path", nargs="?")
    parser.add_argument("--revision", type=int, default=1)
    parser.add_argument("--stdio", action="store_true", help="Keep the LSP alive; read source revisions as JSON lines")
    try:
        sys.exit(asyncio.run(main(parser.parse_args())))
    except Exception as error:
        print(json.dumps({"status": "failure", "message": str(error)}), file=sys.stderr)
        sys.exit(2)
