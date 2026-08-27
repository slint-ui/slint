// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

plugins {
    id("com.android.application") version "8.7.0"
}

android {
    namespace = "dev.slint.demos.usecases"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.slint.demos.usecases"
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
            path = file("../CMakeLists.txt")
            version = "3.22.1"
        }
    }
}
