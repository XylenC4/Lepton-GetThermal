#!/usr/bin/env bash
# Cross-compile the Windows x64 NSIS installer from Linux.
# Prereqs: clang lld llvm nsis, rustup target x86_64-pc-windows-msvc, cargo-xwin, npm install.
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET=x86_64-pc-windows-msvc
REL=src-tauri/target/$TARGET/release
export PATH="$HOME/.cargo/bin:$PATH"

# 1) Make sure the xwin SDK is downloaded (first run), then add case variants that
#    onnxruntime's link directives expect (Linux file systems are case-sensitive).
cargo xwin env --target $TARGET >/dev/null 2>&1 || true
SDK="${XWIN_CACHE_DIR:-$HOME/.cache/cargo-xwin}/xwin/sdk/lib/um/x86_64"
[ -e "$SDK/DirectML.lib" ] || ln -s directml.lib "$SDK/DirectML.lib"
[ -e "$SDK/PathCch.lib" ]  || ln -s pathcch.lib  "$SDK/PathCch.lib"

# 2) ort-sys's copy-dylibs step expects these directories to exist.
mkdir -p "$REL/build/examples" "$REL/build/deps" "$REL/examples" "$REL/deps"

# 3) Fetch ort's prebuilt binaries (build script) and stage DirectML.dll for the installer.
build_app() { npm run tauri build -- --runner cargo-xwin --target $TARGET "$@"; }
DML=$(ls -1 "$HOME"/.cache/ort.pyke.io/dfbin/$TARGET/*/DirectML.dll 2>/dev/null | head -1 || true)
if [ -z "$DML" ]; then
  (cd src-tauri && cargo xwin build --release --target $TARGET -p ort-sys) || true
  DML=$(ls -1 "$HOME"/.cache/ort.pyke.io/dfbin/$TARGET/*/DirectML.dll | head -1)
fi
mkdir -p src-tauri/windows-libs
cp -f "$DML" src-tauri/windows-libs/DirectML.dll
# Microsoft VC++ 2015-2022 x64 runtime (MSVCP140.dll, used by ONNX Runtime); run by the installer if missing.
[ -s src-tauri/windows-libs/vc_redist.x64.exe ] || \
  curl -fL -o src-tauri/windows-libs/vc_redist.x64.exe https://aka.ms/vs/17/release/vc_redist.x64.exe

build_app
ls -la "$REL/bundle/nsis/"
