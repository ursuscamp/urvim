"""Small MessagePack stdio demo plugin for urvim."""

from __future__ import annotations

import struct
import sys
from typing import Any

import msgpack


MAX_FRAME_LEN = 16 * 1024 * 1024


def read_exact(size: int) -> bytes | None:
    data = sys.stdin.buffer.read(size)
    if not data:
        return None
    if len(data) != size:
        raise EOFError(f"expected {size} bytes, got {len(data)}")
    return data


def read_message() -> dict[str, Any] | None:
    header = read_exact(4)
    if header is None:
        return None
    (length,) = struct.unpack(">I", header)
    if length > MAX_FRAME_LEN:
        raise ValueError(f"frame too large: {length}")
    payload = read_exact(length)
    if payload is None:
        raise EOFError("missing frame payload")
    return msgpack.unpackb(payload, raw=False)


def write_message(message: dict[str, Any]) -> None:
    payload = msgpack.packb(message, use_bin_type=True)
    sys.stdout.buffer.write(struct.pack(">I", len(payload)))
    sys.stdout.buffer.write(payload)
    sys.stdout.buffer.flush()


def response_for_request(request: dict[str, Any], result: Any) -> dict[str, Any]:
    return {
        "type": "response",
        "id": request.get("id"),
        "result": result,
    }


def request_editor(request_id: int, method: str, params: dict[str, Any] | None = None) -> Any:
    write_message(
        {
            "type": "request",
            "id": request_id,
            "method": method,
            "params": params or {},
        }
    )
    while True:
        message = read_message()
        if message is None:
            raise EOFError("editor closed while waiting for response")
        if message.get("type") == "response" and message.get("id") == request_id:
            if message.get("error"):
                raise RuntimeError(str(message["error"]))
            return message.get("result")


def handle_request(request: dict[str, Any]) -> dict[str, Any]:
    method = request.get("method")
    if method == "editor/initialize":
        result: dict[str, Any] = {
            "protocol_version": 1,
            "name": "demo-plugin",
            "capabilities": ["demo/echo"],
        }
    elif method == "demo/echo":
        params = request.get("params", {})
        text = params.get("text", "") if isinstance(params, dict) else ""
        active_buffer = request_editor(1000 + int(request.get("id", 0)), "editor/getActiveBuffer")
        edit_result = None
        if isinstance(params, dict) and params.get("insert"):
            edit_result = request_editor(
                2000 + int(request.get("id", 0)),
                "editor/applyEdit",
                {
                    "buffer_id": active_buffer["id"],
                    "kind": "insert",
                    "start": active_buffer["cursor"],
                    "text": str(params["insert"]),
                },
            )
        result = {
            "text": text,
            "params": params,
            "active_buffer": active_buffer,
            "edit": edit_result,
        }
    else:
        result = {
            "echo": request.get("params", {}),
            "method": method,
        }
    return response_for_request(request, result)


def main() -> int:
    while True:
        message = read_message()
        if message is None:
            return 0
        if message.get("type") == "request":
            write_message(handle_request(message))
            if message.get("method") == "editor/initialize":
                write_message(
                    {
                        "type": "notification",
                        "method": "editor/notify",
                        "params": {
                            "level": "info",
                            "message": "demo plugin process initialized",
                        },
                    }
                )
        elif message.get("type") == "notification":
            print(f"notification: {message.get('method')}", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
