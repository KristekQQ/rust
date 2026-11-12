#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
sdk="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
ndk="${ANDROID_NDK_HOME:-$sdk/ndk/27.2.12479018}"
buildtools="$sdk/build-tools/35.0.0"
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home}"
export PATH="$JAVA_HOME/bin:$PATH"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android29-clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
out="$repo/output/native-rim-apk"
mkdir -p "$out/cache" "$out/staging/lib/arm64-v8a" "$out/staging/assets/rim" "$out/staging/assets/licenses"
loader="$out/cache/openxr_loader_for_android-1.1.40.aar"
if [[ ! -f "$loader" ]]; then
  curl -fL --retry 2 -o "$loader" https://repo.maven.apache.org/maven2/org/khronos/openxr/openxr_loader_for_android/1.1.40/openxr_loader_for_android-1.1.40.aar
fi
printf '%s  %s\n' cccad35e6e6fb4a324f376a8628e24a872ea8a3acf10a446128a0fb7d3ea964d "$loader" | shasum -a 256 -c -
cargo build --manifest-path "$here/Cargo.toml" --locked --target aarch64-linux-android --release --lib
cp "$here/target/aarch64-linux-android/release/libkristek_rim_native.so" "$out/staging/lib/arm64-v8a/"
unzip -p "$loader" prefab/modules/openxr_loader/libs/android.arm64-v8a/libopenxr_loader.so > "$out/staging/lib/arm64-v8a/libopenxr_loader.so"
unzip -p "$loader" META-INF/LICENSE > "$out/staging/assets/licenses/openxr-loader.txt"
cp "$repo/assets/models/rim/"*.glb "$repo/assets/models/rim/layout.json" "$repo/assets/models/rim/collisions.json" "$repo/assets/models/rim/SOURCE.md" "$repo/assets/models/rim/LICENSE-three.txt" "$out/staging/assets/rim/"
"$buildtools/aapt2" link -o "$out/unsigned.apk" --manifest "$here/android/AndroidManifest.xml" -I "$sdk/platforms/android-35/android.jar" --min-sdk-version 29 --target-sdk-version 35
python3 - "$out" <<'PY'
from pathlib import Path
import sys,zipfile
out=Path(sys.argv[1])
with zipfile.ZipFile(out/'unsigned.apk','a') as apk:
    for path in sorted((out/'staging').rglob('*')):
        if path.is_file(): apk.write(path,path.relative_to(out/'staging'),compress_type=zipfile.ZIP_STORED)
PY
"$buildtools/zipalign" -P 16 -f 4 "$out/unsigned.apk" "$out/aligned.apk"
key="$out/debug.keystore"
if [[ ! -f "$key" ]]; then
  keytool -genkeypair -keystore "$key" -storepass android -keypass android -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 -dname 'CN=Rim Local Debug,O=Kristek,C=CZ'
fi
"$buildtools/apksigner" sign --ks "$key" --ks-pass pass:android --ks-key-alias androiddebugkey --out "$out/kristek-rim-debug.apk" "$out/aligned.apk"
"$buildtools/apksigner" verify --verbose "$out/kristek-rim-debug.apk"
"$buildtools/zipalign" -c -P 16 4 "$out/kristek-rim-debug.apk"
shasum -a 256 "$out/kristek-rim-debug.apk"
printf 'APK: %s\n' "$out/kristek-rim-debug.apk"
