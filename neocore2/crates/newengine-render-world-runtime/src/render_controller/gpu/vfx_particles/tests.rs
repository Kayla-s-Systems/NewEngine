use super::*;

    fn spawn(instance_id: u64, lifetime_seconds: f32) -> VfxGpuParticleSpawnV1 {
        VfxGpuParticleSpawnV1 {
            instance_id,
            kind: VfxGpuParticleKind::Spark,
            position: [1.0, 2.0, 3.0],
            velocity: [4.0, 5.0, 6.0],
            acceleration: [0.0, -9.81, 0.0],
            size: [0.02, 0.10],
            growth_per_second: [0.0; 2],
            color: [1.0, 0.8, 0.4, 1.0],
            lifetime_seconds,
            fade_start_fraction: 0.6,
            fade_in_fraction: 0.1,
            drag_per_second: 1.25,
            depth_softness_m: 0.06,
            rotation_radians: 0.3,
            angular_velocity_radians_per_second: 4.0,
            texture_slot: 2,
            billboard: newengine_vfx_api::VfxGpuBillboardMode::VelocityAligned,
        }
    }

    #[test]
    fn particle_slot_encoding_matches_std430_contract() {
        let bytes = encode_particle_slot(spawn(9, 2.0));
        assert_eq!(bytes.len(), PARTICLE_SLOT_BYTES);
        let lifetime = f32::from_ne_bytes(bytes[28..32].try_into().unwrap());
        let width = f32::from_ne_bytes(bytes[44..48].try_into().unwrap());
        let height = f32::from_ne_bytes(bytes[56..60].try_into().unwrap());
        let kind = f32::from_ne_bytes(bytes[80..84].try_into().unwrap());
        let depth_softness = f32::from_ne_bytes(bytes[92..96].try_into().unwrap());
        let drag = f32::from_ne_bytes(bytes[96..100].try_into().unwrap());
        let rotation = f32::from_ne_bytes(bytes[100..104].try_into().unwrap());
        assert_eq!(lifetime, 2.0);
        assert_eq!(width, 0.02);
        assert_eq!(height, 0.10);
        assert_eq!(kind, 2.0);
        assert_eq!(depth_softness, 0.06);
        assert_eq!(drag, 1.25);
        assert_eq!(rotation, 0.3);
    }

    #[test]
    fn muzzle_particle_kinds_keep_stable_shader_ids() {
        let mut flash = spawn(10, 0.05);
        flash.kind = VfxGpuParticleKind::MuzzleFlash;
        let flash_bytes = encode_particle_slot(flash);
        assert_eq!(
            f32::from_ne_bytes(flash_bytes[80..84].try_into().unwrap()),
            4.0
        );

        let mut core = spawn(11, 0.04);
        core.kind = VfxGpuParticleKind::MuzzleCore;
        let core_bytes = encode_particle_slot(core);
        assert_eq!(
            f32::from_ne_bytes(core_bytes[80..84].try_into().unwrap()),
            5.0
        );
    }

    #[test]
    fn slot_allocator_does_not_overwrite_live_particles() {
        let mut renderer = VfxGpuRenderer::new();
        renderer.slot_deadlines.fill(10.0);
        renderer.high_water = VFX_GPU_PARTICLE_CAPACITY;
        assert!(renderer.allocate_slot(1, 1.0).is_none());
        renderer.slot_deadlines[17] = 0.0;
        renderer.next_slot = 17;
        assert_eq!(renderer.allocate_slot(2, 1.0), Some(17));
        assert_eq!(renderer.slot_instance_ids[17], 2);
    }
