#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri_plugin_sentry::sentry;

#[tauri::command]
fn rust_breadcrumb() {
    sentry::add_breadcrumb(sentry::Breadcrumb {
        message: Some("This is a breadcrumb from Rust".to_owned()),
        ..Default::default()
    })
}

#[tauri::command]
fn rust_panic() {
    panic!("This is a panic from Rust");
}

#[tauri::command]
fn native_crash() {
    unsafe { sadness_generator::raise_segfault() }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sentry::init(tauri_plugin_sentry::Options {
            client: (
                "https://233a45e5efe34c47a3536797ce15dafa@o447951.ingest.sentry.io/5650507",
                tauri_plugin_sentry::ClientOptions {
                    release: sentry::release_name!(),
                    debug: true,
                    ..Default::default()
                },
            ).into(),
            ..Default::default()
        }))
        .invoke_handler(tauri::generate_handler![
            rust_breadcrumb,
            rust_panic,
            native_crash
        ])
        .run(tauri::generate_context!())
        .expect("error while starting tauri app");
}
