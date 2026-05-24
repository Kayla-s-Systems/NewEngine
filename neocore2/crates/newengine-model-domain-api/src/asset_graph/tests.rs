    use super::*;
    use crate::{DataDrivenAssetLink, DataDrivenObjectConstruction};

    #[test]
    fn root_ref_classifies_ytyp_as_definitions_not_scene() {
        let graph = AssetGraphResolver::resolve_root_ref("world/foo.ytyp@bar");
        let root = graph.nodes.iter().find(|node| node.reference == "world/foo.ytyp@bar").unwrap();
        assert_eq!(root.semantic_gateway, "engine.definitions");
        assert_eq!(root.method, "definitions.entry_json_v1");
        assert_eq!(root.semantic_owner, OBJECT_TYPE_DEFINITIONS_ASSET_KIND);
        assert_ne!(root.semantic_gateway, "engine.scene");
        assert_eq!(root.role, ROLE_DEFINITION_ENTRIES);
        assert_eq!(root.byte_owner, "engine.assets");
    }

    #[test]
    fn construction_plan_builds_declarative_edges() {
        let plan = DataDrivenConstructionPlan {
            source: "world/foo.ytyp".to_owned(),
            objects: vec![DataDrivenObjectConstruction {
                name: "bar".to_owned(),
                definition: DataDrivenAssetLink { logical_path: "world/foo.ytyp@bar".to_owned(), asset_kind: OBJECT_TYPE_DEFINITIONS_ASSET_KIND.to_owned(), extension: "ytyp".to_owned(), required: true, ..Default::default() },
                drawable: Some(DataDrivenAssetLink { logical_path: "models/foo.ydd@bar".to_owned(), asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(), extension: "ydd".to_owned(), required: true, ..Default::default() }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let graph = AssetGraphResolver::resolve_construction_plan(&plan);
        assert!(graph.edges.iter().any(|edge| edge.kind == ROLE_DRAWABLE_DICTIONARY));
        assert!(graph.debug_log.iter().any(|line| line.contains("assets.graph.resolve_v1")));
    }

    #[test]
    fn cycles_are_reported_by_refs_not_internal_ids() {
        let mut graph = AssetGraphResolver::resolve_root_ref("a.ytyp@a");
        push_manifest_dependency(&mut graph, "a.ytyp@a", "b.ydd@b", "test", true);
        push_manifest_dependency(&mut graph, "b.ydd@b", "a.ytyp@a", "test", true);
        finalize_graph(&mut graph);
        assert!(graph.cycle_errors.iter().any(|cycle| cycle.contains("a.ytyp@a")));
    }
