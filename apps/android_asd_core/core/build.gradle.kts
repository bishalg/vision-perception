// owner: WP-A.12. AAR org.curv.asd:core (AND-2): the UniFFI Kotlin bindings and the
// libasd_ffi.so files that scripts/build_asd_android.sh writes to build/asd/android. This
// module holds no logic of its own; everything is in rust/crates/asd-ffi (project CLAUDE.md).
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

val repoRoot: File = rootDir.resolve("../..").canonicalFile

/** Output of scripts/build_asd_android.sh (`-PasdAndroidOut=` overrides it). */
val asdAndroidOut: File =
    (findProperty("asdAndroidOut") as String?)?.let(::File)
        ?: repoRoot.resolve("build/asd/android")

/** The AAR carries the Rust workspace version (rust/Cargo.toml `[workspace.package]`). */
val rustWorkspaceVersion: String =
    Regex("""(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"""")
        .find(repoRoot.resolve("rust/Cargo.toml").readText())
        ?.groupValues?.get(1)
        ?: error("no [workspace.package] version in rust/Cargo.toml")

group = "org.curv.asd"
version = rustWorkspaceVersion

android {
    namespace = "org.curv.asd.core"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets {
        getByName("main") {
            kotlin.srcDir(asdAndroidOut.resolve("kotlin"))
            jniLibs.srcDir(asdAndroidOut.resolve("jniLibs"))
        }
    }

    testOptions {
        unitTests.all { test ->
            // The JVM test loads the host libasd_ffi built by `build_asd_android.sh --test`.
            test.systemProperty("jna.library.path", asdAndroidOut.resolve("host").path)
            test.systemProperty("asd.repoRoot", repoRoot.path)
            test.inputs.dir(asdAndroidOut.resolve("host"))
            test.testLogging {
                events("passed", "skipped", "failed")
                exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
            }
        }
    }
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    // The generated bindings call the library through JNA (Apache-2.0; docs/license_policy.md).
    // `@aar` carries libjnidispatch.so for the Android ABIs.
    implementation("net.java.dev.jna:jna:5.17.0@aar")

    testImplementation("junit:junit:4.13.2")
    // The jar carries the desktop libjnidispatch the JVM unit test needs.
    testImplementation("net.java.dev.jna:jna:5.17.0")
    // Test-only JSON reader for the golden vectors; not part of the AAR.
    testImplementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.8.1")
}
