#!/usr/bin/env bash
# SuiCash UI Shell APK のビルド。
# Gradle 非依存の手順 (aapt2 link → javac → d8 → zipalign → apksigner)。
# 署名は自動生成するデバッグ鍵。
#
# 必要なもの: JDK / Android SDK build-tools 35 以上 / android-29 以上の android.jar
set -euo pipefail

cd "$(dirname "$0")"
OUT_DIR=build
DIST_DIR=dist
KEY_DIR=keys

# ---------------------------------------------------------------- SDK 解決
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$SDK" ] || [ ! -d "$SDK" ]; then
  echo "ERROR: ANDROID_HOME / ANDROID_SDK_ROOT が未設定か存在しない" >&2
  exit 2
fi

BT_VER="$(ls -1 "$SDK/build-tools" 2>/dev/null | sort -V | tail -1)"
BT="$SDK/build-tools/$BT_VER"

ANDROID_JAR=""
for p in $(ls -1 "$SDK/platforms" 2>/dev/null | sed 's/^android-//' | grep -E '^[0-9]+$' | sort -n); do
  if [ "$p" -ge 29 ] && [ -f "$SDK/platforms/android-$p/android.jar" ]; then
    ANDROID_JAR="$SDK/platforms/android-$p/android.jar"
  fi
done
if [ -z "$ANDROID_JAR" ]; then
  echo "ERROR: API 29 以上の android.jar が見つからない" >&2
  exit 2
fi

echo "build-tools : $BT_VER"
echo "android.jar : $ANDROID_JAR"
echo "javac       : $(javac -version 2>&1)"

# ------------------------------------------------------------- 同梱物の確認
# 顔認証エンジン(SAFR eSDK)の素材は非再配布のため git に無い。
# ビルドする本人がローカルに配置すること(.gitignore 参照)。
if [ ! -f assets/ESDKModels.zip ]; then
  echo "ERROR: assets/ESDKModels.zip が無い(エンジンモデル。非再配布のためローカル配置)" >&2
  exit 2
fi
if [ ! -f libs/arm64-v8a/libESDK-lib.so ]; then
  echo "ERROR: libs/arm64-v8a/*.so が無い(エンジンネイティブ。非再配布のためローカル配置)" >&2
  exit 2
fi
if [ ! -f src/jp/serkenn/hicara/suicashui/SafrLicense.java ]; then
  echo "ERROR: SafrLicense.java が無い(エンジンライセンス。非再配布のためローカル配置)" >&2
  exit 2
fi

# ------------------------------------------------------------- 署名鍵の用意
mkdir -p "$KEY_DIR"
if [ ! -f "$KEY_DIR/debug.keystore" ]; then
  keytool -genkeypair -keystore "$KEY_DIR/debug.keystore" \
    -alias androiddebugkey -storepass android -keypass android \
    -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=SuiCash Debug,O=SuiCash,C=JP"
fi

# ------------------------------------------------------------------ ビルド
rm -rf "$OUT_DIR" "$DIST_DIR"
mkdir -p "$OUT_DIR/obj" "$OUT_DIR/dex" "$DIST_DIR"

"$BT/aapt2" link \
  -I "$ANDROID_JAR" \
  --manifest AndroidManifest.xml \
  -A assets \
  --min-sdk-version 29 \
  --target-sdk-version 29 \
  -o "$OUT_DIR/base.apk"

find src -name '*.java' > "$OUT_DIR/sources.txt"
javac -nowarn -source 8 -target 8 \
  -classpath "$ANDROID_JAR" \
  -encoding UTF-8 -d "$OUT_DIR/obj" \
  @"$OUT_DIR/sources.txt"

"$BT/d8" --min-api 29 --lib "$ANDROID_JAR" --output "$OUT_DIR/dex" $(find "$OUT_DIR/obj" -name '*.class')
(cd "$OUT_DIR/dex" && zip -q "$OLDPWD/$OUT_DIR/base.apk" classes.dex)

# ネイティブライブラリ(extractNativeLibs=true なので圧縮のままでよい)
mkdir -p "$OUT_DIR/lib/arm64-v8a"
cp libs/arm64-v8a/*.so "$OUT_DIR/lib/arm64-v8a/"
(cd "$OUT_DIR" && zip -q -r base.apk lib)

"$BT/zipalign" -f -p 4 "$OUT_DIR/base.apk" "$OUT_DIR/aligned.apk"
"$BT/apksigner" sign \
  --ks "$KEY_DIR/debug.keystore" --ks-pass pass:android \
  --out "$DIST_DIR/suicash-ui-shell.apk" \
  "$OUT_DIR/aligned.apk"

echo
echo "OK: $DIST_DIR/suicash-ui-shell.apk ($(wc -c < "$DIST_DIR/suicash-ui-shell.apk") bytes)"
