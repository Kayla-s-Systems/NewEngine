use super::*;

#[test]
fn app_config_keeps_engine_defaults() {
    let config = PlatformAppConfigV1::default();
    assert_eq!(config.title.as_str(), "NewEngine");
    assert_eq!((config.width, config.height), (1600, 900));
    assert_eq!(config.display.render_scale, 1.0);
    assert!(!config.display.vsync);
}

#[test]
fn service_info_exposes_window_snapshot_method() {
    let info = PlatformServiceInfo::default();
    assert!(info
        .methods
        .iter()
        .any(|method| method == PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1));
}

#[test]
fn default_job_callback_is_null() {
    assert!(PlatformHostJobCallbackV1::default().is_null());
}
