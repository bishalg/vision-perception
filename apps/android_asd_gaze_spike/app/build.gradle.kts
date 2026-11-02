// WP-A.14 (AND-4) throw-away spike app. Consumes the ASD core AAR built by
// scripts/build_asd_android.sh (apps/android_asd_core, WP-A.12); holds no gaze maths.
import java.net.URI
import java.security.MessageDigest

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val repoRoot: File = rootDir.resolve("../..").canonicalFile

/** The AAR from `scripts/build_asd_android.sh` (`-PasdCoreAar=` overrides the path). */
val asdCoreAar: File =
    (findProperty("asdCoreAar") as String?)?.let(::File)
        ?: repoRoot.resolve("apps/android_asd_core/core/build/outputs/aar/core-release.aar")

if (!asdCoreAar.isFile) {
    throw GradleException(
        "ASD core AAR not found at $asdCoreAar. Run scripts/build_asd_android.sh from the " +
            "repository root first (or pass -PasdCoreAar=/path/to/core-release.aar).",
    )
}

// MediaPipe Face Landmarker model (Apache-2.0, Google AI Edge model card). Downloaded at build
// time with a SHA-256 check, never committed (docs/license_policy.md section 3).
val faceLandmarkerUrl =
    "https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task"
val faceLandmarkerSha256 = "64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff"
val modelAssetDir: File = layout.buildDirectory.dir("generated/spikeModelAssets").get().asFile

val downloadFaceLandmarker by tasks.registering {
    val out = modelAssetDir.resolve("face_landmarker.task")
    outputs.file(out)
    doLast {
        fun sha256(f: File): String =
            MessageDigest.getInstance("SHA-256").digest(f.readBytes()).joinToString("") { "%02x".format(it) }
        if (out.isFile && sha256(out) == faceLandmarkerSha256) return@doLast
        out.parentFile.mkdirs()
        URI(faceLandmarkerUrl).toURL().openStream().use { input -> out.outputStream().use { input.copyTo(it) } }
        val got = sha256(out)
        if (got != faceLandmarkerSha256) {
            out.delete()
            throw GradleException("face_landmarker.task SHA-256 mismatch: $got (expected $faceLandmarkerSha256)")
        }
    }
}

android {
    namespace = "org.curv.asd.spike"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.curv.asd.spike"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1-spike"
    }

    buildTypes {
        release {
            // Never released: a debug build is the only intended output.
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets {
        getByName("main") {
            assets.srcDir(modelAssetDir)
        }
    }

    androidResources {
        // The .task bundle is memory-mapped by MediaPipe.
        noCompress += "task"
    }
}

kotlin {
    jvmToolchain(17)
}

tasks.named("preBuild") { dependsOn(downloadFaceLandmarker) }

dependencies {
    // ASD Rust core: UniFFI Kotlin bindings + libasd_ffi.so (AND-2). JNA as the AAR declares it.
    implementation(files(asdCoreAar))
    implementation("net.java.dev.jna:jna:5.17.0@aar")

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")

    // Back-end 1: CameraX front camera + MediaPipe Face Landmarker (iris), Apache-2.0.
    // 0.10.21 on purpose: it is the last release with armeabi-v7a (32-bit Android Go phones);
    // 0.10.26+ ship arm64-v8a only.
    val camerax = "1.4.2"
    implementation("androidx.camera:camera-core:$camerax")
    implementation("androidx.camera:camera-camera2:$camerax")
    implementation("androidx.camera:camera-lifecycle:$camerax")
    implementation("androidx.camera:camera-view:$camerax")
    implementation("com.google.mediapipe:tasks-vision:0.10.21")

    // Back-end 2: ARCore Augmented Faces (ARCore Additional Terms of Service; spike only).
    implementation("com.google.ar:core:1.57.0")
}
