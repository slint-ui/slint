// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore getenv

plugins {
    id("com.android.application")
}

// versionName comes from Cargo.toml unless SLINT_VERSION overrides it; the
// versionCode is the Play build's SLINT_BUILD_NUMBER, else derived from it.
val slintVersion = System.getenv("SLINT_VERSION")
    ?: Regex("""(?m)^version = "([^"]+)"""")
        .find(file("../../../../Cargo.toml").readText())!!.groupValues[1]
val (major, minor, patch) = slintVersion.substringBefore('-').split('.').map(String::toInt)

// Mirror `[package.metadata.android]` in tools/viewer/Cargo.toml so the AAB
// and the cargo-apk APK match.
android {
    namespace = "dev.slint.viewer"
    compileSdk = 35
    buildToolsVersion = "35.0.1"

    defaultConfig {
        applicationId = "dev.slint.viewer"
        minSdk = 26
        targetSdk = 35
        versionCode = System.getenv("SLINT_BUILD_NUMBER")?.toIntOrNull()
            ?: (major * 10000 + minor * 100 + patch)
        versionName = slintVersion
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    val keystorePath: String? = System.getenv("ANDROID_KEYSTORE_PATH")
    if (keystorePath != null) {
        signingConfigs {
            create("release") {
                storeFile = file(keystorePath)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEYSTORE_ALIAS")
                keyPassword = storePassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Unsigned bundles work for local tests; Play won't accept them.
            if (keystorePath != null) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
}
