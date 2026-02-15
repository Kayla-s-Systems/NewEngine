#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_ecs::EntityId;
use newengine_primitives::{Primitive, PrimitiveKind};
use newengine_scene::spawn_named;
use newengine_sim::{CameraInputComp, SimFrame, SimSchedule, SimStage};
use newengine_transform::Transform;

use crate::shared::{EditorCommand, EditorShared, SimMode};
use crate::viewport_bridge::ViewportBridge;

pub struct EditorSimModule {
    shared: EditorShared,
    schedule: SimSchedule,
    fixed_tick: u64,

    viewport_bridge: std::sync::Arc<ViewportBridge>,
    last_mode: SimMode,
}

impl EditorSimModule {
    #[inline]
    pub fn new(shared: EditorShared, viewport_bridge: std::sync::Arc<ViewportBridge>) -> Self {
        // Editor schedule intentionally runs only camera + derived state.
        // Gameplay/physics live in their own modules and are stepped in fixed-update.
        let mut schedule = SimSchedule::new();

        schedule.add_system(
            SimStage::Controllers,
            20,
            "orbit_camera",
            newengine_sim::sys_orbit_camera,
        );
        schedule.add_system(
            SimStage::Controllers,
            30,
            "camera_rig_to_transform",
            newengine_sim::sys_camera_rig_to_transform,
        );

        schedule.add_system(
            SimStage::Derived,
            0,
            "scene.update",
            newengine_scene::update_scene_world,
        );

        Self {
            shared,
            schedule,
            fixed_tick: 0,
            viewport_bridge,
            last_mode: SimMode::Edit,
        }
    }

    fn apply_viewport_camera_input(&self, world: &mut newengine_ecs::World) {
        // Read per-frame orbit input from UI and write it into the active camera component.
        // This keeps render fully read-only and deterministic w.r.t. the world state.
        let (dx_px, dy_px, wheel_y, _hovered, dragging) = self.viewport_bridge.read_orbit_input();
        let move_mask = self.viewport_bridge.read_move_keys();

        let mut move_axis = glam::Vec3::ZERO;

        // Orbit pan (XY): D-A, E-Q.
        if (move_mask & (1 << 3)) != 0 {
            move_axis.x += 1.0;
        }
        if (move_mask & (1 << 1)) != 0 {
            move_axis.x -= 1.0;
        }
        if (move_mask & (1 << 5)) != 0 {
            move_axis.y += 1.0;
        }
        if (move_mask & (1 << 4)) != 0 {
            move_axis.y -= 1.0;
        }

        // Orbit dolly (Z): W-S.
        if (move_mask & (1 << 0)) != 0 {
            move_axis.z += 1.0;
        }
        if (move_mask & (1 << 2)) != 0 {
            move_axis.z -= 1.0;
        }

        let speed_mul = if (move_mask & (1 << 6)) != 0 { 3.0 } else { 1.0 };

        let input = newengine_camera::CameraInput {
            look_active: dragging,
            look_delta: glam::Vec2::new(dx_px, -dy_px),
            move_axis,
            speed_mul,
            zoom_delta: wheel_y,
        };

        // Write into the active camera entity.
        for (id, _cam) in world.query::<newengine_scene::ActiveCamera>() {
            if let Some(ci) = world.get_mut_tracked::<CameraInputComp>(id) {
                ci.0 = input;
            } else {
                let _ = world.insert(id, CameraInputComp(input));
            }
        }
    }

    fn apply_commands(&mut self) {
        let mut cmds = self.shared.commands.lock();
        if cmds.is_empty() {
            return;
        }

        let mut scene = self.shared.scene.write();
        let world = scene.world_mut();

        let mut selection = self.shared.selection.write();

        for cmd in cmds.drain(..) {
            match cmd {
                EditorCommand::Select { entity } => {
                    *selection = entity;
                }
                EditorCommand::DeleteSelected => {
                    if let Some(e) = *selection {
                        let _ = world.despawn(e);
                        *selection = None;
                    }
                }
                EditorCommand::CreateEmpty => {
                    let id = spawn_named(world, "Entity");
                    *selection = Some(id);
                }
                EditorCommand::CreatePrimitive { kind } => {
                    let name = match kind {
                        PrimitiveKind::Cube => "Cube",
                        PrimitiveKind::Plane => "Plane",
                    };

                    let id = spawn_named(world, name);
                    let p = Primitive {
                        kind,
                        ..Primitive::default()
                    };
                    let _ = world.insert(id, p);

                    if let Some(t) = world.get_mut_tracked::<Transform>(id) {
                        match kind {
                            PrimitiveKind::Cube => {
                                t.position = glam::Vec3::new(0.0, 0.5, 0.0);
                            }
                            PrimitiveKind::Plane => {
                                t.scale = glam::Vec3::splat(10.0);
                            }
                        }
                    }

                    *selection = Some(id);
                }
            }
        }
    }
}

impl<E: Send + 'static> Module<E> for EditorSimModule {
    fn id(&self) -> &'static str {
        "app.editor_sim"
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        // Ensure scene is non-empty.
        if self.shared.scene.read().world().entity_count() == 0 {
            self.shared.rebuild_default_scene();
        }
        Ok(())
    }

    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>, dt: f32) -> EngineResult<()> {
        self.apply_commands();

        // Handle simulation mode transitions deterministically.
        {
            let sim = self.shared.sim.read();
            if self.last_mode == SimMode::Edit && sim.mode == SimMode::Playing {
                // Capture editor snapshot at the start of Play.
                let snap = crate::shared::SceneSnapshot::capture(&self.shared.scene.read());
                *self.shared.snapshot.lock() = Some(snap);
            }
            if self.last_mode != SimMode::Edit && sim.mode == SimMode::Edit {
                // Returning to Edit restores captured snapshot.
                self.fixed_tick = 0;
                self.shared.restore_snapshot_or_default();
            }
            self.last_mode = sim.mode;
        }

        let mut scene = self.shared.scene.write();
        self.apply_viewport_camera_input(scene.world_mut());

        // Editor camera + derived state run in the variable-step update.
        let frame = SimFrame::new(dt, self.fixed_tick);
        self.schedule
            .run_stage(scene.world_mut(), SimStage::Controllers, frame);
        self.schedule
            .run_stage(scene.world_mut(), SimStage::Derived, frame);
        Ok(())
    }

    fn fixed_update(&mut self, _ctx: &mut ModuleCtx<'_, E>, dt: f32) -> EngineResult<()> {
        let mut sim = self.shared.sim.write();
        if sim.request_reset {
            sim.request_reset = false;
            sim.request_step = false;
            sim.mode = SimMode::Edit;
            drop(sim);
            self.fixed_tick = 0;
            self.shared.restore_snapshot_or_default();
            return Ok(());
        }

        let do_step = match sim.mode {
            SimMode::Playing => true,
            SimMode::Paused | SimMode::Edit => {
                if sim.request_step {
                    sim.request_step = false;
                    true
                } else {
                    false
                }
            }
        };

        if !do_step {
            return Ok(());
        }

        drop(sim);

        let mut scene = self.shared.scene.write();
        let frame = SimFrame::new(dt, self.fixed_tick);

        // Пока шагаем только derived (физика подключится отдельным модулем/плагином).
        self.schedule.run_stage(scene.world_mut(), SimStage::Derived, frame);
        self.fixed_tick = self.fixed_tick.wrapping_add(1);
        Ok(())
    }
}