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

// ADR-0043: no web gameplay bundle is a mobile input. Native packaging will be added only with
// an executable platform adapter; meanwhile the empty launch catalog fails closed.

// AGP 9 supplies built-in Kotlin. The Compose compiler and all UI code live in :shared.
dependencies {
    implementation(project(":shared"))
}

// Optional, ignored localhost voice credential. It is native-only and packaged ONLY in debug;
// release assets never contain it (ADR-0037).
val voiceDevFixture = layout.projectDirectory.file("../voice-dev-grant.json")
val voiceDevAssets = layout.buildDirectory.dir("generated/voice-dev-assets")
val prepareVoiceDevFixture = tasks.register("prepareVoiceDevFixture") {
    // Missing local fixture is an empty collection, not a nonexistent @InputFile. Gradle's
    // optional() permits an unset property; a supplied path still has to exist.
    inputs.files(fileTree(layout.projectDirectory.dir("..")) {
        include("voice-dev-grant.json")
    }).withPropertyName("voiceDevFixture")
    outputs.dir(voiceDevAssets)
    val source = voiceDevFixture.asFile
    val destination = voiceDevAssets.get().asFile
    doLast {
        // A Sync would skip NO-SOURCE and retain a previous token after the fixture is removed.
        // This task clears stale generated assets when the fixture is added, changed or removed.
        check(!destination.exists() || destination.deleteRecursively()) { "Cannot clear voice dev assets" }
        if (source.isFile) {
            check(destination.mkdirs()) { "Cannot create voice dev assets" }
            source.copyTo(destination.resolve("voice-dev-grant.json"))
        }
    }
}
android.sourceSets.getByName("debug").assets.srcDir(voiceDevAssets.get().asFile)
tasks.matching { it.name == "mergeDebugAssets" }.configureEach { dependsOn(prepareVoiceDevFixture) }
