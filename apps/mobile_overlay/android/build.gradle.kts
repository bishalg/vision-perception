plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "org.curv.overlay"
    compileSdk = 34

    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    composeOptions {
        kotlinCompilerExtensionVersion = "1.5.8"
    }
}

dependencies {
    api("androidx.core:core-ktx:1.12.0")
    api("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
    api("androidx.lifecycle:lifecycle-viewmodel-compose:2.7.0")
    api("androidx.activity:activity-compose:1.8.2")

    api(platform("androidx.compose:compose-bom:2024.02.00"))
    api("androidx.compose.ui:ui")
    api("androidx.compose.ui:ui-graphics")
    api("androidx.compose.material3:material3")
    api("androidx.compose.material:material-icons-extended")

    val cameraxVersion = "1.3.1"
    api("androidx.camera:camera-core:$cameraxVersion")
    api("androidx.camera:camera-camera2:$cameraxVersion")
    api("androidx.camera:camera-lifecycle:$cameraxVersion")
    api("androidx.camera:camera-view:$cameraxVersion")
}
