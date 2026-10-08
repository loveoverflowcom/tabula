#!/usr/bin/env bash
# Focused native coordinator tests without Gradle/Compose or a fabricated native backend.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
: "${KOTLIN_HOME:?Set KOTLIN_HOME to the official Kotlin 2.4.20 compiler directory}"
: "${JUNIT_JAR:?Set JUNIT_JAR to JUnit 4.13.2}"
: "${HAMCREST_JAR:?Set HAMCREST_JAR to Hamcrest Core 1.3}"
for file in "$KOTLIN_HOME/bin/kotlinc" "$KOTLIN_HOME/lib/kotlin-test.jar" \
    "$KOTLIN_HOME/lib/kotlin-test-junit.jar" "$JUNIT_JAR" "$HAMCREST_JAR"; do
    [[ -f "$file" ]] || { echo "Missing prerequisite: $file" >&2; exit 2; }
done
build="$(mktemp -d "${TMPDIR:-/tmp}/tabula-native-contract.XXXXXX")"
trap 'rm -rf "$build"' EXIT
base="$root/apps/mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile"
sources=(
    "$base/bridge/StrictJson.kt" "$base/bridge/BridgeMessages.kt"
    "$base/host/GameHostContract.kt" "$base/host/GameRuntimeControls.kt"
    "$base/host/NativeHostContract.kt" "$base/host/NativeHostSession.kt" "$base/host/NativeGameRuntime.kt"
    "$root/apps/mobile/shared/src/androidMain/kotlin/com/loveoverflow/tabula/mobile/host/AndroidNativeRuntimePort.kt"
    "$root/apps/mobile/shared/src/androidMain/kotlin/com/loveoverflow/tabula/mobile/host/AndroidNativeRuntimeFactory.kt"
)
classpath="$KOTLIN_HOME/lib/kotlin-test.jar:$KOTLIN_HOME/lib/kotlin-test-junit.jar:$JUNIT_JAR:$HAMCREST_JAR"
"$KOTLIN_HOME/bin/kotlinc" -version
"$KOTLIN_HOME/bin/kotlinc" "${sources[@]}" \
    "$root/apps/mobile/shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/NativeGameRuntimeTest.kt" \
    "$root/apps/mobile/shared/src/androidHostTest/kotlin/com/loveoverflow/tabula/mobile/AndroidNativeRuntimeSkeletonTest.kt" \
    -Werror -jvm-target 17 -classpath "$classpath" -d "$build/tests.jar"
java -cp "$build/tests.jar:$classpath:$KOTLIN_HOME/lib/kotlin-stdlib.jar" \
    org.junit.runner.JUnitCore com.loveoverflow.tabula.mobile.NativeGameRuntimeTest \
    com.loveoverflow.tabula.mobile.AndroidNativeRuntimeSkeletonTest
if [[ -n "${ANDROID_JAR:-}" ]]; then
    [[ -f "$ANDROID_JAR" ]] || { echo "Missing Android platform jar: $ANDROID_JAR" >&2; exit 2; }
    "$KOTLIN_HOME/bin/kotlinc" "${sources[@]}" \
        "$root/apps/mobile/shared/src/androidMain/kotlin/com/loveoverflow/tabula/mobile/host/AndroidNativeSurfaceBinding.kt" \
        -Werror -jvm-target 17 -classpath "$ANDROID_JAR" -d "$build/android-surface.jar"
    echo "PASS: Android callback source compiled against supplied SDK jar (no APK/device execution)"
fi
if [[ -n "${ANDROID_COMPOSE_CLASSPATH:-}" ]]; then
    # Optional actual-source AndroidView consumer compile. Supply official Android binaries
    # resolved from the repository's pinned CMP/lifecycle metadata, not API stubs or new pins.
    : "${ANDROID_JAR:?ANDROID_COMPOSE_CLASSPATH also requires the Android platform jar}"
    plugin="$KOTLIN_HOME/lib/compose-compiler-plugin.jar"
    [[ -f "$plugin" ]] || { echo "Missing prerequisite: $plugin" >&2; exit 2; }
    android="$root/apps/mobile/shared/src/androidMain/kotlin/com/loveoverflow/tabula/mobile/host"
    "$KOTLIN_HOME/bin/kotlinc" "${sources[@]}" \
        "$base/host/GameHost.kt" "$base/host/GameRuntimeBinding.kt" \
        "$android/AndroidNativeSurfaceBinding.kt" "$android/AndroidNativeGameHost.kt" \
        -Werror -jvm-target 17 -Xplugin="$plugin" \
        -classpath "$ANDROID_JAR:$ANDROID_COMPOSE_CLASSPATH" -d "$build/android-host.jar"
    echo "PASS: actual AndroidView GameHost consumer source compiled (no Gradle/APK/interaction execution)"
fi
