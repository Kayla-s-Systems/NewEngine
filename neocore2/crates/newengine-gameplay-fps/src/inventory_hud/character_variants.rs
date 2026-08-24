use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{PlayerModelAssignment, PlayerModelBinding};

pub const CHARACTER_SELECT_ACTION_PREFIX: &str = "game.character.select.";
pub const LEGACY_ABBY_LATEST_ACTION: &str = "game.character.select.abby_latest";
pub const LEGACY_ABIGAIL_ACTION: &str = "game.character.select.abigail";
pub const LEGACY_ABBY_HYBRID_ACTION: &str = "game.character.select.abby_santa_barbara_hybrid_709";
pub const LEGACY_ABBY_SEATTLE_SOURCE_ACTION: &str = "game.character.select.abby_seattle_source_709";
pub const LEGACY_ABBY_SANTA_BARBARA_SOURCE_ACTION: &str =
    "game.character.select.abby_santa_barbara_source_709";

// Compatibility IDs retained so saved selections/actions survive the native-rig cutover.
// They no longer describe the runtime rig: all four Abby variants use the 1033-joint native skeleton.
pub const ABBY_WLF_DEFAULT_709_ID: &str = "abby_wlf_default_709";
pub const ABBY_SEATTLE_709_ID: &str = "abby_seattle_709";
pub const ABBY_SANTA_BARBARA_709_ID: &str = "abby_santa_barbara_709";
pub const ABBY_SEATTLE_DINA_TANK_709_ID: &str = "abby_seattle_dina_tank_709";
pub const ABIGAIL_LEGACY_ID: &str = "abigail_legacy";

pub const LEGACY_ABBY_HYBRID_709_ID: &str = "abby_santa_barbara_hybrid_709";
pub const LEGACY_ABBY_SEATTLE_SOURCE_709_ID: &str = "abby_seattle_source_709";
pub const LEGACY_ABBY_SANTA_BARBARA_SOURCE_709_ID: &str = "abby_santa_barbara_source_709";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayableCharacterFamily {
    Abby,
    Abigail,
}

impl PlayableCharacterFamily {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Abby => "Abby",
            Self::Abigail => "Abigail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayableCharacterVariantAvailability {
    RuntimeReady,
    #[allow(dead_code)]
    AuthoringSource,
}

