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

// AGP 9 supplies built-in Kotlin. The Compose compiler and all UI code live in :shared.
dependencies {
    implementation(project(":shared"))
}
