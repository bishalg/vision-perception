// WP-A.14 (AND-4): throw-away Android front-camera gaze feasibility spike. Not product code,
// excluded from every gate; see README.md.
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

rootProject.name = "android_asd_gaze_spike"
include(":app")
