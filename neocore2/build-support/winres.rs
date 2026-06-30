pub fn compile_windows_app_resources() {
    #[cfg(windows)]
    {
        let manifest_dir =
            std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
        let candidates = [
            manifest_dir.join("assets").join("editor.ico"),
            manifest_dir.join("assets").join("app.ico"),
        ];
        let Some(icon) = candidates.iter().find(|p| p.is_file()) else {
            return;
        };

        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
        let rc_path = out_dir.join("newengine_app_icon.rc");
        let icon_path = icon.to_string_lossy().replace('\\', "\\\\");
        let rc = format!("1 ICON \"{}\"\n", icon_path);
        if std::fs::write(&rc_path, rc).is_ok() {
            embed_resource::compile(rc_path.to_string_lossy().as_ref(), embed_resource::NONE);
        }
    }
}
