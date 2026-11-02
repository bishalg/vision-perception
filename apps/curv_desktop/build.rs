fn main() {
    // Declaring the app command in the manifest puts it under the ACL: without the
    // generated `allow-set-server-url` permission in a capability, no webview may call
    // it. Only `capabilities/setup.json` grants it, and only to the local setup window.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["set_server_url"])),
    )
    .expect("failed to run tauri-build");
}
