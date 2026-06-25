"""urvim plugin that runs cargo fmt on Rust files after save."""

from __future__ import annotations

import struct
import subprocess
import sys
from pathlib import Path
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


def send_register_event_hook(request_id: int, event: str, method: str) -> None:
    write_message(
        {
            "type": "request",
            "id": request_id,
            "method": "editor/registerEventHook",
            "params": {
                "event": event,
                "method": method,
            },
        }
    )


def find_cargo_root(file_path: Path) -> Path | None:
    current = file_path.parent
    while True:
        if (current / "Cargo.toml").exists():
            return current
        parent = current.parent
        if parent == current:
            return None
        current = parent


def handle_buffer_saved(params: dict[str, Any]) -> None:
    filetype = params.get("filetype", "")
    path_str = params.get("path") or params.get("file_name")

    if filetype != "rust":
        return

    if not path_str:
        return

    file_path = Path(path_str)
    if file_path.suffix != ".rs":
        return

    cargo_root = find_cargo_root(file_path)
    cwd = cargo_root if cargo_root else file_path.parent

    try:
        result = subprocess.run(
            ["cargo", "fmt", "--", str(file_path)],
            cwd=str(cwd),
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            stderr = result.stderr.strip()
            notify_editor(
                "warn",
                f"cargo fmt failed for {file_path.name}: {stderr}"
                if stderr
                else f"cargo fmt failed for {file_path.name}",
            )
    except FileNotFoundError:
        notify_editor("warn", "cargo fmt: cargo not found on PATH")
    except OSError as error:
        notify_editor("warn", f"cargo fmt: {error}")


def handle_event_notification(method: str, params: dict[str, Any]) -> bool:
    if method == "cargoFmt/onBufferSaved":
        handle_buffer_saved(params)
        return True
    return False


def handle_request(request: dict[str, Any]) -> dict[str, Any]:
    method = request.get("method")
    if method == "editor/initialize":
        result: dict[str, Any] = {
            "protocol_version": 1,
            "name": "cargo-fmt",
            "capabilities": [],
        }
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
                    send_register_event_hook(2, "buffer/saved", "cargoFmt/onBufferSaved")
            except Exception as error:
                write_message(error_response_for_request(message, str(error)))
        elif message.get("type") == "notification":
            handle_event_notification(message.get("method", ""), message.get("params", {}))
        # Responses to fire-and-forget requests are silently dropped.


if __name__ == "__main__":
    raise SystemExit(main())
