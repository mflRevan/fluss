fn main() {
    // Windows: embed the app icon (Explorer, Task Manager, notification settings).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=icon.rc");
        println!("cargo:rerun-if-changed=../../ui/src-tauri/icons/icon.ico");
        embed_resource::compile("icon.rc", embed_resource::NONE).manifest_optional().unwrap();
    }
}
