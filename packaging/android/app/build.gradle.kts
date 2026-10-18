plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Version from the workspace Cargo.toml, so the APK matches the app it wraps.
val cargoVersion: String = run {
    val toml = rootProject.file("../../Cargo.toml")
    val m = if (toml.exists()) Regex("""(?m)^version\s*=\s*"([^"]+)"""").find(toml.readText()) else null
    m?.groupValues?.get(1) ?: "0.0.0"
}
val versionParts = cargoVersion.substringBefore('-').split('.').map { it.toIntOrNull() ?: 0 } + listOf(0, 0, 0)

android {
    namespace = "ai.storyteller.vectorcraft"
    compileSdk = 35

    defaultConfig {
        applicationId = "ai.storyteller.vectorcraft"
        minSdk = 26 // Android 8.0: WebView with WebAssembly and WebGL2
        targetSdk = 35
        versionCode = versionParts[0] * 10000 + versionParts[1] * 100 + versionParts[2]
        versionName = cargoVersion
    }

    signingConfigs {
        // Optional release signing from environment variables (CI secrets or your shell):
        // VC_KEYSTORE (path), VC_KEYSTORE_PASSWORD, VC_KEY_ALIAS, VC_KEY_PASSWORD.
        val ks = System.getenv("VC_KEYSTORE")
        if (!ks.isNullOrBlank() && file(ks).exists()) {
            create("release") {
                storeFile = file(ks)
                storePassword = System.getenv("VC_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("VC_KEY_ALIAS")
                keyPassword = System.getenv("VC_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfigs.findByName("release")?.let { signingConfig = it }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        buildConfig = true
    }

    androidResources {
        // The .wasm is already compressed well by the APK's zip; keep it stored so it maps fast.
        noCompress += listOf("wasm")
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation("androidx.webkit:webkit:1.12.1")
}
