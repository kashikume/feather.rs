// The test app's Android project: packs the Rust library (built by `cargo ndk`, see
// app/build.gradle.kts) and its assets into an APK. See doc/android.md.

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

rootProject.name = "FeatherTestApp"
include(":app")
