"""urvim plugin that demonstrates async LSP-backed editor APIs."""

from __future__ import annotations

import struct
import sys
from typing import Any

import msgpack

MAX_FRAME_LEN = 16 * 1024 * 1024
NEXT_REQUEST_ID = 100


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


def next_request_id() -> int:
    global NEXT_REQUEST_ID
    request_id = NEXT_REQUEST_ID
    NEXT_REQUEST_ID += 1
    return request_id


def response_for_request(request: dict[str, Any], result: Any) -> dict[str, Any]:
    return {
        "type": "response",
        "id": request.get("id"),
        "result": result,
    }


def error_response_for_request(request: dict[str, Any], error: str) -> dict[str, Any]:
    return {
        "type": "response",
        "id": request.get("id"),
        "error": error,
    }


def notify_editor(level: str, message: str) -> None:
    write_message(
        {
            "type": "notification",
            "method": "editor/notify",
            "params": {
                "level": level,
                "message": message,
            },
        }
    )


def request_editor(method: str, params: dict[str, Any] | None = None) -> Any:
    request_id = next_request_id()
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
            raise EOFError("editor closed plugin transport")
        if message.get("type") == "response" and message.get("id") == request_id:
            if error := message.get("error"):
                raise RuntimeError(str(error))
            return message.get("result")
        if message.get("type") == "request":
            write_message(handle_request(message))
        elif message.get("type") == "notification":
            # Symbol Lens has no event hooks, but keep the loop protocol-friendly.
            continue


def send_register_command(request_id: int, name: str, method: str, description: str) -> None:
    write_message(
        {
            "type": "request",
            "id": request_id,
            "method": "editor/registerCommand",
            "params": {
                "name": name,
                "request": method,
                "description": description,
            },
        }
    )


def register_startup_contributions() -> None:
    send_register_command(
        2,
        "hover_lens",
        "symbolLens/hoverLens",
        "Show LSP hover at the active cursor",
    )
    send_register_command(
        3,
        "definition_preview",
        "symbolLens/definitionPreview",
        "Show the LSP definition target at the active cursor",
    )
    send_register_command(
        4,
        "completion_lens",
        "symbolLens/completionLens",
        "Show LSP completion candidates at the active cursor",
    )


def active_buffer_and_cursor() -> tuple[int, dict[str, int]]:
    active = request_editor("editor/getActiveBuffer")
    buffer_id = active.get("buffer_id")
    if buffer_id is None:
        raise RuntimeError("no active buffer")
    cursor_result = request_editor("editor/getCursor", {"buffer_id": buffer_id})
    cursor = cursor_result.get("cursor") or cursor_result
    return int(buffer_id), cursor


def handle_hover_lens() -> dict[str, str]:
    buffer_id, cursor = active_buffer_and_cursor()
    try:
        hover = request_editor(
            "editor/requestHover",
            {"buffer_id": buffer_id, "line": cursor["line"], "col": cursor["col"]},
        )
        contents = hover.get("contents") if isinstance(hover, dict) else None
        message = contents.strip() if isinstance(contents, str) and contents.strip() else "no hover available"
    except RuntimeError as error:
        message = f"hover lens unavailable: {error}"
    notify_editor("info", message)
    return {"message": message}


def handle_definition_preview() -> dict[str, str]:
    buffer_id, cursor = active_buffer_and_cursor()
    try:
        definition = request_editor(
            "editor/requestDefinition",
            {"buffer_id": buffer_id, "line": cursor["line"], "col": cursor["col"]},
        )
        target = definition.get("target") if isinstance(definition, dict) else None
        if target:
            path = target.get("path", "<unknown>")
            line = int(target.get("line", 0)) + 1
            col = int(target.get("col", 0)) + 1
            message = f"definition: {path}:{line}:{col}"
        else:
            message = "no definition available"
    except RuntimeError as error:
        message = f"definition preview unavailable: {error}"
    notify_editor("info", message)
    return {"message": message}


def handle_completion_lens() -> dict[str, str]:
    buffer_id, cursor = active_buffer_and_cursor()
    try:
        completion = request_editor(
            "editor/requestCompletion",
            {"buffer_id": buffer_id, "line": cursor["line"], "col": cursor["col"]},
        )
        items = completion.get("items", []) if isinstance(completion, dict) else []
        labels = [str(item.get("label", "")) for item in items[:5] if isinstance(item, dict)]
        if labels:
            message = f"{len(items)} completions: {', '.join(labels)}"
        else:
            message = "no completions available"
    except RuntimeError as error:
        message = f"completion lens unavailable: {error}"
    notify_editor("info", message)
    return {"message": message}


def handle_request(request: dict[str, Any]) -> dict[str, Any]:
    method = request.get("method")
    if method == "editor/initialize":
        result: dict[str, Any] = {
            "protocol_version": 1,
            "name": "symbol-lens",
            "capabilities": [
                "symbolLens/hoverLens",
                "symbolLens/definitionPreview",
                "symbolLens/completionLens",
            ],
        }
    elif method == "symbolLens/hoverLens":
        result = handle_hover_lens()
    elif method == "symbolLens/definitionPreview":
        result = handle_definition_preview()
    elif method == "symbolLens/completionLens":
        result = handle_completion_lens()
    else:
        result = {
            "method": method,
            "params": request.get("params", {}),
        }
    return response_for_request(request, result)


def main() -> int:
    while True:
        message = read_message()
        if message is None:
            return 0
        if message.get("type") == "request":
            try:
                write_message(handle_request(message))
                if message.get("method") == "editor/initialize":
                    register_startup_contributions()
            except Exception as error:
                write_message(error_response_for_request(message, str(error)))
        # Responses to startup registration requests are silently dropped.


if __name__ == "__main__":
    raise SystemExit(main())
