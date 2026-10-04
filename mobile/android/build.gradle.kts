plugins {
    alias(libs.plugins.android.application)
}

android {
    namespace = "com.loveoverflow.tabula.mobile.android"
    compileSdk = 37
    defaultConfig {
        applicationId = "com.loveoverflow.tabula"
        minSdk = 24
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

// The first-party game bundle (ADR-0033) is staged by `cargo xtask stage-mobile-game` into
// `target/tabula-mobile-game` and packaged as assets under `tabula-game/`. The app serves it to its
// WebView through request interception; there is no network and no file:// access. Without the
// staged bundle the app still builds and its Home screen says no game is packaged, unless
// `-Ptabula.requireGameBundle=true` (CI and release) turns that into a build failure.
val gameBundleSource = layout.projectDirectory.dir("../../target/tabula-mobile-game")
val gameBundleAssets = layout.buildDirectory.dir("generated/tabula-game-assets")
// A separate check task: a Sync with a missing source is skipped as NO-SOURCE, so a guard inside it
// would never run and a missing bundle would pass silently.
val verifyGameBundle = tasks.register("verifyGameBundle") {
    val required = providers.gradleProperty("tabula.requireGameBundle").map { it == "true" }.orElse(false)
    val manifest = gameBundleSource.file("tabula-games.json").asFile
    doLast {
        if (!manifest.isFile) {
            if (required.get()) throw GradleException("Game bundle missing at ${manifest.parent}; run `cargo xtask stage-mobile-game` first.")
            logger.warn("No staged game bundle at ${manifest.parent}: the app will show no packaged game.")
        }
    }
}
val prepareGameBundle = tasks.register<Sync>("prepareGameBundle") {
    dependsOn(verifyGameBundle)
    from(gameBundleSource)
    into(gameBundleAssets.map { it.dir("tabula-game") })
}
// A plain File, not a Provider: the source-set API rejects providers; the task dependency is explicit below.
android.sourceSets.getByName("main").assets.srcDir(gameBundleAssets.get().asFile)
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }.configureEach { dependsOn(prepareGameBundle) }

// AGP 9 supplies built-in Kotlin. The Compose compiler and all UI code live in :shared.
dependencies {
    implementation(project(":shared"))
}
