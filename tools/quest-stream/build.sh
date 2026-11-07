#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
BT="$SDK/build-tools/35.0.0"
ANDROID_JAR="$SDK/platforms/android-35/android.jar"
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export PATH="$JAVA_HOME/bin:$PATH"
for tool in "$BT/aapt2" "$BT/d8" "$BT/zipalign" "$BT/apksigner" "$ANDROID_JAR"; do
  [[ -f "$tool" ]] || { echo "Missing Android SDK file: $tool" >&2; exit 1; }
done
mkdir -p build/classes build/dex
find build/classes build/dex -type f -delete
javac --release 8 -classpath "$ANDROID_JAR" -d build/classes src/cz/honza/queststream/*.java
jar cf build/classes.jar -C build/classes .
"$BT/d8" --lib "$ANDROID_JAR" --min-api 29 --output build/dex build/classes.jar
"$BT/aapt2" link -I "$ANDROID_JAR" --manifest AndroidManifest.xml -A assets -o build/unsigned.apk
(cd build/dex && zip -q -j ../unsigned.apk classes.dex)
"$BT/zipalign" -f 4 build/unsigned.apk build/aligned.apk
if [[ ! -f build/development.keystore ]]; then
  keytool -genkeypair -keystore build/development.keystore -storepass android -keypass android -alias development \
    -dname 'CN=Quest Stream Local Development' -keyalg RSA -keysize 2048 -validity 3650 >/dev/null 2>&1
fi
"$BT/apksigner" sign --ks build/development.keystore --ks-pass pass:android --key-pass pass:android --out build/QuestStream.apk build/aligned.apk
"$BT/apksigner" verify build/QuestStream.apk
echo "APK: $(pwd)/build/QuestStream.apk"
