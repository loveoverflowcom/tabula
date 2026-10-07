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
)
classpath="$KOTLIN_HOME/lib/kotlin-test.jar:$KOTLIN_HOME/lib/kotlin-test-junit.jar:$JUNIT_JAR:$HAMCREST_JAR"
"$KOTLIN_HOME/bin/kotlinc" -version
"$KOTLIN_HOME/bin/kotlinc" "${sources[@]}" \
    "$root/apps/mobile/shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/NativeGameRuntimeTest.kt" \
    -Werror -jvm-target 17 -classpath "$classpath" -d "$build/tests.jar"
java -cp "$build/tests.jar:$classpath:$KOTLIN_HOME/lib/kotlin-stdlib.jar" \
    org.junit.runner.JUnitCore com.loveoverflow.tabula.mobile.NativeGameRuntimeTest
if [[ -n "${ANDROID_JAR:-}" ]]; then
    [[ -f "$ANDROID_JAR" ]] || { echo "Missing Android platform jar: $ANDROID_JAR" >&2; exit 2; }
    "$KOTLIN_HOME/bin/kotlinc" "${sources[@]}" \
        "$root/apps/mobile/shared/src/androidMain/kotlin/com/loveoverflow/tabula/mobile/host/AndroidNativeSurfaceBinding.kt" \
        -Werror -jvm-target 17 -classpath "$ANDROID_JAR" -d "$build/android-surface.jar"
    echo "PASS: Android callback source compiled against supplied SDK jar (no APK/device execution)"
fi
