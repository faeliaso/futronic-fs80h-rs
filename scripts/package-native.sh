#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n1)"
case "$(uname -s)" in
  Darwin) target="darwin-$(uname -m)"; library="libfutronic_fs80h_rs.dylib"; libusb="$(pkg-config --variable=libdir libusb-1.0)/libusb-1.0.0.dylib" ;;
  Linux) target="linux-$(uname -m)"; library="libfutronic_fs80h_rs.so"; libusb="$(pkg-config --variable=libdir libusb-1.0)/libusb-1.0.so.0" ;;
  *) echo "unsupported host; package on macOS or Linux" >&2; exit 1 ;;
esac

cd "$root"
cargo build --release --features native
out="dist/futronic-fs80h-native-v${version}-${target}"
rm -rf "$out"
mkdir -p "$out/lib" "$out/include" "$out/docs"
cp "target/release/$library" "$out/lib/"
if [ -f "$libusb" ]; then cp -L "$libusb" "$out/lib/"; else echo "libusb runtime not found: $libusb" >&2; exit 1; fi
if [ "$(uname -s)" = Darwin ]; then
  install_name_tool -id "@rpath/$library" "$out/lib/$library"
  install_name_tool -change "$(otool -L "$out/lib/$library" | awk 'NR == 3 {print $1}')" '@loader_path/libusb-1.0.0.dylib' "$out/lib/$library"
elif command -v patchelf >/dev/null; then
  patchelf --set-rpath '$ORIGIN' "$out/lib/$library"
else
  echo "patchelf is required to package a relocatable Linux artifact" >&2
  exit 1
fi
cp include/futronic_fs80h.h "$out/include/"
cp README.md README_EN.md EXAMPLES.md LICENSE c_src/LICENSE-NIST "$out/docs/"
driver_sha="$(shasum -a 256 "$out/lib/$library" | awk '{print $1}')"
usb_sha="$(shasum -a 256 "$out/lib/$(basename "$libusb")" | awk '{print $1}')"
header_sha="$(shasum -a 256 "$out/include/futronic_fs80h.h" | awk '{print $1}')"
cat > "$out/manifest.json" <<EOF
{
  "version": "$version",
  "abi_version": 2,
  "target": "$target",
  "enabled_features": ["wsq", "image-formats", "templates", "ansi-378", "matcher", "biometric-core", "mcc"],
  "artifacts": {
    "lib/$library": "$driver_sha",
    "lib/$(basename "$libusb")": "$usb_sha",
    "include/futronic_fs80h.h": "$header_sha"
  }
}
EOF
(cd dist && tar -czf "$(basename "$out").tar.gz" "$(basename "$out")")
shasum -a 256 "dist/$(basename "$out").tar.gz" > "dist/$(basename "$out").tar.gz.sha256"
