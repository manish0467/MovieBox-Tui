---
trigger: model_decision
description: "Use when the task concerns the separate MovieBox-Android app. This repository only holds the shared engine."
---

# Android app

The Android app, its JNI bridge, Gradle/Compose UI and Media3 playback live in the
separate [MovieBox-Android](https://github.com/manish0467/MovieBox-Android)
repository. Do not add app code or Android build steps to this repository.

The engine contract and the engine-side rules stay in the root `AGENTS.md`; see
`docs/android.md` for the repository pointer.
