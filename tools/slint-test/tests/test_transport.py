# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import socket
import struct
import threading
import time
from types import SimpleNamespace

import pytest
import slint_testing as low

from slint_test import Session
from slint_test.core import BoundApplication, OperationTimeout


def test_timed_out_partial_response_is_drained_before_failure_capture(monkeypatch):
    client, server = socket.socketpair()
    raw = low.Application.__new__(low.Application)
    raw.aut_connection = client
    monkeypatch.setattr(
        raw, "process", SimpleNamespace(poll=lambda: None), raising=False
    )
    session = Session()
    bound = BoundApplication(raw, session)
    errors = []

    def respond():
        try:
            for index in range(2):
                header = server.recv(4, socket.MSG_WAITALL)
                size = struct.unpack(">I", header)[0]
                server.recv(size, socket.MSG_WAITALL)
                response = low.AUTResponse()
                response.window_list.window_handles.add(index=index + 1, generation=1)
                payload = response.SerializeToString()
                framed = struct.pack(">I", len(payload)) + payload
                server.sendall(framed[:2])
                if index == 0:
                    time.sleep(0.03)
                server.sendall(framed[2:])
        except Exception as error:  # noqa: BLE001
            errors.append(error)
        finally:
            server.close()

    worker = threading.Thread(target=respond)
    worker.start()
    try:
        with pytest.raises(OperationTimeout), session.operation(10):
            bound._send_request(low.RequestToAUT())
        # Reporting uses the original client, so it must see the next response.
        window = raw.first_window
        assert window is not None
        assert window.handle.index == 2
    finally:
        client.close()
        worker.join(timeout=2)
    assert not worker.is_alive()
    assert not errors
