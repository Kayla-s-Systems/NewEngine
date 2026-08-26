use std::sync::OnceLock;

#[inline]
pub(super) fn shadow_torture_acceptance_trace_enabled() -> bool {
    crate::env_config::var("NEWENGINE_PROJECT_LAUNCH_PRESET")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("shadow_test"))
        || matches!(
            crate::env_config::var("NEWENGINE_SHADOW_TORTURE_TEST").as_deref(),
            Some("1")
                | Some("true")
                | Some("TRUE")
                | Some("yes")
                | Some("YES")
                | Some("on")
                | Some("ON")
        )
}

pub(super) fn shadow_receiver_debug_mode() -> f32 {
    static MODE: OnceLock<f32> = OnceLock::new();
    *MODE.get_or_init(|| {
        let raw = crate::env_config::var("NEWENGINE_SHADOW_RECEIVER_DEBUG").unwrap_or_default();
        let normalized = raw.trim().to_ascii_lowercase();
        let mode = match normalized.as_str() {
            "" | "0" | "off" | "none" => 0.0,
            "1" | "n" | "normal" | "normal_ws" => 1.0,
            "2" | "ndotl" => 2.0,
            "3" | "shadow" | "shadow_visibility" => 3.0,
            "4" | "cloud" | "cloud_shadow" => 4.0,
            "5" | "direct" => 5.0,
            "6" | "indirect" | "ambient" => 6.0,
            "7" | "instance" | "instance_id" => 7.0,
            "8" | "anomaly" | "composite" => 8.0,
            _ => normalized
                .parse::<f32>()
                .ok()
                .map(|value| value.clamp(0.0, 8.0))
                .unwrap_or(0.0),
        };
        if mode > 0.0 {
            newengine_ulog_api::ulog::warn!(
                "render receiver diagnostic enabled mode={} source=NEWENGINE_SHADOW_RECEIVER_DEBUG",
                mode
            );
        }
        mode
    })
}
