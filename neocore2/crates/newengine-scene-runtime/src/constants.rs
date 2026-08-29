use newengine_scene_io::method as scene_method;

pub const SCENE_GATEWAY_OWNER: &str = "newengine-scene-runtime.scene-gateway";

pub(crate) mod authored_scene_method {
    // Local bridge constants keep authored-structure methods buildable even when
    // an incremental workspace still contains a stale scene-io artifact.
    pub(crate) const GRAPH_JSON_V1: &str = "scene.graph_json_v1";
    pub(crate) const ARCHETYPE_GRAPH_JSON_V1: &str = "scene.archetype_graph_json_v1";
    pub(crate) const PLACEMENTS_JSON_V1: &str = "scene.placements_json_v1";
    pub(crate) const INSTANTIATE_PREFAB_JSON_V1: &str = "scene.instantiate_prefab_json_v1";
    pub(crate) const INSTANTIATE_ARCHETYPE_JSON_V1: &str = "scene.instantiate_archetype_json_v1";
}

pub(crate) const SCENE_SERVICE_METHODS: &[&str] = &[
    scene_method::FORMATS_JSON,
    scene_method::LOAD_JSON_V1,
    scene_method::SAVE_JSON_V1,
    authored_scene_method::GRAPH_JSON_V1,
    authored_scene_method::ARCHETYPE_GRAPH_JSON_V1,
    authored_scene_method::PLACEMENTS_JSON_V1,
    authored_scene_method::INSTANTIATE_PREFAB_JSON_V1,
    authored_scene_method::INSTANTIATE_ARCHETYPE_JSON_V1,
    scene_method::SHUTDOWN_V1,
];

pub(crate) const SCENE_FORMAT_METHODS: &[&str] = &[
    scene_method::FORMATS_JSON,
    scene_method::LOAD_JSON_V1,
    scene_method::SAVE_JSON_V1,
    authored_scene_method::GRAPH_JSON_V1,
    authored_scene_method::ARCHETYPE_GRAPH_JSON_V1,
    authored_scene_method::PLACEMENTS_JSON_V1,
    scene_method::SHUTDOWN_V1,
];