impl PlayableCharacterVariantAvailability {
    pub const fn is_runtime_ready(self) -> bool {
        matches!(self, Self::RuntimeReady)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeReady => "Runtime ready",
            Self::AuthoringSource => "Authoring source",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlayableCharacterVariantDescriptor {
    pub id: &'static str,
    pub family: PlayableCharacterFamily,
    pub display_name: &'static str,
    pub subtitle: &'static str,
    pub availability: PlayableCharacterVariantAvailability,
    pub rig_label: &'static str,
    pub source_provenance: &'static str,
    pub runtime_model_ref: Option<&'static str>,
    pub properties_ref: Option<&'static str>,
    pub texture_dictionary: Option<&'static str>,
    pub skeleton_ref: Option<&'static str>,
    pub animations: [Option<&'static str>; 6],
    pub target_height: f32,
    pub yaw_offset: f32,
}

impl PlayableCharacterVariantDescriptor {
    pub fn assignment(self) -> Option<PlayerModelAssignment> {
        if !self.availability.is_runtime_ready() {
            return None;
        }
        let [idle, walk, run, sprint, jump, fall] = self.animations;
        Some(PlayerModelAssignment {
            enabled: true,
            source: self.runtime_model_ref?.to_owned(),
            properties_ref: self.properties_ref.map(ToOwned::to_owned),
            texture_dictionary: self.texture_dictionary.map(ToOwned::to_owned),
            skeleton_source: self.skeleton_ref.map(ToOwned::to_owned),
            idle_animation: idle.map(ToOwned::to_owned),
            walk_animation: walk.map(ToOwned::to_owned),
            run_animation: run.map(ToOwned::to_owned),
            sprint_animation: sprint.map(ToOwned::to_owned),
            jump_animation: jump.map(ToOwned::to_owned),
            fall_animation: fall.map(ToOwned::to_owned),
            target_height: self.target_height,
            yaw_offset: self.yaw_offset,
            hide_in_first_person: true,
            ..PlayerModelAssignment::default()
        })
    }
}

const ABBY_WLF_MODEL: &str = "models/characters/abby/abby.ydd@abby";
const ABBY_SEATTLE_MODEL: &str = "models/characters/abby/variants/abby_seattle.ydd@abby_seattle";
const ABBY_SANTA_BARBARA_MODEL: &str =
    "models/characters/abby/variants/abby_santa_barbara.ydd@abby_santa_barbara";
const ABBY_SEATTLE_DINA_TANK_MODEL: &str =
    "models/characters/abby/variants/abby_seattle_dina_tank.ydd@abby_seattle_dina_tank";
const ABBY_PROPERTIES: &str = "definitions/fps/player_abby.ytyp@player_abby";
const ABBY_TEXTURES: &str = "textures/characters/abby.ytd";
const ABBY_SKELETON: &str = "models/characters/abby/abby.ymt@abby";
const ABBY_ANIMS: [Option<&str>; 6] = [
    Some("animations/characters/abby/mm-explore.ycd@abby-mm-explore-idle"),
    Some("animations/characters/abby/mm-explore.ycd@abby-mm-explore-walk-loop-fw"),
    Some("animations/characters/abby/mm-explore.ycd@abby-mm-explore-run-loop-fw"),
    Some("animations/characters/abby/mm-explore.ycd@abby-mm-explore-sprint-loop-fw"),
    None,
    None,
];

pub const PLAYABLE_CHARACTER_VARIANTS: &[PlayableCharacterVariantDescriptor] = &[
    PlayableCharacterVariantDescriptor {
        id: ABBY_WLF_DEFAULT_709_ID,
        family: PlayableCharacterFamily::Abby,
        display_name: "Abby — WLF / Default Jacket",
        subtitle: "Native TLOU II PAK geometry, native braid skin and 1033-joint JOINT_HIERARCHY with OrbisAnim locomotion",
        availability: PlayableCharacterVariantAvailability::RuntimeReady,
        rig_label: "North Star JOINT_HIERARCHY / 1033 joints",
        source_provenance: "northstar.tlou2.pc GEOMETRY_1 + JOINT_HIERARCHY + native braid",
        runtime_model_ref: Some(ABBY_WLF_MODEL),
        properties_ref: Some(ABBY_PROPERTIES),
        texture_dictionary: Some(ABBY_TEXTURES),
        skeleton_ref: Some(ABBY_SKELETON),
        animations: ABBY_ANIMS,
        target_height: 1.78,
        yaw_offset: core::f32::consts::PI,
    },
    PlayableCharacterVariantDescriptor {
        id: ABBY_SEATTLE_709_ID,
        family: PlayableCharacterFamily::Abby,
        display_name: "Abby — Seattle",
        subtitle: "Seattle outfit geometry remapped into the native 1033-joint Abby skeleton with native OrbisAnim locomotion and braid chain",
        availability: PlayableCharacterVariantAvailability::RuntimeReady,
        rig_label: "North Star JOINT_HIERARCHY / 1033 joints",
        source_provenance:
            "pak head/hair + gltf2 outfit/body geometry remapped 709->native1033 + native braid joints",
        runtime_model_ref: Some(ABBY_SEATTLE_MODEL),
        properties_ref: Some(ABBY_PROPERTIES),
        texture_dictionary: Some(ABBY_TEXTURES),
        skeleton_ref: Some(ABBY_SKELETON),
        animations: ABBY_ANIMS,
        target_height: 1.78,
        yaw_offset: core::f32::consts::PI,
    },
    PlayableCharacterVariantDescriptor {
        id: ABBY_SANTA_BARBARA_709_ID,
        family: PlayableCharacterFamily::Abby,
        display_name: "Abby — Santa Barbara",
        subtitle: "Santa Barbara outfit geometry remapped into the native 1033-joint Abby skeleton with native OrbisAnim locomotion and braid chain",
        availability: PlayableCharacterVariantAvailability::RuntimeReady,
        rig_label: "North Star JOINT_HIERARCHY / 1033 joints",
        source_provenance:
            "pak head/hair + gltf2 Santa Barbara geometry remapped 709->native1033 + native braid joints",
        runtime_model_ref: Some(ABBY_SANTA_BARBARA_MODEL),
        properties_ref: Some(ABBY_PROPERTIES),
        texture_dictionary: Some(ABBY_TEXTURES),
        skeleton_ref: Some(ABBY_SKELETON),
        animations: ABBY_ANIMS,
        target_height: 1.78,
        yaw_offset: core::f32::consts::PI,
    },
    PlayableCharacterVariantDescriptor {
        id: ABBY_SEATTLE_DINA_TANK_709_ID,
        family: PlayableCharacterFamily::Abby,
        display_name: "Abby — Seattle / Alternate Tank",
        subtitle: "Alternate Seattle tank outfit remapped into the native 1033-joint Abby skeleton with native OrbisAnim locomotion and braid chain",
        availability: PlayableCharacterVariantAvailability::RuntimeReady,
        rig_label: "North Star JOINT_HIERARCHY / 1033 joints",
        source_provenance:
            "pak head/hair + gltf2 alternate tank geometry remapped 709->native1033 + native braid joints",
        runtime_model_ref: Some(ABBY_SEATTLE_DINA_TANK_MODEL),
        properties_ref: Some(ABBY_PROPERTIES),
        texture_dictionary: Some(ABBY_TEXTURES),
        skeleton_ref: Some(ABBY_SKELETON),
        animations: ABBY_ANIMS,
        target_height: 1.78,
        yaw_offset: core::f32::consts::PI,
    },
    PlayableCharacterVariantDescriptor {
        id: ABIGAIL_LEGACY_ID,
        family: PlayableCharacterFamily::Abigail,
        display_name: "Abigail — Legacy/Test Avatar",
        subtitle: "Legacy alternate player used for import and character-swap regression coverage",
        availability: PlayableCharacterVariantAvailability::RuntimeReady,
        rig_label: "RAGE/OpenFormats legacy skeleton",
        source_provenance: "rage.openformats.csb_abigail",
        runtime_model_ref: Some("models/characters/abigail/csb_abigail.ydd@csb_abigail"),
        properties_ref: Some("definitions/fps/player_abigail.ytyp@player_abigail"),
        texture_dictionary: Some("textures/characters/abigail.ytd"),
        skeleton_ref: Some("skeletons/characters/abigail/csb_abigail.ymt@csb_abigail"),
        animations: [None; 6],
        target_height: 1.78,
        yaw_offset: 0.0,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayableCharacterSelection {
    pub variant_id: String,
}

pub fn variant_by_id(id: &str) -> Option<&'static PlayableCharacterVariantDescriptor> {
    let id = id.trim();
    let canonical = if id.eq_ignore_ascii_case(LEGACY_ABBY_HYBRID_709_ID) {
        ABBY_WLF_DEFAULT_709_ID
    } else if id.eq_ignore_ascii_case(LEGACY_ABBY_SEATTLE_SOURCE_709_ID) {
        ABBY_SEATTLE_709_ID
    } else if id.eq_ignore_ascii_case(LEGACY_ABBY_SANTA_BARBARA_SOURCE_709_ID) {
        ABBY_SANTA_BARBARA_709_ID
    } else {
        id
    };
    PLAYABLE_CHARACTER_VARIANTS
        .iter()
        .find(|variant| variant.id.eq_ignore_ascii_case(canonical))
}

pub fn variant_from_action(action_id: &str) -> Option<&'static PlayableCharacterVariantDescriptor> {
    match action_id {
        LEGACY_ABBY_LATEST_ACTION | LEGACY_ABBY_HYBRID_ACTION => {
            return variant_by_id(ABBY_WLF_DEFAULT_709_ID)
        }
        LEGACY_ABBY_SEATTLE_SOURCE_ACTION => return variant_by_id(ABBY_SEATTLE_709_ID),
        LEGACY_ABBY_SANTA_BARBARA_SOURCE_ACTION => return variant_by_id(ABBY_SANTA_BARBARA_709_ID),
        LEGACY_ABIGAIL_ACTION => return variant_by_id(ABIGAIL_LEGACY_ID),
        _ => {}
    }
    variant_by_id(action_id.strip_prefix(CHARACTER_SELECT_ACTION_PREFIX)?)
}

pub fn selected_variant(
    world: &World,
    player: EntityId,
) -> Option<&'static PlayableCharacterVariantDescriptor> {
    if let Some(selection) = world.get::<PlayableCharacterSelection>(player) {
        if let Some(variant) = variant_by_id(&selection.variant_id) {
            return Some(variant);
        }
    }
    let binding = world.get::<PlayerModelBinding>(player)?;
    PLAYABLE_CHARACTER_VARIANTS.iter().find(|variant| {
        variant.availability.is_runtime_ready()
            && variant
                .runtime_model_ref
                .is_some_and(|source| source.eq_ignore_ascii_case(binding.source.trim()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn character_variant_ids_and_actions_are_unique() {
        let mut ids = BTreeSet::new();
        let mut actions = BTreeSet::new();
        for variant in PLAYABLE_CHARACTER_VARIANTS {
            assert!(ids.insert(variant.id));
            assert!(actions.insert(format!("{CHARACTER_SELECT_ACTION_PREFIX}{}", variant.id)));
        }
    }

    #[test]
    fn all_four_abby_skins_are_runtime_ready() {
        let variants = PLAYABLE_CHARACTER_VARIANTS
            .iter()
            .filter(|variant| variant.family == PlayableCharacterFamily::Abby)
            .collect::<Vec<_>>();
        assert_eq!(variants.len(), 4);
        assert!(variants
            .iter()
            .all(|variant| variant.availability.is_runtime_ready()));
    }

    #[test]
    fn runtime_abby_variants_keep_abby_rig_and_animation_family() {
        for variant in PLAYABLE_CHARACTER_VARIANTS.iter().filter(|variant| {
            variant.family == PlayableCharacterFamily::Abby
                && variant.availability.is_runtime_ready()
        }) {
            let assignment = variant.assignment().expect("runtime-ready assignment");
            assert!(assignment.source.contains("/characters/abby/"));
            assert!(assignment
                .skeleton_source
                .as_deref()
                .is_some_and(|value| value.contains("/characters/abby/")));
            for clip in [
                assignment.idle_animation.as_deref(),
                assignment.walk_animation.as_deref(),
                assignment.run_animation.as_deref(),
                assignment.sprint_animation.as_deref(),
            ] {
                assert!(clip.is_some_and(|value| value.contains("/characters/abby/")));
            }
            assert!(assignment.jump_animation.is_none());
            assert!(assignment.fall_animation.is_none());
        }
    }

    #[test]
    fn runtime_abby_skins_have_distinct_model_refs() {
        let mut refs = BTreeSet::new();
        for variant in PLAYABLE_CHARACTER_VARIANTS
            .iter()
            .filter(|variant| variant.family == PlayableCharacterFamily::Abby)
        {
            assert!(refs.insert(variant.runtime_model_ref.expect("runtime Abby model")));
        }
        assert_eq!(refs.len(), 4);
    }

    #[test]
    fn legacy_abby_ids_and_actions_map_to_canonical_skins() {
        assert_eq!(
            variant_by_id(LEGACY_ABBY_HYBRID_709_ID).unwrap().id,
            ABBY_WLF_DEFAULT_709_ID
        );
        assert_eq!(
            variant_by_id(LEGACY_ABBY_SEATTLE_SOURCE_709_ID).unwrap().id,
            ABBY_SEATTLE_709_ID
        );
        assert_eq!(
            variant_by_id(LEGACY_ABBY_SANTA_BARBARA_SOURCE_709_ID)
                .unwrap()
                .id,
            ABBY_SANTA_BARBARA_709_ID
        );
        assert_eq!(
            variant_from_action(LEGACY_ABBY_LATEST_ACTION).unwrap().id,
            ABBY_WLF_DEFAULT_709_ID
        );
        assert_eq!(
            variant_from_action(LEGACY_ABBY_HYBRID_ACTION).unwrap().id,
            ABBY_WLF_DEFAULT_709_ID
        );
        assert_eq!(
            variant_from_action(LEGACY_ABBY_SEATTLE_SOURCE_ACTION)
                .unwrap()
                .id,
            ABBY_SEATTLE_709_ID
        );
        assert_eq!(
            variant_from_action(LEGACY_ABBY_SANTA_BARBARA_SOURCE_ACTION)
                .unwrap()
                .id,
            ABBY_SANTA_BARBARA_709_ID
        );
        assert_eq!(
            variant_from_action(LEGACY_ABIGAIL_ACTION).unwrap().id,
            ABIGAIL_LEGACY_ID
        );
    }
}
