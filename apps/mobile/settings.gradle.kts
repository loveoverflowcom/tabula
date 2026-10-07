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
        // LiveKit 2.29.0 pins davidliu/audioswitch at 039a35aefab7747c557242fa216c9ea11743b604.
        // Resolve only that provider module from JitPack, never arbitrary project dependencies.
        exclusiveContent {
            forRepository { maven { url = uri("https://jitpack.io") } }
            filter { includeModule("com.github.davidliu", "audioswitch") }
        }
    }
}

rootProject.name = "tabula-mobile"

// `:shared` is the Compose Multiplatform library (UI, navigation, host interfaces).
// `:android` is the Android application host; `ios/` is the Xcode host of the same library.
include(":shared", ":android")
// Desktop preview and UI tests of the shared shell (testing only, ADR-0033). Not a product.
include(":previewApp")
