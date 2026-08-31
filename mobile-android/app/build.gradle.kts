import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// `kotlinOptions { jvmTarget = "17" }` (the String-setter form) is a
// hard error under the Kotlin Gradle plugin version this project
// resolved -- confirmed directly by running `gradle wrapper` against
// this exact build script, not assumed from a version-compatibility
// table. The current, non-deprecated form is the `compilerOptions` DSL.
kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

// ONYX-MOB-01 §3/§25: namespace/applicationId com.onyx, minSdk 29,
// Java/Kotlin JVM target 17, jniLibs native delivery, target ABIs
// arm64-v8a/armeabi-v7a/x86_64.
android {
    namespace = "com.onyx"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.onyx"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-a1-skeleton"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    // Native libraries are delivered via jniLibs, built separately by
    // tool/build_rust_jni.sh (cargo-ndk, mirroring mobile/tool/
    // build_rust_android.sh's existing pattern) rather than a Gradle
    // Cargo plugin -- keeps this module's build self-contained and
    // matches the frozen Flutter app's own existing convention instead
    // of introducing a second, different native-build mechanism.
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    packaging {
        // mobile-core is a real, sizeable native library shared with
        // multiple ABIs; no reason yet to strip/compress differently
        // than Android's own defaults, so nothing overridden here.
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.03.00")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.10.0")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    // HTTP client for A3's login/hierarchy/refresh calls, mirroring
    // Dart's `net/auth.dart` (which uses `dio` for the identical
    // purpose). OkHttp 4.12.0 -- confirmed via Context7
    // (square.github.io/okhttp) and Maven Central's own search API that
    // this is the latest genuinely *stable* release; 5.x exists only as
    // a long-running alpha series (5.0.0-alpha.16 at time of checking),
    // not something to depend on for real app code.
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    // ReLinker deliberately NOT included -- see DECISIONS.md's A1 entry.
    // Confirmed via current Android NDK docs (developer.android.com/ndk/
    // guides/jni-tips): ReLinker addresses native-library loading issues
    // "on older Android versions" and is called out specifically for
    // "apps targeting Android API levels below 18". This project's real
    // minimum is API 29 (ONYX-MOB-01 §3), well above that threshold, so
    // the documented failure mode ReLinker exists for does not apply
    // here.

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
}
