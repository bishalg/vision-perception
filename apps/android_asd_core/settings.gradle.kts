// owner: WP-A.12. Android packaging of the ASD Rust core (AND-2): the UniFFI Kotlin
// bindings and libasd_ffi.so from scripts/build_asd_android.sh, as the AAR org.curv.asd:core.
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

rootProject.name = "android_asd_core"
include(":core")
