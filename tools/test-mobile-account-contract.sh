#!/usr/bin/env bash
# Focused account/coordinator/copy/public-route/Back tests. No Compose/provider/device acceptance.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
: "${KOTLIN_HOME:?Set KOTLIN_HOME to the official Kotlin 2.4.20 compiler directory}"
: "${COROUTINES_JAR:?Set COROUTINES_JAR explicitly; report its version with the result}"
: "${JUNIT_JAR:?Set JUNIT_JAR to JUnit 4.13.2}"
: "${HAMCREST_JAR:?Set HAMCREST_JAR to Hamcrest Core 1.3}"
for file in "$KOTLIN_HOME/bin/kotlinc" "$KOTLIN_HOME/lib/kotlin-test.jar" \
    "$KOTLIN_HOME/lib/kotlin-test-junit.jar" "$COROUTINES_JAR" "$JUNIT_JAR" "$HAMCREST_JAR"; do
    [[ -f "$file" ]] || { echo "Missing prerequisite: $file" >&2; exit 2; }
done
build="$(mktemp -d "${TMPDIR:-/tmp}/tabula-account-contract.XXXXXX")"
trap 'rm -rf "$build"' EXIT
base="$root/apps/mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile"
tests="$root/apps/mobile/shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile"
preview="$root/apps/mobile/previewApp/src"
sources=(
    "$base/account/AccountState.kt" "$base/account/AccountSessionPort.kt" "$base/account/AccountSessionController.kt"
    "$base/localization/ShellStrings.kt" "$base/localization/AccountStrings.kt"
    "$base/navigation/AccountTaskBackPort.kt" "$base/navigation/BackStack.kt"
    "$base/host/GameHostContract.kt" "$base/bridge/BridgeMessages.kt" "$base/bridge/StrictJson.kt"
    "$tests/AccountStateTest.kt" "$tests/AccountSessionControllerTest.kt"
    "$tests/AccountStringsTest.kt" "$tests/ShellStringsTest.kt" "$tests/AccountTaskBackPortTest.kt"
    "$tests/BackStackTest.kt"
    "$preview/main/kotlin/com/loveoverflow/tabula/mobile/preview/SyntheticPreviewAccountFixture.kt"
    "$preview/test/kotlin/com/loveoverflow/tabula/mobile/preview/SyntheticPreviewAccountFixtureTest.kt"
)
classpath="$COROUTINES_JAR:$KOTLIN_HOME/lib/kotlin-test.jar:$KOTLIN_HOME/lib/kotlin-test-junit.jar:$JUNIT_JAR:$HAMCREST_JAR"
"$KOTLIN_HOME/bin/kotlinc" -version
"$KOTLIN_HOME/bin/kotlinc" "${sources[@]}" -Werror -jvm-target 17 -classpath "$classpath" -d "$build/tests.jar"
java -cp "$build/tests.jar:$classpath:$KOTLIN_HOME/lib/kotlin-stdlib.jar" \
    org.junit.runner.JUnitCore \
    com.loveoverflow.tabula.mobile.AccountStateTest \
    com.loveoverflow.tabula.mobile.AccountSessionControllerTest \
    com.loveoverflow.tabula.mobile.AccountStringsTest \
    com.loveoverflow.tabula.mobile.ShellStringsTest \
    com.loveoverflow.tabula.mobile.AccountTaskBackPortTest \
    com.loveoverflow.tabula.mobile.BackStackTest \
    com.loveoverflow.tabula.mobile.preview.SyntheticPreviewAccountFixtureTest
