#[cfg(test)]
mod tests {
    use super::*;
    use newengine_input_actions_api::ActionCommandFrame;

    #[test]
    fn player_command_handoff_preserves_frame_sequence_and_generic_actions() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "test-player", Vec3::ZERO);
        let actions = ActionCommandFrame {
            held: vec!["game.action.held".into()],
            pressed: vec!["game.action.pulse".into()],
            released: Vec::new(),
        };

        apply_player_command_frame(&mut world, player, 42, actions.clone());

        assert_eq!(
            world.get::<PlayerCommandFrame>(player).cloned(),
            Some(PlayerCommandFrame::new(42, actions))
        );
    }

    #[test]
    fn transient_input_buffers_pulses_and_replaces_latest_held_state() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "test-player", Vec3::ZERO);
        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD,
            Vec2::new(2.0, -1.0),
            true,
        );
        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD,
            Vec2::new(3.0, 4.0),
            true,
        );
        apply_player_command_frame(
            &mut world,
            player,
            7,
            ActionCommandFrame {
                held: vec!["game.fire".into()],
                pressed: vec!["game.pulse.a".into()],
                released: Vec::new(),
            },
        );
        apply_player_command_frame(
            &mut world,
            player,
            8,
            ActionCommandFrame {
                held: vec!["game.aim".into()],
                pressed: vec!["game.pulse.b".into()],
                released: Vec::new(),
            },
        );

        assert_eq!(
            world.get::<MotorInput>(player).map(|input| input.look_delta),
            Some(Vec2::new(5.0, 3.0))
        );
        let pending = world
            .get::<PlayerCommandFrame>(player)
            .cloned()
            .expect("player command frame");
        assert_eq!(pending.source_frame, 8);
        assert!(pending.actions.is_pressed("game.pulse.a"));
        assert!(pending.actions.is_pressed("game.pulse.b"));
        assert!(!pending.actions.is_held("game.fire"));
        assert!(pending.actions.is_held("game.aim"));

        consume_player_transient_input(&mut world);

        assert_eq!(
            world.get::<MotorInput>(player).map(|input| input.look_delta),
            Some(Vec2::ZERO)
        );
        let consumed = world
            .get::<PlayerCommandFrame>(player)
            .cloned()
            .expect("player command frame");
        assert!(consumed.actions.pressed.is_empty());
        assert!(consumed.actions.released.is_empty());
        assert!(consumed.actions.is_held("game.aim"));
    }

    #[test]
    fn dead_player_cannot_apply_movement_or_exertion() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "dead-player", Vec3::ZERO);
        let _ = world.insert(player, CharacterLifeState::Dead);

        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD | input_move::SPRINT,
            Vec2::new(4.0, 2.0),
            true,
        );

        let input = world.get::<MotorInput>(player).expect("motor input");
        assert_eq!(input.move_axis, Vec3::ZERO);
        assert_eq!(input.look_delta, Vec2::ZERO);
        assert!(!world.get::<CharacterExertionState>(player).unwrap().sprinting);
    }

    #[test]
    fn exhausted_stamina_blocks_sprint_until_resume_threshold() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "stamina-player", Vec3::ZERO);
        let tuning = world.get::<StaminaTuning>(player).copied().expect("stamina tuning");
        {
            let stamina = world.get_mut::<Stamina>(player).expect("stamina");
            stamina.spend(stamina.maximum, tuning);
        }

        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD | input_move::SPRINT,
            Vec2::ZERO,
            false,
        );
        assert_eq!(world.get::<MotorInput>(player).unwrap().speed_mul, 1.0);
        assert!(!world.get::<CharacterExertionState>(player).unwrap().sprinting);

        {
            let stamina = world.get_mut::<Stamina>(player).expect("stamina");
            stamina.restore(stamina.maximum * tuning.exhausted_resume_fraction, tuning);
        }
        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD | input_move::SPRINT,
            Vec2::ZERO,
            false,
        );
        assert_eq!(
            world.get::<MotorInput>(player).unwrap().speed_mul,
            world
                .get::<PlayerMovementSpeeds>(player)
                .copied()
                .unwrap_or_default()
                .sprint_multiplier()
        );
        assert!(world.get::<CharacterExertionState>(player).unwrap().sprinting);
    }

    #[test]
    fn character_spawn_projects_generic_body_into_capsule_and_stance() {
        let mut world = World::new();
        let body = CharacterBody {
            radius: 0.33,
            standing_half_height: 0.77,
            crouched_half_height: 0.31,
            standing_eye_height: 1.42,
            crouched_eye_height: 0.91,
            visual_radius: 0.4,
            visual_half_height: 1.1,
        };
        let player = spawn_player_controller(
            &mut world,
            None,
            "generic-character",
            Vec3::ZERO,
            body,
            CharacterMotionTuning::default(),
            false,
        );

        let stored = world.get::<CharacterBody>(player).copied().expect("character body");
        assert_eq!(stored, body.sanitized());
        let physics = world.get::<PhysicsBodyDesc>(player).expect("physics body");
        assert_eq!(
            physics.shape,
            CollisionShapeDesc::Capsule {
                radius: stored.radius,
                half_height: stored.standing_half_height,
            }
        );
        assert_eq!(
            world.get::<PlayerStanceState>(player).map(|state| state.current_eye_height),
            Some(stored.standing_eye_height)
        );
        assert_eq!(
            world
                .get::<PlayerModelAssignment>(player)
                .map(|assignment| (assignment.enabled, assignment.revision)),
            Some((false, 0))
        );
    }

    #[test]
    fn player_model_assignment_is_revisioned_without_replacing_player_actor() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "model-player", Vec3::ZERO);

        let first_revision = set_player_model_assignment(
            &mut world,
            player,
            PlayerModelAssignment::new(
                "models/characters/abigail/csb_abigail.ydd@csb_abigail",
            ),
        )
        .expect("first model assignment");
        assert_eq!(first_revision, 1);
        assert!(world.get::<PlayerActor>(player).is_some());
        assert_eq!(
            world
                .get::<PlayerModelAssignment>(player)
                .map(|assignment| assignment.source.as_str()),
            Some("models/characters/abigail/csb_abigail.ydd@csb_abigail")
        );

        let second_revision = set_player_model_assignment(
            &mut world,
            player,
            PlayerModelAssignment::new("models/characters/other/hero.ydd@hero"),
        )
        .expect("replacement model assignment");
        assert_eq!(second_revision, 2);
        assert!(world.get::<PlayerActor>(player).is_some());

        let cleared_revision =
            clear_player_model_assignment(&mut world, player).expect("clear model assignment");
        assert_eq!(cleared_revision, 3);
        let cleared = world
            .get::<PlayerModelAssignment>(player)
            .expect("cleared assignment component");
        assert!(!cleared.enabled);
        assert!(cleared.source.is_empty());
    }

    #[test]
    fn generic_stance_geometry_preserves_foot_plane() {
        let mut world = World::new();
        let body = CharacterBody::default().sanitized();
        let player = spawn_player_controller(
            &mut world,
            None,
            "stance-character",
            Vec3::new(0.0, body.standing_half_height + body.radius, 0.0),
            body,
            CharacterMotionTuning::default(),
            false,
        );
        let before_center = world.get::<Transform>(player).expect("transform").position.y;
        let before_foot = before_center - body.standing_half_height - body.radius;

        assert!(apply_player_stance_geometry(
            &mut world,
            player,
            PlayerStanceKind::Crouched,
            5,
        ));

        let after_center = world.get::<Transform>(player).expect("transform").position.y;
        let after_foot = after_center - body.crouched_half_height - body.radius;
        assert!((after_foot - before_foot).abs() <= 1.0e-6);
        let stance = world.get::<PlayerStanceState>(player).expect("stance state");
        assert_eq!(stance.current, PlayerStanceKind::Crouched);
        assert_eq!(stance.target_eye_height, body.crouched_eye_height);
    }
}
