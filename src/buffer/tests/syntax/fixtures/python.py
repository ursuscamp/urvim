#!/usr/bin/env python3
"""Python syntax fixture.

This docstring spans multiple lines.
"""

# Line comment

def greet(name: str) -> str:
    value = 3.14
    count = 1
    if name in ("Ada", "Grace"):
        return f"Hello, {name}"
    return "Hello"


class Thing(Exception):
    pass


items = ["one", "two", "three"]
mapping = {"count": count, "active": True, "missing": None}
escaped = "line 1\nline 2"
formatted = f"{name} -> {count}"
multiline = f"""first line
{name}
third line"""

@decorator
def decorated(value: int) -> int:
    raw = r"line\nsecond"
    bytes_value = b"bytes\nvalue"
    unicode_value = u"unicode"
    raw_bytes = rb"raw bytes\n"
    combined = r"{name}\n{value}"
    raw_combined = r"{name}\n"
    hex_value = 0xFF
    oct_value = 0o755
    bin_value = 0b1010_0011
    float_value = 1_234.5e-2
    imag = 3.5j
    raw_multiline = r"""first
second"""
    return value
