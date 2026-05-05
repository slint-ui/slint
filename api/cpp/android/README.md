# Slint C++ Android Template

This directory contains a template project for building Slint C++ applications
on Android using Gradle and CMake.

See the [Slint C++ Android documentation](https://slint.dev/docs/guide/platforms/mobile/android-cpp)
for detailed instructions.

## Quick Start

1. Copy this directory to create your project.

2. Set up the required environment variables:
   ```sh
   export ANDROID_HOME=$HOME/Android/Sdk
   export ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/<version>
   ```

3. Install the Rust Android target:
   ```sh
   rustup target add aarch64-linux-android
   ```

4. Edit `app/src/main/cpp/main.cpp` and `app/src/main/cpp/main.slint` with
   your application code.

5. Build and deploy:
   ```sh
   ./gradlew installDebug
   ```

## Project Structure

- `app/build.gradle.kts` - Android app configuration (SDK versions, CMake setup)
- `app/src/main/AndroidManifest.xml` - Android manifest with NativeActivity
- `app/src/main/cpp/CMakeLists.txt` - CMake build for C++ code and Slint
- `app/src/main/cpp/main.cpp` - Application entry point (`slint_main`)
- `app/src/main/cpp/main.slint` - Slint UI definition
