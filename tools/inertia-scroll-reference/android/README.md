# Android Inertia Reference

This local-only project records Android `OverScroller` inertia for the same medium-flick probe used by the Slint example.

```sh
cd tools/inertia-scroll-reference/android
gradle assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb logcat -c
adb shell monkey -p dev.slint.inertiareference 1
sleep 5
adb logcat -d | grep -E 'inertia-reference|source,gesture|android-over-scroller,'
```

Trace rows use:

```text
source,gesture,frame,time_ms,y_px,velocity_px_s,phase
```
