# C++ Android Support for Slint

Tracking issue: https://github.com/slint-ui/slint/issues/6281

## Status

Initial implementation complete. The todo example has been successfully built
and tested as an APK on an Android device.

## What was implemented

### Phase 1: Rust-side plumbing (api/cpp/)

**Cargo.toml** — new `backend-android-activity` feature:
- Enables `i-slint-backend-android-activity` (native-activity, aa-06)
- Enables `i-slint-backend-selector/backend-android-activity`
- Pulls in `renderer-skia` and `std`
- No version suffix needed (C++ doesn't expose AndroidApp types)

**lib.rs** — `android_main` bridge:
- Gated on `#[cfg(all(target_os = "android", feature = "backend-android-activity"))]`
- Implements `android_main(AndroidApp)` that initializes the Android platform
- Calls `extern "C" slint_main()` — the user's C++ entry point

**cbindgen.rs**:
- Added `backend_android_activity` to feature declarations
  (generates `SLINT_FEATURE_BACKEND_ANDROID_ACTIVITY` define)
- Added `using ::slint::Timer` and `Option<T>` template in cbindgen_private
  namespace (needed for Android-only `ContextMenu::long_press_timer` field)

### Phase 2: CMake integration (api/cpp/CMakeLists.txt)

- `define_cargo_dependent_feature(backend-android-activity ...)` option
- Android auto-configuration block (before feature definitions, so FORCE
  cache values are picked up):
  - Maps `ANDROID_ABI` -> Rust target triple (arm64-v8a, armeabi-v7a, x86_64, x86)
  - Forces `BUILD_SHARED_LIBS=ON`
  - Enables android backend + skia, disables winit/qt/femtovg
  - Sets Material style as default
- NDK/SDK environment variable propagation to the Rust build
  (`ANDROID_NDK_ROOT`, `ANDROID_HOME` via `CMAKE_ANDROID_NDK`/`CMAKE_ANDROID_SDK`)

### Phase 3: Gradle template project (api/cpp/android/)

Complete, copyable template project with:
- `build.gradle.kts` / `settings.gradle.kts` / `gradle.properties`
- `gradle/wrapper/gradle-wrapper.properties` (Gradle 8.11)
- `app/build.gradle.kts` (compileSdk 35, minSdk 26, arm64-v8a)
- `app/src/main/AndroidManifest.xml` (NativeActivity)
- `app/src/main/cpp/CMakeLists.txt` (FetchContent for Slint)
- `app/src/main/cpp/main.cpp` (example `slint_main()`)
- `app/src/main/cpp/main.slint` (hello world UI)

### Phase 4: Documentation

- New page: `docs/astro/.../android-cpp.mdx` covering prerequisites, project
  setup, writing apps, building, deploying, JNI interop, troubleshooting
- Updated `android.mdx` — removed "only Rust" note, added link to C++ guide
- Updated `general.mdx` — mentions C++ Android support

### Phase 5: Todo example (examples/todo/cpp/android/)

Android build of the existing C++ todo example, tested and produces a working
APK for arm64-v8a.

### Bug fixes discovered during implementation

1. **slint_callbacks.h — `std::apply` incompatibility with NDK r27**:
   NDK r27's libc++ is stricter about `std::apply` with const tuple references.
   Fixed by replacing `std::apply` with a custom `detail::apply` helper that
   properly forwards tuple elements.

2. **cbindgen.rs — missing `Option<Timer>` definition for Android**:
   The `ContextMenu` struct has an Android-only `long_press_timer` field of
   type `Option<Timer>`. cbindgen forward-declared `Option` but never defined
   it, and `Timer` was not in the `cbindgen_private` namespace. Fixed by adding
   a layout-compatible `Option<T>` template and a `using` declaration for Timer.

## Architecture

### Runtime flow

```
Android OS loads libapp.so
  -> android-activity crate glue calls android_main(AndroidApp)   [Rust, in slint-cpp]
    -> initializes AndroidPlatform (Skia renderer, input, clipboard, etc.)
      -> calls extern "C" slint_main()                            [user's C++ code]
        -> normal Slint C++ API: create windows, run event loop
```

### Build flow (Gradle)

```
Gradle (orchestrates)
  -> CMake (NDK toolchain)
    -> Corrosion (cross-compiles slint-cpp Rust crate to Android target)
    -> NDK clang (compiles user's C++ code)
    -> links into libapp.so
  -> packages into APK
```

### Build flow (manual CMake, used for the todo example)

```
cmake -DCMAKE_TOOLCHAIN_FILE=$NDK/build/cmake/android.toolchain.cmake \
      -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 ...
cmake --build .
# Then package with aapt2 + zipalign + apksigner
```

## Design Decisions

| Decision | Rationale |
|---|---|
| `slint_main()` entry point | Platform-neutral; hides Android/Rust complexity; reusable for iOS |
| Gradle + CMake (not xbuild) | Standard C++ Android workflow; no new tools |
| Skia renderer only | Proven on Android; FemtoVG has fontconfig issues |
| `backend-android-activity` (no version suffix) | C++ doesn't expose AndroidApp types |
| Template project (not generator) | Simple, inspectable; same pattern as ESP-IDF |
| Minimum API 26 | Required for InMemoryDexClassLoader; 95%+ of devices |
| Android block before feature definitions | Cache FORCE values must be set before option() reads them |

## Remaining Work

1. **JNI exposure**: Expose `JNIEnv*` and activity `jobject` to C++ users
   for calling Java APIs (sensors, notifications, etc.)

2. **Android lifecycle callbacks**: Expose pause/resume/save-state events
   to C++ (the Rust backend handles them internally today).

3. **GameActivity support**: Currently uses NativeActivity only.

4. **Multi-ABI builds**: Template defaults to arm64-v8a. Supporting multiple
   ABIs multiplies Rust compile time.

5. **CI integration**: Add Android C++ build to CI pipeline.

6. **Strip/compress native libraries**: The APK is ~33MB uncompressed.
   Stripping debug symbols and compressing the .so files would help.
