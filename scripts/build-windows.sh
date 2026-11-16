#!/usr/bin/env bash
# Cross-compile Lepton-GetThermal for Windows x64 from Linux into ONE exe.
#   scripts/build-windows.sh
# Output: src-tauri/target/x86_64-pc-windows-msvc/release/Lepton-GetThermal_<ver>_x64.exe
#
# The exe uses a static C runtime and statically links WebView2Loader. ONNX
# Runtime (AI upscaling) and the VC++ DLLs it needs are embedded and only
# unpacked to %LOCALAPPDATA%\Lepton-GetThermal\ when upscaling is switched on.
#
# Prereqs: clang lld llvm cabextract curl unzip, rustup target
# x86_64-pc-windows-msvc, cargo-xwin, npm ci.
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET=x86_64-pc-windows-msvc
REL=src-tauri/target/$TARGET/release
LIBS=src-tauri/windows-libs
ORT_VER=1.24.4
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p "$LIBS"

# 1) ONNX Runtime (CPU) from the official Microsoft release.
if [ ! -s "$LIBS/onnxruntime.dll" ]; then
  T=$(mktemp -d)
  curl -fL -o "$T/ort.zip" "https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VER/onnxruntime-win-x64-$ORT_VER.zip"
  unzip -q "$T/ort.zip" -d "$T"
  cp "$T/onnxruntime-win-x64-$ORT_VER/lib/onnxruntime.dll" "$LIBS/"
  rm -rf "$T"
fi

# 2) VC++ 2015-2022 x64 runtime DLLs needed by onnxruntime.dll.
if [ ! -s "$LIBS/MSVCP140.dll" ]; then
  T=$(mktemp -d)
  curl -fL -o "$T/vc_redist.x64.exe" https://aka.ms/vs/17/release/vc_redist.x64.exe
  cabextract -q -d "$T/l1" "$T/vc_redist.x64.exe"
  for f in "$T"/l1/*; do cabextract -q -d "$T/l2" "$f" 2>/dev/null || true; done
  for n in msvcp140 msvcp140_1 vcruntime140 vcruntime140_1; do
    cp "$T/l2/$n.dll_amd64" "$LIBS/${n^^}.dll"
  done
  rm -rf "$T"
fi

# 3) Build (no installer: the exe is the deliverable).
npm run tauri build -- --runner cargo-xwin --target $TARGET --no-bundle

VER=$(node -p "require('./src-tauri/tauri.conf.json').version")
OUT=$REL/Lepton-GetThermal_${VER}_x64.exe
cp "$REL/lepton-getthermal.exe" "$OUT"
ls -la "$OUT"
