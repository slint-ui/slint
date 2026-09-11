// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore getenv Pslint

import com.android.build.api.variant.FilterConfiguration.FilterType.ABI

plugins {
    id("com.android.application")
}

// versionName comes from Cargo.toml unless SLINT_VERSION overrides it; the
// versionCode is the Play build's SLINT_BUILD_NUMBER, else derived from it.
val slintVersion = System.getenv("SLINT_VERSION")
    ?: Regex("""(?m)^version = "([^"]+)"""")
        .find(file("../../../../Cargo.toml").readText())!!.groupValues[1]
val (major, minor, patch) = slintVersion.substringBefore('-').split('.').map(String::toInt)
val releaseVersionCode = major * 10000 + minor * 100 + patch

// Each per-ABI APK needs a distinct versionCode: the release code times ten
// plus this offset, so a device supporting several ABIs (arm64 also runs
// armeabi-v7a) installs the highest, i.e. arm64. The build entries of the
// F-Droid recipe (fdroid/dev.slint.viewer.yml) follow this order.
val abiVersionCodeOffset = mapOf("armeabi-v7a" to 1, "x86_64" to 2, "arm64-v8a" to 3)

// -Pslint.abi=<abi> restricts the APKs to one ABI; F-Droid builds one ABI
// per version code that way.
val abis = (project.findProperty("slint.abi") as String?)?.let { listOf(it) } ?: abiVersionCodeOffset.keys.toList()

// Mirror `[package.metadata.android]` in tools/viewer/Cargo.toml so the AAB
// and the cargo-apk APK match.
android {
    namespace = "dev.slint.viewer"
    compileSdk = 36
    buildToolsVersion = "36.0.0"

    defaultConfig {
        applicationId = "dev.slint.viewer"
        minSdk = 26
        targetSdk = 36
        versionCode = System.getenv("SLINT_BUILD_NUMBER")?.toIntOrNull() ?: releaseVersionCode
        versionName = slintVersion
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
            // The VCS record would carry the checkout path, which F-Droid's
            // rebuild can't match.
            vcsInfo {
                include = false
            }
        }
    }

    // One APK per ABI so a download carries only its architecture's native
    // libraries (Skia dominates the size). `bundleRelease` ignores `splits`
    // and keeps every ABI in the bundle.
    splits {
        abi {
            isEnable = true
            reset()
            include(*abis.toTypedArray())
        }
    }
}

// The bundle has no ABI filter and keeps `defaultConfig`'s versionCode.
androidComponents {
    onVariants { variant ->
        variant.outputs.forEach { output ->
            val abi = output.filters.find { it.filterType == ABI }?.identifier ?: return@forEach
            output.versionCode.set(releaseVersionCode * 10 + abiVersionCodeOffset.getValue(abi))
        }
    }
}
