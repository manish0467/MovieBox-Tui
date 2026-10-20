//! JNI bridge between the Android app (Kotlin / Jetpack Compose) and the
//! MovieBox core engine.
//!
//! The bridge is intentionally thin: every call is a synchronous JNI entry
//! point that blocks on the shared tokio runtime and returns a JSON envelope
//! (`{"ok":true,"data":...}` / `{"ok":false,"error":"..."}`) that the Kotlin
//! side decodes with kotlinx.serialization.
//!
//! Build (after `rustup target add aarch64-linux-android` and installing the
//! Android NDK):
//!
//! ```text
//! cargo ndk -p moviebox-android -t arm64-v8a -o ./android/app/src/main/jniLibs build --release
//! ```

mod api;
mod downloads;

use std::sync::OnceLock;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;

use api::Bridge;

static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// Returns the shared bridge, creating it on first use.
fn bridge() -> &'static Bridge {
    BRIDGE.get_or_init(Bridge::new)
}

fn read_string(env: &mut JNIEnv<'_>, value: &JString<'_>) -> String {
    env.get_string(value)
        .map(|s| s.into())
        .unwrap_or_else(|_| String::new())
}

fn return_json(env: &mut JNIEnv<'_>, payload: String) -> jstring {
    match env.new_string(payload) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn error_json(env: &mut JNIEnv<'_>, message: &str) -> jstring {
    return_json(env, api::error_envelope(message))
}

// ---------------------------------------------------------------- lifecycle

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeInit<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    config_dir: JString<'local>,
    data_dir: JString<'local>,
    cache_dir: JString<'local>,
) -> jstring {
    let config_dir = read_string(&mut env, &config_dir);
    let data_dir = read_string(&mut env, &data_dir);
    let cache_dir = read_string(&mut env, &cache_dir);

    match api::init_directories(&config_dir, &data_dir, &cache_dir) {
        Ok(()) => return_json(
            &mut env,
            api::ok_envelope(&serde_json::json!({ "ready": true })),
        ),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeProviders<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let payload = bridge().providers();
    return_json(&mut env, payload)
}

// ------------------------------------------------------------------ content

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeSearch<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    provider: JString<'local>,
    query: JString<'local>,
    page: jni::sys::jint,
) -> jstring {
    let provider = read_string(&mut env, &provider);
    let query = read_string(&mut env, &query);
    let page = page.max(1) as usize;

    match bridge().search(&provider, &query, page) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeSuggest<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    query: JString<'local>,
) -> jstring {
    let query = read_string(&mut env, &query);
    match bridge().suggest(&query) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeHomepage<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    tab_id: JString<'local>,
    page: jni::sys::jint,
) -> jstring {
    let tab_id = read_string(&mut env, &tab_id);
    let page = page.max(1) as usize;

    match bridge().homepage(&tab_id, page) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeDetails<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    provider: JString<'local>,
    subject_id: JString<'local>,
) -> jstring {
    let provider = read_string(&mut env, &provider);
    let subject_id = read_string(&mut env, &subject_id);

    match bridge().details(&provider, &subject_id) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeEpisodeStreams<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    provider: JString<'local>,
    subject_id: JString<'local>,
    season: jni::sys::jint,
    episode: jni::sys::jint,
) -> jstring {
    let provider = read_string(&mut env, &provider);
    let subject_id = read_string(&mut env, &subject_id);
    let season = season.max(0) as usize;
    let episode = episode.max(0) as usize;

    match bridge().episode_streams(&provider, &subject_id, season, episode) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeResolvePlayback<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    provider: JString<'local>,
    release_json: JString<'local>,
) -> jstring {
    let provider = read_string(&mut env, &provider);
    let release_json = read_string(&mut env, &release_json);

    match bridge().resolve_playback(&provider, &release_json) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeSubtitles<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    subject_id: JString<'local>,
    resource_id: JString<'local>,
    sibling_ids_json: JString<'local>,
    season: jni::sys::jint,
    episode: jni::sys::jint,
) -> jstring {
    let subject_id = read_string(&mut env, &subject_id);
    let resource_id = read_string(&mut env, &resource_id);
    let sibling_ids_json = read_string(&mut env, &sibling_ids_json);
    let season = season.max(0) as usize;
    let episode = episode.max(0) as usize;

    match bridge().subtitles(
        &subject_id,
        &resource_id,
        &sibling_ids_json,
        season,
        episode,
    ) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

// ----------------------------------------------------------------- library

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeFavorites<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    match bridge().favorites() {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeToggleFavorite<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    item_json: JString<'local>,
) -> jstring {
    let item_json = read_string(&mut env, &item_json);
    match bridge().toggle_favorite(&item_json) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeHistory<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    match bridge().history() {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeRecordWatchProgress<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    item_json: JString<'local>,
) -> jstring {
    let item_json = read_string(&mut env, &item_json);
    match bridge().record_watch_progress(&item_json) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeDownloadStart<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    request_json: JString<'local>,
) -> jstring {
    let request_json = read_string(&mut env, &request_json);
    match bridge().start_download(&request_json) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeDownloadList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    match bridge().list_downloads() {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeDownloadCancel<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    job_id: JString<'local>,
) -> jstring {
    let job_id = read_string(&mut env, &job_id);
    match bridge().cancel_download(&job_id) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeDownloadRemove<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    job_id: JString<'local>,
) -> jstring {
    let job_id = read_string(&mut env, &job_id);
    match bridge().remove_download(&job_id) {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativeDownloadDirectory<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    match bridge().download_directory() {
        Ok(value) => return_json(&mut env, api::ok_envelope(&value)),
        Err(err) => error_json(&mut env, &err),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_moviebox_app_core_NativeBridge_nativePosterUrlHeaders<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    url: JString<'local>,
) -> jstring {
    let url = read_string(&mut env, &url);
    let payload = api::poster_headers(&url, bridge().user_agent());
    return_json(&mut env, payload)
}
