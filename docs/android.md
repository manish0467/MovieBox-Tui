# Android app

The native Android app (Jetpack Compose + Media3) lives in its own repository:

**https://github.com/manish0467/MovieBox-Android**

It is GPL-3.0, so it could not live in this MIT/Apache repository while reusing
the interface work of [Flow](https://github.com/Flow-Tube/Flow) (also GPL-3.0).
The Rust core stays here and is consumed by the app as a git dependency on the
`v0.1.28` tag, so both projects ship the same engine.

| | |
| :--- | :--- |
| The app, the JNI bridge and the build steps | [MovieBox-Android](https://github.com/manish0467/MovieBox-Android) |
| The engine and the terminal client | this repository |
| The planned work | [ROADMAP.md](../ROADMAP.md) |

The app covers search, details, sources per resolution, playback with the CDN
headers, subtitles, favourites, watch history, provider switching and downloads.
Live TV and the Stremio addons are on the roadmap.
