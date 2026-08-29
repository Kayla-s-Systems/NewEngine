#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::RenderHardwareTier;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Declarative render-runtime profile loaded through the host plugin config service.
///
/// The reusable engine runtime must not hard-code GPU model names, environment toggles
/// or game-specific fallbacks. Profiles describe degradable capability tiers; providers
/// and feature packs decide what to register. This is the small engine-side contract
/// that lets a host/application select a safer first playable path without recompiling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RenderRuntimeProfile {
    #[serde(default = "default_profile_id")]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) gpu_safe: bool,
    #[serde(default)]
    pub(crate) graphics: GraphicsProfile,
    #[serde(default)]
    pub(crate) world: WorldRuntimeProfile,
    #[serde(default)]
    pub(crate) input: GameplayInputProfile,
    #[serde(default)]
    pub(crate) ui: UiRestoreProfile,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct GraphicsProfile {
    #[serde(default = "default_clear_color")]
    pub(crate) clear_color: [f32; 4],
    #[serde(default)]
    pub(crate) sky: SkyPassProfile,
    #[serde(default)]
    pub(crate) shadows: FeatureSwitch,
    #[serde(default)]
    pub(crate) hdr_scene: FeatureSwitch,
    #[serde(default)]
    pub(crate) postfx: FeatureSwitch,
    #[serde(default)]
    pub(crate) deferred: FeatureSwitch,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct SkyPassProfile {
    /// `native` keeps authored sky visuals, `clear_gradient` and `disabled` quarantine
    /// skydome primitives so broken fallback materials cannot fill the scene.
    #[serde(default = "default_sky_mode")]
    pub(crate) mode: SkyPassMode,
    #[serde(default)]
    pub(crate) tick_cycle: FeatureSwitch,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SkyPassMode {
    Native,
    ClearGradient,
    Disabled,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct WorldRuntimeProfile {
    #[serde(default)]
    pub(crate) runtime_terrain_streaming: FeatureSwitch,
    #[serde(default)]
    pub(crate) service_physics: FeatureSwitch,
    #[serde(default = "default_true")]
    pub(crate) fallback_ecs_physics: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct GameplayInputProfile {
    #[serde(default = "default_true")]
    pub(crate) capture_cursor_on_play: bool,
    #[serde(default = "default_true")]
    pub(crate) force_gameplay_look: bool,
    #[serde(default = "default_true")]
    pub(crate) force_gameplay_actions: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct UiRestoreProfile {
    #[serde(default = "default_true")]
    pub(crate) restore_viewport_pass_on_close: bool,
    #[serde(default = "default_true")]
    pub(crate) restore_gameplay_input_on_close: bool,
    #[serde(default = "default_true")]
    pub(crate) invalidate_shadow_cache_on_close: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FeatureSwitch {
    Enabled,
    Disabled,
}

impl Default for FeatureSwitch {
    #[inline]
    fn default() -> Self {
        Self::Enabled
    }
}

impl FeatureSwitch {
    #[inline]
    pub(crate) const fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

impl Default for RenderRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self {
            id: default_profile_id(),
            gpu_safe: false,
            graphics: GraphicsProfile::default(),
            world: WorldRuntimeProfile::default(),
            input: GameplayInputProfile::default(),
            ui: UiRestoreProfile::default(),
        }
    }
}

impl Default for GraphicsProfile {
    #[inline]
    fn default() -> Self {
        Self {
            clear_color: default_clear_color(),
            sky: SkyPassProfile::default(),
            shadows: FeatureSwitch::Enabled,
            hdr_scene: FeatureSwitch::Enabled,
            postfx: FeatureSwitch::Enabled,
            deferred: FeatureSwitch::Disabled,
        }
    }
}

impl Default for SkyPassProfile {
    #[inline]
    fn default() -> Self {
        Self {
            mode: SkyPassMode::Native,
            tick_cycle: FeatureSwitch::Enabled,
        }
    }
}

impl Default for WorldRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self {
            runtime_terrain_streaming: FeatureSwitch::Enabled,
            service_physics: FeatureSwitch::Enabled,
            fallback_ecs_physics: false,
        }
    }
}

impl Default for GameplayInputProfile {
    #[inline]
    fn default() -> Self {
        Self {
            capture_cursor_on_play: true,
            force_gameplay_look: true,
            force_gameplay_actions: true,
        }
    }
}

impl Default for UiRestoreProfile {
    #[inline]
    fn default() -> Self {
        Self {
            restore_viewport_pass_on_close: true,
            restore_gameplay_input_on_close: true,
            invalidate_shadow_cache_on_close: true,
        }
    }
}

impl RenderRuntimeProfile {
    pub(crate) fn load() -> Self {
        let raw = newengine_plugin_host::get_plugin_overrides_with_env("engine.runtime");
        let Some(render_value) = raw.get("render") else {
            return Self::default();
        };
        let candidate = render_value
            .get("runtime_profile")
            .or_else(|| render_value.get("profile"))
            .cloned()
            .unwrap_or_else(|| render_value.clone());
        match serde_json::from_value::<Self>(candidate) {
            Ok(profile) => {
                newengine_ulog_api::ulog::info!(
                    "render runtime profile: loaded id='{}' gpu_safe={} sky={:?} service_physics={} terrain_streaming={} shadows={} hdr={} postfx={} deferred={}",
                    profile.id,
                    profile.gpu_safe,
                    profile.graphics.sky.mode,
                    profile.world.service_physics.enabled(),
                    profile.world.runtime_terrain_streaming.enabled(),
                    profile.graphics.shadows.enabled(),
                    profile.graphics.hdr_scene.enabled(),
                    profile.graphics.postfx.enabled(),
                    profile.graphics.deferred.enabled(),
                );
                profile
            }
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "render runtime profile: failed to decode config; using defaults err='{}' raw={}",
                    e,
                    compact_json(&raw),
                );
                Self::default()
            }
        }
    }

    #[inline]
    pub(crate) fn gpu_safe_enabled(&self) -> bool {
        self.gpu_safe
    }

    #[inline]
    pub(crate) fn accepts_hardware_tier_resolution(&self) -> bool {
        self.id == default_profile_id()
    }

    #[inline]
    pub(crate) fn draw_sky_visuals(&self) -> bool {
        matches!(self.graphics.sky.mode, SkyPassMode::Native)
    }

    #[inline]
    pub(crate) fn tick_sky_cycle(&self) -> bool {
        self.graphics.sky.tick_cycle.enabled()
    }

    #[inline]
    pub(crate) fn use_service_physics(&self) -> bool {
        self.world.service_physics.enabled()
    }

    #[inline]
    pub(crate) fn use_runtime_terrain_streaming(&self) -> bool {
        self.world.runtime_terrain_streaming.enabled()
    }

    #[inline]
    pub(crate) fn use_fallback_ecs_physics(&self) -> bool {
        !self.use_service_physics() && self.world.fallback_ecs_physics
    }

    #[inline]
    pub(crate) fn shadows_enabled(&self) -> bool {
        self.graphics.shadows.enabled()
            && newengine_core::startup_launch_settings()
                .graphics
                .shadows_enabled
    }

    #[inline]
    pub(crate) fn hdr_scene_enabled(&self) -> bool {
        self.graphics.hdr_scene.enabled()
    }

    #[inline]
    pub(crate) fn postfx_enabled(&self) -> bool {
        self.graphics.postfx.enabled()
    }

    #[inline]
    pub(crate) fn deferred_enabled(&self) -> bool {
        self.graphics.deferred.enabled()
    }

    #[inline]
    pub(crate) fn configured_clear_color(&self) -> [f32; 4] {
        self.graphics.clear_color
    }

    pub(crate) fn apply_hardware_tier(&mut self, tier: RenderHardwareTier) {
        match tier {
            RenderHardwareTier::LegacyGtx => {
                self.id = tier.profile_id().to_owned();
                self.gpu_safe = true;
                self.graphics.shadows = FeatureSwitch::Disabled;
                self.graphics.hdr_scene = FeatureSwitch::Disabled;
                self.graphics.postfx = FeatureSwitch::Disabled;
                self.graphics.deferred = FeatureSwitch::Disabled;
                self.world.runtime_terrain_streaming = FeatureSwitch::Disabled;
            }
            RenderHardwareTier::Gtx => {
                self.id = tier.profile_id().to_owned();
                // Production safety policy for the GTX auto tier: keep the proven
                // forward + shadow path while async PostFX recovery is exercised by
                // explicit test profiles. Authored non-auto profiles are not rewritten.
                self.gpu_safe = false;
                self.graphics.shadows = FeatureSwitch::Enabled;
                self.graphics.hdr_scene = FeatureSwitch::Disabled;
                self.graphics.postfx = FeatureSwitch::Disabled;
                self.graphics.deferred = FeatureSwitch::Disabled;
            }
            RenderHardwareTier::Rtx => {
                self.id = tier.profile_id().to_owned();
                self.gpu_safe = false;
            }
            RenderHardwareTier::Headless | RenderHardwareTier::Unknown => {}
        }
    }
}

#[inline]
fn default_profile_id() -> String {
    "newengine.render.runtime.tier.auto".to_owned()
}

#[inline]
const fn default_true() -> bool {
    true
}

#[inline]
const fn default_clear_color() -> [f32; 4] {
    [0.020, 0.025, 0.035, 1.0]
}

#[inline]
const fn default_sky_mode() -> SkyPassMode {
    SkyPassMode::Native
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unprintable>".to_owned())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_gtx_tier_uses_stable_forward_shadow_profile() {
        let mut profile = RenderRuntimeProfile::default();
        profile.apply_hardware_tier(RenderHardwareTier::Gtx);
        assert_eq!(profile.id, RenderHardwareTier::Gtx.profile_id());
        assert!(profile.graphics.shadows.enabled());
        assert!(!profile.graphics.hdr_scene.enabled());
        assert!(!profile.graphics.postfx.enabled());
        assert!(!profile.graphics.deferred.enabled());
    }

    #[test]
    fn explicit_profiles_are_identifiable_before_hardware_tier_application() {
        let mut profile = RenderRuntimeProfile::default();
        assert!(profile.accepts_hardware_tier_resolution());
        profile.id = "newengine.render.runtime.custom".to_owned();
        assert!(!profile.accepts_hardware_tier_resolution());
    }
}
