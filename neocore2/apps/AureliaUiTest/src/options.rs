use newengine_runtime_host::app_launcher::RuntimeHostBootOption;

pub const FIXED_DT_MS: u32 = 16;
pub const APP_DIR_NAME: &str = "AureliaUiTest";
pub const APP_ASSETS_ENV: &str = "NEWENGINE_AURELIA_UI_TEST_ASSETS_DIR";
pub const SURFACE_ID: &str = "apps.aurelia_ui_test.main";

/// AureliaUiTest is a minimal runtime UI app.
///
/// It needs platform, render, UI and plugin loading, but it does not need the
/// core pre-start config editor/window. Omitting `PreStartConfigWindow` is the
/// explicit declaration that prevents this app from showing the engine config
/// boot surface before it reaches Aurelia.
pub const BOOT_OPTIONS: &[RuntimeHostBootOption] = &[
    RuntimeHostBootOption::RuntimePlugins,
    RuntimeHostBootOption::PlatformWindow,
    RuntimeHostBootOption::RenderBackend,
    RuntimeHostBootOption::UiBackend,
];

pub const ENV_DEFAULTS: &[(&str, &str)] = &[
    ("NEWENGINE_PLUGIN_TARGET", "runtime"),
    ("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred"),
    ("NEWENGINE_SHADER_ASYNC_PREBAKED_UNTIL_READY", "1"),
];
