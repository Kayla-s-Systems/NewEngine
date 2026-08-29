//! Provider-local visual asset grouping for environment assets.
//!
//! This module is the first step away from repeated ad-hoc asset refs in
//! weather/profile tables. It mirrors the authored grouping manifest in
//! `assets/environment/visuals/sky_texture_groups.json`, while keeping runtime
//! evaluation deterministic and cheap. The important rule is that environment
//! profiles choose a visual group; they do not each spell out separate sky,
//! cloud and celestial texture paths.

#[derive(Clone, Copy, Debug)]
pub(crate) struct EnvironmentVisualAssetGroupDescriptor {
    pub id: &'static str,
    pub texture_dictionary_ref: &'static str,
    pub sky_texture_ref: &'static str,
    pub starfield_texture_ref: &'static str,
    pub cloud_density_texture_ref: &'static str,
    pub cloud_detail_texture_ref: &'static str,
    pub cloud_dither_texture_ref: &'static str,
    pub sun_disk_texture_ref: &'static str,
    pub moon_disk_texture_ref: &'static str,
}

pub(crate) const SKYDOME_TEXTURE_DICTIONARY_REF: &str = "textures/environment/skydome.ytd";
pub(crate) const SKYDOME_STARFIELD_REF: &str = "textures/environment/skydome.ytd@starfield";
pub(crate) const SKYDOME_CLOUD_DENSITY_REF: &str =
    "textures/environment/sky_clouds_v2.ytd@cloud_base_shape";
pub(crate) const SKYDOME_CLOUD_DETAIL_REF: &str =
    "textures/environment/sky_clouds_v2.ytd@cloud_detail_erosion";
pub(crate) const SKYDOME_DITHER_REF: &str = "textures/environment/skydome.ytd@dither";
pub(crate) const SKYDOME_MOON_DISK_REF: &str = "textures/environment/skydome.ytd@moon_new";

// There is no authored `sun_disk` entry in the current YTD inventory. Keep the
// sun as an explicit procedural visual until a real `.ytd@sun_disk` entry is
// authored, instead of emitting a fake/missing `textures/sky/celestial.ytd` ref.
pub(crate) const PROCEDURAL_SUN_DISK_REF: &str =
    "procedural://engine.world.environment/celestial/sun_disk";

pub(crate) const GAME_READY_SKYDOME_VISUALS: EnvironmentVisualAssetGroupDescriptor =
    EnvironmentVisualAssetGroupDescriptor {
        id: "environment.visuals.game_ready_skydome.v1",
        texture_dictionary_ref: SKYDOME_TEXTURE_DICTIONARY_REF,
        sky_texture_ref: SKYDOME_STARFIELD_REF,
        starfield_texture_ref: SKYDOME_STARFIELD_REF,
        cloud_density_texture_ref: SKYDOME_CLOUD_DENSITY_REF,
        cloud_detail_texture_ref: SKYDOME_CLOUD_DETAIL_REF,
        cloud_dither_texture_ref: SKYDOME_DITHER_REF,
        sun_disk_texture_ref: PROCEDURAL_SUN_DISK_REF,
        moon_disk_texture_ref: SKYDOME_MOON_DISK_REF,
    };

#[allow(dead_code)]
pub(crate) const VISUAL_GROUPS: &[EnvironmentVisualAssetGroupDescriptor] =
    &[GAME_READY_SKYDOME_VISUALS];

#[allow(dead_code)]
pub(crate) fn visual_group_by_id(id: &str) -> &'static EnvironmentVisualAssetGroupDescriptor {
    VISUAL_GROUPS
        .iter()
        .find(|group| group.id == id)
        .unwrap_or(&GAME_READY_SKYDOME_VISUALS)
}
