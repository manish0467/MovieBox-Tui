---
trigger: model_decision
description: "Use when building, changing or debugging the Android app: the JNI bridge in crates/moviebox-android, the Gradle/Compose UI in android/, Media3 playback, or cross-compiling the Rust library for Android."
---

# Android app playbook

The full project guide is `@../AGENTS.md` — read it if you have not already. This file
covers only the Android-specific workflow.

## Where things live

| Path | Role |
| :--- | :--- |
| `crates/moviebox-android/src/lib.rs` | JNI entry points (`Java_com_moviebox_app_core_NativeBridge_*`) |
| `crates/moviebox-android/src/api.rs` | Bridge operations + the `{ok,data,error}` JSON envelope |
| `android/app/src/main/kotlin/com/moviebox/app/core/NativeBridge.kt` | Kotlin `external fun` declarations |
| `android/app/src/main/kotlin/com/moviebox/app/core/MovieBoxRepository.kt` | Kotlin side of the envelope |
| `android/app/src/main/kotlin/com/moviebox/app/core/Dtos.kt` | Kotlin mirrors of the Rust models |
| `android/app/src/main/kotlin/com/moviebox/app/player/PlayerActivity.kt` | Media3 / ExoPlayer screen |

## Rules that break the app when violated

- **Change the JNI pair together.** A new bridge function needs a Rust export *and* a
  Kotlin `external fun`. Missing either side → `UnsatisfiedLinkError` at runtime.
  Kotlin side is `@JvmStatic external fun`, so the Rust symbol is
  `Java_com_moviebox_app_core_NativeBridge_<fn>`.
- **Keep the JSON envelope.** Every call returns `{"ok":true,"data":…}` or
  `{"ok":false,"error":"…"}`. Kotlin decodes it in `MovieBoxRepository.kt`.
- **Never drop the stream headers.** `User-Agent`, `Referer` and the CloudFront cookie
  must be set on the ExoPlayer `DefaultHttpDataSource`; without them the CDN answers 403
  and playback fails with "unplayable stream".
- **Do not import `src/tui/` from the bridge.** The bridge depends on the engine with
  `default-features = false`.
- **Do not re-enable the public-DNS fallback on Android.** `src/net.rs` uses the system
  resolver there on purpose.
- **Never commit `android/app/src/main/jniLibs/`** — it is build output.

## Build and verify

```bash
# 1. Cross-compile the Rust library (needs the NDK)
NDK=$ANDROID_HOME/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64
export CC_aarch64_linux_android=$NDK/bin/aarch64-linux-android24-clang
export AR_aarch64_linux_android=$NDK/bin/llvm-ar
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK/bin/aarch64-linux-android24-clang
cargo build -p moviebox-android --target aarch64-linux-android --release
cp target/aarch64-linux-android/release/libmoviebox_android.so \
   android/app/src/main/jniLibs/arm64-v8a/

# 2. Build the APK
cd android && ./gradlew assembleDebug
```

`-PmovieboxBuildRust=true` makes Gradle run cargo-ndk for you.

Reference: `docs/android.md`.
