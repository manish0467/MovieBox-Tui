import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.moviebox.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.moviebox.app"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
        debug {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.ui)
    implementation(libs.androidx.ui.graphics)
    implementation(libs.androidx.ui.tooling.preview)
    implementation(libs.androidx.material3)
    implementation(libs.androidx.material.icons.extended)
    implementation(libs.androidx.navigation.compose)

    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.ui)
    implementation(libs.androidx.media3.datasource.okhttp)

    implementation(libs.coil.compose)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)

    debugImplementation(libs.androidx.ui.tooling)
    debugImplementation(libs.androidx.ui.test.manifest)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.ui.test.junit4)
}

/**
 * Builds the Rust core into `src/main/jniLibs/<abi>/libmoviebox_android.so`.
 *
 * Requires the Android NDK and `cargo-ndk`:
 *   cargo install cargo-ndk
 *   rustup target add aarch64-linux-android
 *
 * Enable it with `-PmovieboxBuildRust=true` (or set the property in
 * `gradle.properties`) so that a plain `./gradlew assembleDebug` does not fail
 * on machines without an NDK.
 */
val cargoNdk = "cargo-ndk"
val rustTarget = "aarch64-linux-android"
val rustProfile = "release"

val buildRustLibrary by tasks.registering(Exec::class) {
    group = "moviebox"
    description = "Cross-compiles the Rust core into jniLibs with cargo-ndk."
    workingDir = rootProject.file("..")
    doFirst {
        file("src/main/jniLibs").mkdirs()
    }
    commandLine(
        cargoNdk,
        "-p", "moviebox-android",
        "-t", "arm64-v8a",
        "-o", file("src/main/jniLibs").absolutePath,
        "build",
        "--release",
    )
}

if (project.findProperty("movieboxBuildRust") == "true") {
    tasks.named("preBuild") {
        dependsOn(buildRustLibrary)
    }
}
