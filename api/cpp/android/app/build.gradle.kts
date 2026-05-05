// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

plugins {
    id("com.android.application")
}

android {
    namespace = "dev.slint.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.slint.app"
        // API 26 required for InMemoryDexClassLoader used by the Slint Android backend
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"

        ndk {
            // Add other ABIs as needed: "armeabi-v7a", "x86_64", "x86"
            abiFilters += "arm64-v8a"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }
}
