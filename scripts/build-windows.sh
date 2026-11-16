#!/usr/bin/env bash
# Cross-compile Lepton-GetThermal for Windows x64 from Linux.
#   scripts/build-windows.sh              -> portable zip (default)
#   scripts/build-windows.sh --installer  -> portable zip + NSIS setup.exe
# Prereqs: clang lld llvm cabextract (nsis for --installer), rustup target
# x86_64-pc-windows-msvc, cargo-xwin, npm install.
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET=x86_64-pc-windows-msvc
REL=src-tauri/target/$TARGET/release
LIBS=src-tauri/windows-libs
export PATH="$HOME/.cargo/bin:$PATH"
INSTALLER=0; [ "${1:-}" = "--installer" ] && INSTALLER=1

# 1) xwin SDK + case variants that onnxruntime's link directives expect.
cargo xwin env --target $TARGET >/dev/null 2>&1 || true
SDK="${XWIN_CACHE_DIR:-$HOME/.cache/cargo-xwin}/xwin/sdk/lib/um/x86_64"
[ -e "$SDK/DirectML.lib" ] || ln -s directml.lib "$SDK/DirectML.lib"
[ -e "$SDK/PathCch.lib" ]  || ln -s pathcch.lib  "$SDK/PathCch.lib"

# 2) ort-sys's copy-dylibs step expects these directories to exist.
mkdir -p "$REL/build/examples" "$REL/build/deps" "$REL/examples" "$REL/deps"

# 3) Runtime DLLs: DirectML (from ort's prebuilt ONNX Runtime) and the
#    Microsoft VC++ 2015-2022 x64 runtime (app-local deployment is allowed).
mkdir -p "$LIBS"
DML=$(ls -1 "$HOME"/.cache/ort.pyke.io/dfbin/$TARGET/*/DirectML.dll 2>/dev/null | head -1 || true)
if [ -z "$DML" ]; then
  (cd src-tauri && cargo xwin build --release --target $TARGET -p ort-sys) || true
  DML=$(ls -1 "$HOME"/.cache/ort.pyke.io/dfbin/$TARGET/*/DirectML.dll | head -1)
fi
cp -f "$DML" "$LIBS/DirectML.dll"
[ -s "$LIBS/vc_redist.exe" ] || \
  curl -fL -o "$LIBS/vc_redist.exe" https://aka.ms/vs/17/release/vc_redist.x64.exe
if [ ! -s "$LIBS/MSVCP140.dll" ]; then
  T=$(mktemp -d)
  cabextract -q -d "$T/l1" "$LIBS/vc_redist.exe"
  for f in "$T"/l1/*; do cabextract -q -d "$T/l2" "$f" 2>/dev/null || true; done
  for n in msvcp140 msvcp140_1 vcruntime140 vcruntime140_1; do
    cp "$T/l2/$n.dll_amd64" "$LIBS/${n^^}.dll"
  done
  rm -rf "$T"
fi

# 4) Build.
if [ $INSTALLER = 1 ]; then
  npm run tauri build -- --runner cargo-xwin --target $TARGET
else
  npm run tauri build -- --runner cargo-xwin --target $TARGET --no-bundle
fi

# 5) Portable zip: exe + DLLs, nothing to install.
VER=$(node -p "require('./src-tauri/tauri.conf.json').version")
NAME=Lepton-GetThermal_${VER}_x64-portable
OUT=$REL/portable/$NAME
rm -rf "$REL/portable" && mkdir -p "$OUT"
cp "$REL/lepton-getthermal.exe" "$OUT/Lepton-GetThermal.exe"
cp "$LIBS"/DirectML.dll "$LIBS"/MSVCP140.dll "$LIBS"/MSVCP140_1.dll \
   "$LIBS"/VCRUNTIME140.dll "$LIBS"/VCRUNTIME140_1.dll "$OUT/"
cp packaging/windows/README-portable.txt "$OUT/README.txt"
(cd "$REL/portable" && python3 -I -c "import shutil,sys; shutil.make_archive(sys.argv[1], 'zip', '.', sys.argv[1])" "$NAME")
ls -la "$REL/portable/$NAME.zip"
[ $INSTALLER = 1 ] && ls -la "$REL/bundle/nsis/" || true
