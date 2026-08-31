#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::spawn_default_player;
    use newengine_math::Vec3;
    use newengine_physics_api::{PhysicsContactEventDto, PhysicsEventDto, PhysicsQueryHitDto};

    #[test]
    fn report_conversion_preserves_contact_lifecycle() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let key_to_entity = BTreeMap::from([(a.stable_u64(), a), (b.stable_u64(), b)]);
        let contact = PhysicsContactEventDto {
            a: a.stable_u64(),
            b: b.stable_u64(),
            point: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            impulse: 4.5,
        };

        let report = report_from_dto(
            PhysicsStepReportDto {
                fixed_tick: 7,
                dt: 1.0 / 60.0,
                contacts: 1,
                ..PhysicsStepReportDto::default()
            },
            vec![
                PhysicsEventDto::ContactBegin(contact),
                PhysicsEventDto::ContactPersist(contact),
                PhysicsEventDto::ContactEnd {
                    a: a.stable_u64(),
                    b: b.stable_u64(),
                },
            ],
            &key_to_entity,
        );

        assert_eq!(report.fixed_tick, 7);
        assert_eq!(report.events.len(), 3);
        match report.events[0] {
            PhysicsEvent::ContactBegin(event) => {
                assert_eq!(event.a, a.into());
                assert_eq!(event.b, b.into());
                assert_eq!(event.point, newengine_math::Vec3::new(1.0, 2.0, 3.0));
                assert_eq!(event.normal, newengine_math::Vec3::Y);
                assert_eq!(event.impulse, 4.5);
            }
            ref other => panic!("expected contact begin, got {other:?}"),
        }
        assert!(matches!(report.events[1], PhysicsEvent::ContactPersist(_)));
        assert!(matches!(report.events[2], PhysicsEvent::ContactEnd { .. }));
    }

    #[test]
    fn report_conversion_drops_contacts_with_unknown_entities_and_sanitizes_impulse() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let key_to_entity = BTreeMap::from([(a.stable_u64(), a), (b.stable_u64(), b)]);

        let report = report_from_dto(
            PhysicsStepReportDto::default(),
            vec![
                PhysicsEventDto::ContactBegin(PhysicsContactEventDto {
                    a: a.stable_u64(),
                    b: b.stable_u64(),
                    point: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    impulse: f32::NAN,
                }),
                PhysicsEventDto::ContactPersist(PhysicsContactEventDto {
                    a: a.stable_u64(),
                    b: u64::MAX,
                    point: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    impulse: 1.0,
                }),
            ],
            &key_to_entity,
        );

        assert_eq!(report.events.len(), 1);
        match report.events[0] {
            PhysicsEvent::ContactBegin(event) => assert_eq!(event.impulse, 0.0),
            ref other => panic!("expected sanitized contact begin, got {other:?}"),
        }
    }

    #[test]
    fn ground_query_hit_marks_player_grounded_and_missing_hit_clears_it() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "grounded-player", Vec3::ZERO);
        let ground_key = u64::MAX - 7;

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 11,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 0.0, 0.0],
                    angular_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: ground_key,
                    position: [0.0, -0.03, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.03,
                }],
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );

        let grounded = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground state");
        assert!(grounded.grounded);
        assert_eq!(grounded.ground_entity, Some(ground_key));
        assert_eq!(grounded.last_fixed_tick, 11);

        for fixed_tick in [12_u64, 13] {
            apply_frame_output(
                &mut world,
                PhysicsFrameOutput {
                    fixed_tick,
                    ..PhysicsFrameOutput::default()
                },
                &GameplayPhysicsQueryProviderRegistry::new(),
            );
            let retained = world
                .get::<PlayerGroundState>(player)
                .copied()
                .expect("ground state");
            assert!(
                retained.grounded,
                "single/double probe miss must retain contact"
            );
            assert_eq!(retained.ground_entity, Some(ground_key));
            assert_eq!(
                retained.last_fixed_tick, 11,
                "misses are not contact observations"
            );
        }

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 14,
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );
        let cleared = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground state");
        assert!(!cleared.grounded);
        assert_eq!(cleared.ground_entity, None);
        assert_eq!(
            cleared.last_fixed_tick, 11,
            "last contact tick remains diagnostic truth"
        );
    }

    #[test]
    fn landing_and_footstep_events_use_ground_surface_profile() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning {
            footstep_stride: 0.2,
            landing_speed_threshold: 2.0,
            ..FpsPlayerTuning::default()
        }
        .sanitized();
        world.insert_resource(FpsDemoRules {
            player: tuning,
            ..FpsDemoRules::default()
        });
        let player = spawn_default_player(&mut world, None, "surface-player", Vec3::ZERO);
        let ground = world.spawn();
        crate::gameplay::ensure_physics_body(
            &mut world,
            ground,
            PhysicsBodyDesc::static_solid(newengine_physics_contracts::CollisionShapeDesc::Box {
                half_extents: [5.0, 0.5, 5.0],
            }),
        );
        let _ = world.insert(
            ground,
            PhysicsSurface {
                id: "surface.metal".to_owned(),
                ..PhysicsSurface::default()
            },
        );
        let _ = world.insert(player, Velocity(Vec3::new(6.0, -4.0, 0.0)));
        if let Some(locomotion) = world.get_mut::<PlayerLocomotionState>(player) {
            locomotion.was_grounded = false;
            locomotion.airborne_time = 0.5;
            locomotion.max_downward_speed = 4.0;
            locomotion.step_distance = 0.15;
        }

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 30,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 0.0, 0.0],
                    angular_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: ground.stable_u64(),
                    position: [0.0, -0.02, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.02,
                }],
                report: PhysicsStepReportDto {
                    fixed_tick: 30,
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );

        let events = crate::gameplay::drain_player_events(&mut world);
        assert!(events.iter().any(|event| {
            event.kind == PlayerEventKind::Landed
                && event.message.contains("surface='surface.metal'")
        }));
        assert!(events.iter().any(|event| {
            event.kind == PlayerEventKind::Footstep
                && event.message.contains("surface='surface.metal'")
        }));
    }

    #[test]
    fn ground_probe_rejects_surface_above_slope_limit() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "slope-player", Vec3::ZERO);
        let steep_normal = [0.866_025_4, 0.5, 0.0];

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 15,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 0.0, 0.0],
                    angular_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: u64::MAX - 8,
                    position: [0.0, -0.03, 0.0],
                    normal: steep_normal,
                    distance: 0.03,
                }],
                report: PhysicsStepReportDto {
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );

        let state = world
            .get::<PlayerGroundState>(player)
            .copied()
            .expect("ground state");
        assert!(!state.grounded);
        assert!(!state.walkable);
        assert!((state.slope_radians.to_degrees() - 60.0).abs() < 0.1);
    }

    #[test]
    fn stand_clearance_hit_keeps_player_crouched() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let player = spawn_default_player(&mut world, None, "blocked-stance", Vec3::ZERO);
        crate::gameplay::apply_player_stance_geometry(
            &mut world,
            player,
            PlayerStanceKind::Crouched,
            tuning,
            1,
        );
        if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
            state.stand_requested = true;
        }
        let ceiling = world.spawn();
        crate::gameplay::ensure_physics_body(
            &mut world,
            ceiling,
            PhysicsBodyDesc::static_solid(newengine_physics_contracts::CollisionShapeDesc::Box {
                half_extents: [1.0, 0.1, 1.0],
            }),
        );

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 2,
                query_hits: vec![PhysicsQueryHitDto {
                    seq: super::super::frame_input::stand_probe_query_seq(player.stable_u64(), 0),
                    entity: ceiling.stable_u64(),
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0, -1.0, 0.0],
                    distance: 0.1,
                }],
                report: PhysicsStepReportDto {
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );

        let state = world
            .get::<PlayerStanceState>(player)
            .expect("stance state");
        assert_eq!(state.current, PlayerStanceKind::Crouched);
        assert!(state.stand_blocked);
    }

    #[test]
    fn clear_stand_probe_restores_standing_capsule_and_foot_plane() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let initial_y = tuning.body_half_height + tuning.body_radius;
        let player = spawn_default_player(
            &mut world,
            None,
            "clear-stance",
            Vec3::new(0.0, initial_y, 0.0),
        );
        crate::gameplay::apply_player_stance_geometry(
            &mut world,
            player,
            PlayerStanceKind::Crouched,
            tuning,
            1,
        );
        if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
            state.stand_requested = true;
        }

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 2,
                report: PhysicsStepReportDto {
                    dt: 1.0 / 60.0,
                    ..PhysicsStepReportDto::default()
                },
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );

        let state = world
            .get::<PlayerStanceState>(player)
            .expect("stance state");
        assert_eq!(state.current, PlayerStanceKind::Standing);
        let transform = world.get::<Transform>(player).expect("player transform");
        assert!((transform.position.y - initial_y).abs() < 1.0e-6);
        let body = world.get::<PhysicsBodyDesc>(player).expect("player body");
        assert!(matches!(
            body.shape,
            newengine_physics_contracts::CollisionShapeDesc::Capsule { half_height, .. }
                if (half_height - tuning.body_half_height).abs() < 1.0e-6
        ));
    }

    #[test]
    fn upward_velocity_prevents_ground_probe_from_rearming_jump() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "jumping-player", Vec3::ZERO);

        apply_frame_output(
            &mut world,
            PhysicsFrameOutput {
                fixed_tick: 20,
                velocity_updates: vec![PhysicsBodyVelocityUpdate {
                    entity: player.stable_u64(),
                    linear_velocity: [0.0, 5.5, 0.0],
                    angular_velocity: [0.0, 0.0, 0.0],
                }],
                query_hits: vec![PhysicsQueryHitDto {
                    seq: player.stable_u64(),
                    entity: u64::MAX - 9,
                    position: [0.0, -0.02, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.02,
                }],
                ..PhysicsFrameOutput::default()
            },
            &GameplayPhysicsQueryProviderRegistry::new(),
        );

        assert!(
            !world
                .get::<PlayerGroundState>(player)
                .expect("ground state")
                .grounded
        );
    }
}
