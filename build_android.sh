#!/bin/bash
#
# Build Tantivy Rust library for Android ABIs.
# Prerequisites:
#   rustup target add aarch64-linux-android x86_64-linux-android
#   cargo install cargo-ndk
#   ANDROID_NDK_HOME environment variable (or Android Studio default path)

set -euo pipefail

# Ensure cargo is on PATH
export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$SCRIPT_DIR/rust"
JNI_LIBS_DIR="$SCRIPT_DIR/tantivy-android/src/main/jniLibs"

# Auto-detect NDK from Android SDK if not set
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    # macOS: Android Studio default
    SDK_DIR="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
    if [ -f "$SDK_DIR/ndk/27.2.12479018/source.properties" ]; then
        export ANDROID_NDK_HOME="$SDK_DIR/ndk/27.2.12479018"
    elif [ -f "$SDK_DIR/ndk/27.1.12297006/source.properties" ]; then
        export ANDROID_NDK_HOME="$SDK_DIR/ndk/27.1.12297006"
    elif [ -f "$SDK_DIR/ndk-bundle/source.properties" ]; then
        export ANDROID_NDK_HOME="$SDK_DIR/ndk-bundle"
    elif command -v ndk-build &>/dev/null; then
        export ANDROID_NDK_HOME="$(dirname "$(command -v ndk-build)")"
    else
        echo "Error: ANDROID_NDK_HOME not set and NDK not found."
        echo "Install NDK via Android Studio SDK Manager or set ANDROID_NDK_HOME."
        exit 1
    fi
fi

echo "NDK: $ANDROID_NDK_HOME"

# Build for supported ABIs
TARGETS=(
    "aarch64-linux-android"
    "x86_64-linux-android"
)

ABI_MAP_aarch64_linux_android="arm64-v8a"
ABI_MAP_x86_64_linux_android="x86_64"

for target in "${TARGETS[@]}"; do
    echo ""
    echo "=== Building for $target ==="
    cd "$RUST_DIR"
    cargo ndk --target "$target" build --release

    abi_var="ABI_MAP_${target//-/_}"
    abi="${!abi_var}"
    out_dir="$JNI_LIBS_DIR/$abi"
    mkdir -p "$out_dir"

    # Find the .so file
    so_file="$RUST_DIR/target/$target/release/libtantivy_android.so"
    if [ -f "$so_file" ]; then
        cp "$so_file" "$out_dir/"
        SIZE=$(ls -lh "$so_file" | awk '{print $5}')
        echo "  → $abi ($SIZE)"
    else
        echo "  Error: .so not found at $so_file"
        exit 1
    fi
done

echo ""
echo "Done. Libraries in: $JNI_LIBS_DIR"
ls -lh "$JNI_LIBS_DIR"/*/
