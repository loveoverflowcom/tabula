pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "tabula-mobile"

// `:shared` is the Compose Multiplatform library (UI, navigation, host interfaces).
// `:android` is the Android application host; `ios/` is the Xcode host of the same library.
include(":shared", ":android")
// Desktop preview and UI tests of the shared shell (testing only, ADR-0033). Not a product.
include(":previewApp")
