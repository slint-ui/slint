// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore getenv androidkey

plugins {
    id("com.android.application")
}

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
        versionCode = System.getenv("SLINT_BUILD_NUMBER")?.toIntOrNull() ?: 1
        versionName = System.getenv("SLINT_VERSION") ?: "0.0.0"
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
                keyAlias = System.getenv("ANDROID_KEY_ALIAS") ?: "androidkey"
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
                    ?: System.getenv("ANDROID_KEYSTORE_PASSWORD")
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
