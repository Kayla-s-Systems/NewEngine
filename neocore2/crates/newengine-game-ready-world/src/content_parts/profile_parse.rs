use super::*;

#[path = "profile_parse/load.rs"]
mod load;
#[path = "profile_parse/projection.rs"]
mod projection;
#[path = "profile_parse/specs.rs"]
mod specs;
#[path = "profile_parse/xml.rs"]
mod xml;

pub(crate) use load::load_authored_world_profile;
pub(super) use specs::{
    RawAudioEmitterSpec, RawAudioSpec, RawDefinitionInstanceSpec, RawFoliageSpec, RawPrefabSpec,
    RawShadowSpec,
};

// Child modules intentionally share only the raw-payload/sanitize namespace through this
// facade. Asset I/O, XML conversion and runtime projection remain separate responsibilities.
