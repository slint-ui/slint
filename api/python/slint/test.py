import asyncio
import slint
import signal

signal.signal(signal.SIGINT, signal.SIG_IGN)

async def fetch(host: str, path: str = "/"):
    reader, writer = await asyncio.open_connection(host, 80)

    request = f"GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    writer.write(request.encode())
    await writer.drain()

    response = []
    while chunk := await reader.read(1024):
        response.append(chunk)

    writer.close()
    await writer.wait_closed()

    print(b"".join(response).decode(errors="ignore"))

#asyncio.run(fetch("google.com", "/"), loop_factory=slint.Loop, debug=True)
loop = slint.Loop()
asyncio.set_event_loop(loop)
try:
    #loop.run_until_complete(fetch("google.com", "/"))
    loop.run_until_complete(fetch("142.251.143.78", "/"))
finally:
    println("closing")
    loop.close()
