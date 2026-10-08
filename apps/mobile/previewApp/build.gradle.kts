import org.jetbrains.compose.reload.gradle.ComposeHotRun

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.compose.hot.reload)
}

// Desktop preview and UI tests of the shared Compose shell (ADR-0033). Testing only: there is no
// desktop product, no installer and no WebView here. The game page is a labelled simulation that
// speaks the real bridge protocol so the shell, GameSession and recomposition behaviour can be run
// and asserted on a laptop. It is not evidence about Android WebView or iOS WKWebView.
kotlin { jvmToolchain(17) }

dependencies {
    implementation(project(":shared"))
    implementation(compose.desktop.currentOs)
    implementation(libs.coroutines.swing)
    testImplementation(kotlin("test"))
    testImplementation(libs.coroutines.test)
    testImplementation(compose.desktop.uiTestJUnit4)
}

fun JavaExec.configurePreview() {
    mainClass.set("com.loveoverflow.tabula.mobile.preview.DesktopPreviewKt")
    maxHeapSize = "1g"
    systemProperty("compose.layers.type", "ON_SAME_CANVAS")
    for (option in listOf("width", "height", "dark", "language", "reducedMotion", "fontScale", "catalog", "smokeWindow",
        "account", "accountLongFields", "accountAvatar")) {
        providers.gradleProperty("preview.$option").orNull?.let { systemProperty("tabula.preview.$option", it) }
    }
}

// The existing entrypoint/fixtures serve both ordinary preview and official CHR; no second shell.
tasks.withType<ComposeHotRun>().configureEach { configurePreview() }

tasks.register<JavaExec>("run") {
    group = "application"
    description = "Open the shell in a phone-sized desktop window with the simulated game page."
    classpath = sourceSets.main.get().runtimeClasspath
    configurePreview()
}

tasks.test {
    maxParallelForks = 1
    maxHeapSize = "1g"
    systemProperty("java.awt.headless", "true")
    systemProperty("skiko.renderApi", "SOFTWARE")
    // Screenshots of the rendered shell; copied into the verification ledger by hand, never regenerated silently.
    val screenshots = layout.buildDirectory.dir("reports/shell-screenshots")
    outputs.dir(screenshots)
    systemProperty("tabula.preview.screenshots", screenshots.get().asFile.absolutePath)
}
