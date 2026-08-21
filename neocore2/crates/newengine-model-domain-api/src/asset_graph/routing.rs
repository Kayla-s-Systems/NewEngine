use super::split_asset_ref;
use crate::{
    DRAWABLE_DICTIONARY_ASSET_KIND, MATERIAL_LIBRARY_ASSET_KIND,
    OBJECT_TYPE_DEFINITIONS_ASSET_KIND, ROLE_ASSET_PROPERTIES, ROLE_DRAWABLE_DICTIONARY,
    ROLE_MATERIAL_LIBRARY, ROLE_TEXTURE_DICTIONARY, ROLE_UV_LAYOUT_DICTIONARY,
    TEXTURE_DICTIONARY_ASSET_KIND, UV_LAYOUT_DICTIONARY_ASSET_KIND,
};

pub(super) fn classify_ref(
    reference: &str,
) -> (&'static str, &'static str, &'static str, &'static str) {
    let (path, _) = split_asset_ref(reference);
    let ext = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "ytyp" => (
            ROLE_ASSET_PROPERTIES,
            OBJECT_TYPE_DEFINITIONS_ASSET_KIND,
            "engine.assets.definitions",
            "definitions.properties_json_v1",
        ),
        "ydd" => (
            ROLE_DRAWABLE_DICTIONARY,
            DRAWABLE_DICTIONARY_ASSET_KIND,
            "engine.model",
            "model.drawable_dictionary_manifest_json_v1",
        ),
        "ytyd" => (
            ROLE_UV_LAYOUT_DICTIONARY,
            UV_LAYOUT_DICTIONARY_ASSET_KIND,
            "engine.model",
            "model.uv_layout_dictionary_manifest_json_v1",
        ),
        "nemat" => (
            ROLE_MATERIAL_LIBRARY,
            MATERIAL_LIBRARY_ASSET_KIND,
            "engine.materials",
            "materials.load_descriptor_v1",
        ),
        "ytd" => (
            ROLE_TEXTURE_DICTIONARY,
            TEXTURE_DICTIONARY_ASSET_KIND,
            "engine.assets",
            "asset.texture_dictionary_runtime_v1",
        ),
        "ymap" => (
            "map_data",
            "map_data",
            "engine.scene",
            "scene.resolve_map_v1",
        ),
        "ybn" | "ybd" | "ycol" => (
            "physics_dictionary",
            "physics_dictionary",
            "engine.physics",
            "physics.validate_v1",
        ),
        "ydr" | "yft" | "yvr" | "yld" => (
            "model_dependency",
            "model_dependency",
            "engine.model",
            "model.resolve_drawable_v1",
        ),
        "ycd" | "yed" | "yfd" | "ypdb" => (
            "skeleton_animation_dependency",
            "skeleton_animation_dependency",
            "engine.model",
            "model.resolve_animation_dependency_v1",
        ),
        "ymf" => (
            "asset_manifest",
            "asset_manifest",
            "engine.assets.graph",
            "assets.graph.resolve_v1",
        ),
        "ymt" | "ytf" => (
            "metadata",
            "metadata",
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            newengine_assets_api::definitions_method::ENTRY_JSON_V1,
        ),
        "ywr" => (
            "scene_dependency",
            "scene_dependency",
            "engine.scene",
            "scene.resolve_waypoints_v1",
        ),
        "ysc" => (
            "script_module",
            "script_module",
            "engine.scripting",
            "scripting.load_module_v1",
        ),
        "nebrain" => (
            "ai_brain",
            "ai_brain_dictionary",
            "engine.ai",
            "ai.brain_manifest_v1",
        ),
        "negoal" => (
            "ai_goal",
            "ai_goal_dictionary",
            "engine.ai",
            "ai.goal_manifest_v1",
        ),
        "nebt" | "nebehavior" => (
            "ai_behavior_tree",
            "ai_behavior_tree",
            "engine.ai",
            "ai.behavior_tree_manifest_v1",
        ),
        "neutility" => (
            "ai_utility",
            "ai_utility_dictionary",
            "engine.ai",
            "ai.utility_manifest_v1",
        ),
        "nebb" | "nemem" => (
            "ai_blackboard",
            "ai_blackboard_schema",
            "engine.ai",
            "ai.memory_manifest_v1",
        ),
        "nepat" => (
            "ai_pattern",
            "ai_pattern_dictionary",
            "engine.ai",
            "ai.pattern_manifest_v1",
        ),
        _ => ("asset", "unknown", "engine.assets", "asset.decode_v1"),
    }
}
