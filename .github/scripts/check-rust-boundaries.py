#!/usr/bin/env python3
"""Enforces src/gitlab/ UI isolation with proper Rust lexical scanning."""

from __future__ import annotations

import os
import re
import subprocess
import sys

FORBIDDEN_CODE_RE = re.compile(
    r"\bgpui_kit\b|\bgpui\s*::|\bgpui_app\b|\binclude\s*!\s*[\(\[\{]"
)
FORBIDDEN_PATH_ATTR_RE = re.compile(r"#\s*\[[^\]]*\bpath\s*=", re.DOTALL)
TOML_ALIAS_RE = re.compile(r"""package\s*=\s*["']gpui(?:-kit)?["']""")


def strip_rust_comments_and_strings(src: str) -> str:
    """Replace Rust comments and char/string literals with spaces, preserving newlines."""
    out = []
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        # Line comment
        if ch == "/" and nxt == "/":
            while i < n and src[i] != "\n":
                out.append(" ")
                i += 1
        # Nested block comment
        elif ch == "/" and nxt == "*":
            depth = 1
            out.extend([" ", " "])
            i += 2
            while i < n and depth > 0:
                if src[i] == "/" and i + 1 < n and src[i + 1] == "*":
                    depth += 1
                    out.extend([" ", " "])
                    i += 2
                elif src[i] == "*" and i + 1 < n and src[i + 1] == "/":
                    depth -= 1
                    out.extend([" ", " "])
                    i += 2
                else:
                    out.append("\n" if src[i] == "\n" else " ")
                    i += 1
        # Raw string literal: r#"..."#, br#"..."#, cr#"..."#
        elif (ch == "r" or (ch in ("b", "c") and nxt == "r")) and (
            i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")
        ):
            j = i + (2 if ch in ("b", "c") else 1)
            hashes = 0
            while j < n and src[j] == "#":
                hashes += 1
                j += 1
            if j < n and src[j] == '"':
                out.extend([" "] * (j - i + 1))
                i = j + 1
                end_seq = '"' + "#" * hashes
                while i < n:
                    if src.startswith(end_seq, i):
                        out.extend([" "] * len(end_seq))
                        i += len(end_seq)
                        break
                    out.append("\n" if src[i] == "\n" else " ")
                    i += 1
            else:
                out.append(ch)
                i += 1
        # Char or byte literal (without consuming lifetimes like 'a or 'static)
        elif ch == "'" and i + 2 < n:
            if src[i + 1] == "\\":
                out.extend([" ", " "])
                i += 2
                while i < n and src[i] != "'" and src[i] != "\n":
                    out.append(" ")
                    i += 1
                if i < n and src[i] == "'":
                    out.append(" ")
                    i += 1
            elif src[i + 1] not in ("'", "\n") and src[i + 2] == "'":
                out.extend([" ", " ", " "])
                i += 3
            else:
                out.append(ch)
                i += 1
        # Normal / byte / C string literal
        elif ch == '"':
            out.append(" ")
            i += 1
            while i < n:
                if src[i] == "\\" and i + 1 < n:
                    out.append("\n" if src[i + 1] == "\n" else " ")
                    out.append(" ")
                    i += 2
                elif src[i] == '"':
                    out.append(" ")
                    i += 1
                    break
                else:
                    out.append("\n" if src[i] == "\n" else " ")
                    i += 1
        else:
            out.append(ch)
            i += 1
    return "".join(out)


def check_test_mod_gated(test_file: str) -> str | None:
    """Verify a *_tests.rs file is declared strictly under contiguous #[cfg(test)]."""
    mod_name = os.path.basename(test_file)[:-3]
    parent_dir = os.path.dirname(test_file)
    candidates = [os.path.join(parent_dir, "mod.rs"), f"{parent_dir}.rs"]
    mod_decl_re = re.compile(rf"\bmod\s+{re.escape(mod_name)}\s*;")

    found_decl = False
    for parent in candidates:
        if not os.path.isfile(parent):
            continue
        cleaned = strip_rust_comments_and_strings(open(parent, encoding="utf-8").read())
        lines = cleaned.splitlines()
        for idx, line in enumerate(lines):
            m = mod_decl_re.search(line)
            if not m:
                continue
            found_decl = True
            prefix = line[: m.start()]
            has_inline_delim = any(tok in prefix for tok in (";", "{", "}"))
            tail_prefix = re.split(r"[;{}]", prefix)[-1] if has_inline_delim else prefix
            attrs = [tail_prefix]
            if not has_inline_delim:
                j = idx - 1
                while j >= 0:
                    stripped = lines[j].strip()
                    if stripped == "":
                        j -= 1
                        continue
                    if any(tok in stripped for tok in (";", "{", "}")):
                        break
                    if stripped.startswith("#") or stripped.endswith("]") or stripped.startswith("pub"):
                        attrs.append(stripped)
                        j -= 1
                    else:
                        break
            joined = " ".join(reversed(attrs))
            if not re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", joined):
                return f"{parent}:{idx + 1}: mod {mod_name}; is not directly gated by #[cfg(test)]"
    if not found_decl:
        return f"{test_file}:1: test module {mod_name} has no #[cfg(test)] declaration in parent module"
    return None


def main() -> int:
    errors = []
    if os.path.isfile("Cargo.toml"):
        cargo_text = open("Cargo.toml", encoding="utf-8").read()
        for idx, line in enumerate(cargo_text.splitlines(), 1):
            code = line.split("#", 1)[0]
            if TOML_ALIAS_RE.search(code):
                errors.append(("Cargo.toml", idx, "Renamed gpui/gpui-kit dependency alias is disallowed"))

    raw = subprocess.check_output(
        ["git", "-c", "core.quotePath=false", "ls-files", "-z", "src/gitlab/*.rs"]
    )
    for entry in raw.split(b"\x00"):
        if not entry:
            continue
        path = entry.decode("utf-8", errors="surrogateescape")
        if not os.path.isfile(path) or os.path.islink(path):
            continue
        if path.endswith("_tests.rs"):
            err = check_test_mod_gated(path)
            if err:
                file_part, line_part, msg = err.split(":", 2)
                errors.append((file_part, int(line_part), msg.strip()))
            continue

        cleaned = strip_rust_comments_and_strings(open(path, encoding="utf-8").read())
        for m in FORBIDDEN_CODE_RE.finditer(cleaned):
            lineno = cleaned.count("\n", 0, m.start()) + 1
            errors.append((path, lineno, f"UI or include! dependency found in UI-agnostic module {path}:{lineno}"))
        for m in FORBIDDEN_PATH_ATTR_RE.finditer(cleaned):
            lineno = cleaned.count("\n", 0, m.start()) + 1
            errors.append((path, lineno, f"Attribute #[... path = ...] is disallowed in UI-agnostic module {path}:{lineno}"))

    gha = bool(os.environ.get("GITHUB_ACTIONS"))
    for file_path, lineno, msg in errors:
        if gha:
            print(f"::error file={file_path},line={lineno}::{msg}")
        else:
            print(f"ERROR: {file_path}:{lineno}: {msg}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
