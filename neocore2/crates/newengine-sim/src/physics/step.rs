#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::World;

use super::kinematic::{apply_kinematic_targets_locked, gather_kinematic_targets};
use super::types::{PhysicsCtx, PhysicsSettings, PhysicsStepState};

pub fn physics_step_jolt(world: &mut World, frame: super::super::SimFrame) {
    if !frame.dt.is_finite() || frame.dt <= 0.0 {
        return;
    }
    if world.resource::<PhysicsCtx>().is_none() {
        return;
    }

    let settings = *world
        .resource::<PhysicsSettings>()
        .unwrap_or(&PhysicsSettings::default());

    let dt = frame.dt.min(settings.max_frame_dt);
    let kin_targets = gather_kinematic_targets(world);

    let (mut accum, mut tick) = {
        let s = world.resource::<PhysicsStepState>().expect("physics step state must exist");
        (s.accum, s.tick)
    };

    let use_external_tick = frame.fixed_tick != 0
        && settings.fixed_dt > 0.0
        && (dt - settings.fixed_dt).abs() <= settings.fixed_dt * 0.02;

    let mut steps_to_run: u32 = 0;
    if use_external_tick {
        let delta = frame.fixed_tick.saturating_sub(tick);
        steps_to_run = delta.min(settings.max_substeps as u64) as u32;
        accum = 0.0;
    } else {
        accum = (accum + dt).max(0.0);
        steps_to_run = (accum / settings.fixed_dt).floor() as u32;
        steps_to_run = steps_to_run.min(settings.max_substeps);
    }

    if steps_to_run == 0 {
        if !use_external_tick && settings.fixed_dt > 0.0 {
            let alpha = (accum / settings.fixed_dt).clamp(0.0, 1.0);
            if let Some(s) = world.resource_mut::<PhysicsStepState>() {
                s.accum = accum;
                s.alpha = alpha;
                s.steps_last = 0;
            }
        }
        return;
    }

    let mut executed: u32 = 0;
    {
        let mut pw_guard = {
            let ctx = world.resource::<PhysicsCtx>().expect("physics ctx must exist");
            ctx.world.lock().ok()
        };
        let Some(mut pw_guard) = pw_guard else { return; };
        let pw = &mut *pw_guard;

        for _ in 0..steps_to_run {
            apply_kinematic_targets_locked(pw, &kin_targets);

            if pw.step(settings.fixed_dt).is_err() {
                break;
            }

            executed += 1;
            tick = tick.wrapping_add(1);
            if !use_external_tick {
                accum -= settings.fixed_dt;
            }
        }
    }

    let alpha = if use_external_tick {
        0.0
    } else if settings.fixed_dt > 0.0 {
        (accum / settings.fixed_dt).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let state = world.resource_mut::<PhysicsStepState>().expect("physics step state must exist");
    state.accum = accum;
    state.alpha = alpha;
    state.tick = tick;
    state.steps_last = executed;
}