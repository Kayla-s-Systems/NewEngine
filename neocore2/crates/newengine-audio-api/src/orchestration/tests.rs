    use super::*;

    fn test_graph() -> AudioMixGraph {
        AudioMixGraph {
            buses: vec![
                AudioMixBusSpec {
                    id: AudioRouteId::new("project.output"),
                    gain_db: -1.0,
                    ..Default::default()
                },
                AudioMixBusSpec {
                    id: AudioRouteId::new("project.world"),
                    parent: Some(AudioRouteId::new("project.output")),
                    gain_db: -2.0,
                },
                AudioMixBusSpec {
                    id: AudioRouteId::new("project.world.custom.fx"),
                    parent: Some(AudioRouteId::new("project.world")),
                    gain_db: -3.0,
                },
            ],
            snapshots: vec![AudioMixSnapshotSpec {
                id: "focus.anything".to_owned(),
                transition_seconds: 0.5,
                patches: vec![AudioMixPatch {
                    route: AudioRouteId::new("project.world"),
                    gain_db: -6.0,
                }],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn project_routes_are_opaque_and_snapshot_patches_inherit() {
        let graph = test_graph();
        graph.validate().expect("graph");
        let weights = BTreeMap::from([("focus.anything".to_owned(), 0.5)]);
        let gain = graph
            .effective_gain_db(&AudioRouteId::new("project.world.custom.fx"), &weights)
            .expect("gain");
        assert!((gain - -9.0).abs() < 1.0e-6);
    }

    #[test]
    fn mix_graph_rejects_parent_cycles() {
        let graph = AudioMixGraph {
            buses: vec![
                AudioMixBusSpec {
                    id: AudioRouteId::new("a"),
                    parent: Some(AudioRouteId::new("b")),
                    gain_db: 0.0,
                },
                AudioMixBusSpec {
                    id: AudioRouteId::new("b"),
                    parent: Some(AudioRouteId::new("a")),
                    gain_db: 0.0,
                },
            ],
            ..Default::default()
        };
        assert!(graph.validate().is_err());
    }

    #[test]
    fn object_parameters_do_not_require_engine_known_names() {
        let mut parameters = AudioParameterSet::default();
        parameters
            .set_scalar("project.machine.pressure", 0.73)
            .expect("scalar");
        parameters
            .set_switch("project.location.surface", "obsidian")
            .expect("switch");
        assert_eq!(parameters.scalars["project.machine.pressure"], 0.73);
        assert_eq!(parameters.switches["project.location.surface"], "obsidian");
    }

    #[test]
    fn mix_graph_rejects_duplicate_voice_budget_ids_case_insensitively() {
        let graph = AudioMixGraph {
            voice_budgets: vec![
                AudioVoiceBudgetReservation {
                    id: "project.dialogue".to_owned(),
                    reserved_physical_voices: 4,
                },
                AudioVoiceBudgetReservation {
                    id: "PROJECT.DIALOGUE".to_owned(),
                    reserved_physical_voices: 2,
                },
            ],
            ..Default::default()
        };
        assert!(graph.validate().is_err());
    }
    #[test]
    fn parameter_overlay_is_deterministic_global_object_instance_precedence() {
        let mut global = AudioParameterSet::default();
        global.set_scalar("project.value", 1.0).unwrap();
        global.set_switch("project.mode", "global").unwrap();
        let mut object = AudioParameterSet::default();
        object.set_scalar("project.value", 2.0).unwrap();
        object.set_switch("project.mode", "object").unwrap();
        let mut instance = AudioParameterSet::default();
        instance.set_scalar("project.value", 3.0).unwrap();

        global.overlay_from(&object);
        global.overlay_from(&instance);
        assert_eq!(global.scalars.get("project.value"), Some(&3.0));
        assert_eq!(
            global.switches.get("project.mode").map(String::as_str),
            Some("object")
        );
    }
