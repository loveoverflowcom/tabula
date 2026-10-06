plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.android.kmp.library)
}

// Compose Multiplatform owns the Tabula app UI and navigation on Android and iOS (ADR-0032).
// Game rules, presentation and rendering stay in Rust; this module never imports them.
kotlin {
    jvmToolchain(17)

    androidLibrary {
        namespace = "com.loveoverflow.tabula.mobile.shared"
        compileSdk = 37
        minSdk = 24
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
        withHostTest { }
    }

    // The desktop JVM target exists so `:previewApp` can run and test the shared shell on a laptop
    // (testing only, ADR-0033). It is not a product: Tabula ships Android and iOS from here.
    jvm("desktop")

    // Kotlin/Native compiles iOS targets only on macOS; elsewhere Gradle skips them
    // (`kotlin.native.ignoreDisabledTargets`), so the iOS framework is NOT built on Linux.
    listOf(iosArm64(), iosSimulatorArm64()).forEach { target ->
        target.binaries.framework {
            baseName = "TabulaShared"
            isStatic = true
        }
    }

    sourceSets {
        commonMain.dependencies {
            implementation(libs.coroutines.core)
            api(libs.compose.runtime)
            api(libs.compose.foundation)
            api(libs.compose.ui.backhandler)
            api(libs.lifecycle.runtime.compose)
        }
        androidMain.dependencies {
            api(libs.activity.compose)
            implementation(libs.androidx.webkit)
            implementation(libs.coroutines.android)
            implementation(libs.livekit.android)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
    }
}
