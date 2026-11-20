#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
out=../../output/quest-debug-awake
sdk="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
tools="$sdk/build-tools/35.0.0"
platform="$sdk/platforms/android-35/android.jar"
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export PATH="$JAVA_HOME/bin:$PATH"
mkdir -p "$out/classes" "$out/dex"
javac --release 8 -classpath "$platform" -d "$out/classes" DebugService.java
jar cf "$out/classes.jar" -C "$out/classes" .
"$tools/d8" --lib "$platform" --min-api 29 --output "$out/dex" "$out/classes.jar"
"$tools/aapt2" link -I "$platform" --manifest AndroidManifest.xml -o "$out/base.apk"
zip -qj "$out/base.apk" "$out/dex/classes.dex"
"$tools/zipalign" -f 4 "$out/base.apk" "$out/aligned.apk"
if [[ ! -f "$out/debug.keystore" ]]; then
 keytool -genkeypair -keystore "$out/debug.keystore" -storepass android -keypass android -alias debug -dname 'CN=Local Quest development' -keyalg RSA -validity 3650
fi
"$tools/apksigner" sign --ks "$out/debug.keystore" --ks-pass pass:android --out "$out/debug-awake.apk" "$out/aligned.apk"
