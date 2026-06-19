#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# cef_fake.sh — CI smoke fixture: simulate `libcef.so` layout without
# shipping the real Chromium binary (~280 MB).
#
# Purpose:
#   `cargo test -p viscos-webview --features viscos-webview/test-cef-mock`
#   runs `mock_cef_browser_handle` test which expects:
#   1. `runtime_dir/libcef.so` (or `libcef.dll` on Windows) to exist
#      so that `CefBackend::dll_path_or_error()` resolves.
#   2. The file does NOT need to be a valid ELF/PE — a zero-byte
#      symlink target is sufficient because `check_cef_dll_present`
#      only checks `is_file()`, not integrity.
#
# Usage:
#   ./tests/fixtures/cef_fake.sh /tmp/cef-fake
#   RUNTIME_DIR=/tmp/cef-fake cargo test -p viscos-webview \
#       --features viscos-webview/test-cef-mock

set -euo pipefail

TARGET_DIR="${1:-${RUNTIME_DIR:-/tmp/cef-fake}}"

if [[ -e "$TARGET_DIR" ]]; then
    echo "fixture: $TARGET_DIR already exists, skipping create" >&2
    exit 0
fi

mkdir -p "$TARGET_DIR"

case "$(uname -s)" in
    Linux)
        touch "$TARGET_DIR/libcef.so"
        ;;
    Darwin)
        touch "$TARGET_DIR/libcef.dylib"
        ;;
    *)
        # Windows: bash fixture no-op (Cygwin/Git-Bash path). Real Windows
        # CI uses cef-backend.yml workflow with `windows-version` detection.
        echo "fixture: non-POSIX platform, no-op" >&2
        ;;
esac

echo "fixture: fake CEF runtime created at $TARGET_DIR"
