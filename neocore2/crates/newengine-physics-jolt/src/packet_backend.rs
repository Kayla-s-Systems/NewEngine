use std::collections::{BTreeMap, BTreeSet};
use std::ptr;

use joltc_sys as sys;
use newengine_physics_api::{
    PhysicsBodyKindDto, PhysicsBodyPoseUpdate, PhysicsBodyVelocityUpdate, PhysicsCommandKindDto,
    PhysicsEventDto, PhysicsFrameBodySnapshot, PhysicsFrameColliderSnapshot, PhysicsFrameInput,
    PhysicsFrameOutput, PhysicsQueryHitDto, PhysicsQueryKindDto, PhysicsStepReportDto,
};

use crate::raw::{arr_from_quat, arr_from_rvec3, arr_from_vec3, quat, rvec3, vec3};
use crate::shapes::{create_body_shape, create_collider_shape, BodySignature};
use crate::world::{JoltInitDesc, PhysicsWorld, LAYER_DYNAMIC, LAYER_STATIC};

const INVALID_BODY_ID: sys::JPC_BodyID = u32::MAX;

struct BodyRecord {
    body_id: sys::JPC_BodyID,
    shape: *mut sys::JPC_Shape,
    signature: BodySignature,
    produces_output: bool,
}

impl BodyRecord {
    fn new(
        body_id: sys::JPC_BodyID,
        shape: *mut sys::JPC_Shape,
        signature: BodySignature,
        produces_output: bool,
    ) -> Self {
        Self { body_id, shape, signature, produces_output }
    }
}

/// Packet-facing Jolt backend used by the `physics.api` plugin.
///
/// This type owns the native Jolt world and maps the stable NewEngine
/// `PhysicsFrameInput` DTO into Jolt bodies. ECS never crosses this boundary.
pub struct JoltPacketPhysicsBackend {
    world: PhysicsWorld,
    bodies: BTreeMap<u64, BodyRecord>,
    body_to_entity: BTreeMap<sys::JPC_BodyID, u64>,
}

// SAFETY: this backend owns native Jolt handles and is only accessed through the
// plugin's mutex. The runtime service boundary uses single-writer step calls.
unsafe impl Send for JoltPacketPhysicsBackend {}

impl JoltPacketPhysicsBackend {
    pub fn new(init: JoltInitDesc) -> Result<Self, String> {
        let world = PhysicsWorld::new(init).map_err(|e| e.to_string())?;
        Ok(Self { world, bodies: BTreeMap::new(), body_to_entity: BTreeMap::new() })
    }

    pub fn shutdown(&mut self) {
        let entities = self.bodies.keys().copied().collect::<Vec<_>>();
        for entity in entities {
            self.destroy_body(entity);
        }
    }

    pub fn step_frame(&mut self, input: PhysicsFrameInput) -> Result<PhysicsFrameOutput, String> {
        let mut events = Vec::new();
        let mut commands_applied = 0usize;

        self.sync_bodies(&input, &mut events)?;
        commands_applied += self.apply_commands(&input)?;

        self.world.step(input.dt).map_err(|e| e.to_string())?;

        let mut output = PhysicsFrameOutput {
            fixed_tick: input.fixed_tick,
            pose_updates: Vec::new(),
            velocity_updates: Vec::new(),
            events,
            query_hits: self.execute_queries(&input),
            report: PhysicsStepReportDto {
                fixed_tick: input.fixed_tick,
                dt: input.dt,
                substeps: 1,
                active_bodies: self.bodies.len(),
                static_bodies: input
                    .bodies
                    .iter()
                    .filter(|b| b.kind == PhysicsBodyKindDto::Static)
                    .count()
                    + input.colliders.len(),
                dynamic_bodies: input
                    .bodies
                    .iter()
                    .filter(|b| b.kind == PhysicsBodyKindDto::Dynamic)
                    .count(),
                contacts: 0,
                commands_applied,
            },
        };

        self.collect_body_outputs(&mut output);
        Ok(output)
    }

    fn sync_bodies(
        &mut self,
        input: &PhysicsFrameInput,
        events: &mut Vec<PhysicsEventDto>,
    ) -> Result<(), String> {
        let mut present = BTreeSet::new();
        self.sync_frame_bodies(input, events, &mut present)?;
        self.sync_static_colliders(input, events, &mut present)?;
        self.destroy_stale_bodies(&present, events);
        Ok(())
    }

    fn sync_frame_bodies(
        &mut self,
        input: &PhysicsFrameInput,
        events: &mut Vec<PhysicsEventDto>,
        present: &mut BTreeSet<u64>,
    ) -> Result<(), String> {
        for snapshot in &input.bodies {
            present.insert(snapshot.entity);
            let signature = BodySignature::from_body(snapshot);
            self.recreate_if_signature_changed(snapshot.entity, signature, events);

            if !self.bodies.contains_key(&snapshot.entity) {
                let record = self.create_frame_body(snapshot, signature)?;
                self.body_to_entity.insert(record.body_id, snapshot.entity);
                self.bodies.insert(snapshot.entity, record);
                events.push(PhysicsEventDto::BodyCreated { entity: snapshot.entity });
            } else {
                // ECS remains authoritative for the next frame input. For controlled
                // characters, host-side output application decides which axes are
                // accepted back from physics.
                self.set_pose(snapshot.entity, snapshot.position, snapshot.rotation);
                self.set_linear_velocity(snapshot.entity, snapshot.linear_velocity);
            }
        }
        Ok(())
    }

    fn sync_static_colliders(
        &mut self,
        input: &PhysicsFrameInput,
        events: &mut Vec<PhysicsEventDto>,
        present: &mut BTreeSet<u64>,
    ) -> Result<(), String> {
        for snapshot in &input.colliders {
            present.insert(snapshot.entity);
            let signature = BodySignature::from_collider(snapshot);
            self.recreate_if_signature_changed(snapshot.entity, signature, events);

            if !self.bodies.contains_key(&snapshot.entity) {
                let record = self.create_static_collider(snapshot, signature)?;
                self.body_to_entity.insert(record.body_id, snapshot.entity);
                self.bodies.insert(snapshot.entity, record);
                events.push(PhysicsEventDto::BodyCreated { entity: snapshot.entity });
            } else {
                self.set_pose(snapshot.entity, snapshot.position, snapshot.rotation);
            }
        }
        Ok(())
    }

    fn recreate_if_signature_changed(
        &mut self,
        entity: u64,
        signature: BodySignature,
        events: &mut Vec<PhysicsEventDto>,
    ) {
        let recreate = self
            .bodies
            .get(&entity)
            .map(|record| record.signature != signature)
            .unwrap_or(false);
        if recreate {
            self.destroy_body(entity);
            events.push(PhysicsEventDto::BodyDestroyed { entity });
        }
    }

    fn destroy_stale_bodies(&mut self, present: &BTreeSet<u64>, events: &mut Vec<PhysicsEventDto>) {
        let stale = self
            .bodies
            .keys()
            .copied()
            .filter(|entity| !present.contains(entity))
            .collect::<Vec<_>>();
        for entity in stale {
            self.destroy_body(entity);
            events.push(PhysicsEventDto::BodyDestroyed { entity });
        }
    }

    fn create_frame_body(
        &mut self,
        snapshot: &PhysicsFrameBodySnapshot,
        signature: BodySignature,
    ) -> Result<BodyRecord, String> {
        let shape = create_body_shape(snapshot.shape, snapshot.material.density.max(0.0001))?;
        let body_interface = self.body_interface_mut()?;

        let mut settings = sys::JPC_BodyCreationSettings::default();
        settings.Position = rvec3(snapshot.position);
        settings.Rotation = quat(snapshot.rotation);
        settings.LinearVelocity = vec3(snapshot.linear_velocity);
        settings.AngularVelocity = vec3([0.0, 0.0, 0.0]);
        settings.UserData = snapshot.entity;
        settings.ObjectLayer = match snapshot.kind {
            PhysicsBodyKindDto::Static => LAYER_STATIC,
            PhysicsBodyKindDto::Dynamic | PhysicsBodyKindDto::Kinematic => LAYER_DYNAMIC,
        };
        settings.MotionType = match snapshot.kind {
            PhysicsBodyKindDto::Static => sys::JPC_MOTION_TYPE_STATIC,
            PhysicsBodyKindDto::Dynamic => sys::JPC_MOTION_TYPE_DYNAMIC,
            PhysicsBodyKindDto::Kinematic => sys::JPC_MOTION_TYPE_KINEMATIC,
        };
        settings.IsSensor = snapshot.flags.is_trigger;
        settings.Friction = snapshot.material.friction.clamp(0.0, 10.0);
        settings.Restitution = snapshot.material.restitution.clamp(0.0, 1.0);
        settings.GravityFactor = if snapshot.kind == PhysicsBodyKindDto::Dynamic { 1.0 } else { 0.0 };
        settings.Shape = shape as *const sys::JPC_Shape;

        let body_id = unsafe {
            sys::JPC_BodyInterface_CreateAndAddBody(
                body_interface,
                &settings,
                sys::JPC_ACTIVATION_ACTIVATE,
            )
        };

        if body_id == INVALID_BODY_ID {
            unsafe { sys::JPC_Shape_Release(shape as *const sys::JPC_Shape) };
            return Err(format!("Jolt CreateAndAddBody failed for entity {}", snapshot.entity));
        }

        Ok(BodyRecord::new(body_id, shape, signature, true))
    }

    fn create_static_collider(
        &mut self,
        snapshot: &PhysicsFrameColliderSnapshot,
        signature: BodySignature,
    ) -> Result<BodyRecord, String> {
        let shape = create_collider_shape(&snapshot.collider)?;
        let body_interface = self.body_interface_mut()?;

        let mut settings = sys::JPC_BodyCreationSettings::default();
        settings.Position = rvec3(snapshot.position);
        settings.Rotation = quat(snapshot.rotation);
        settings.LinearVelocity = vec3([0.0, 0.0, 0.0]);
        settings.AngularVelocity = vec3([0.0, 0.0, 0.0]);
        settings.UserData = snapshot.entity;
        settings.ObjectLayer = LAYER_STATIC;
        settings.MotionType = sys::JPC_MOTION_TYPE_STATIC;
        settings.IsSensor = snapshot.flags.is_trigger;
        settings.Friction = snapshot.material.friction.clamp(0.0, 10.0);
        settings.Restitution = snapshot.material.restitution.clamp(0.0, 1.0);
        settings.GravityFactor = 0.0;
        settings.Shape = shape as *const sys::JPC_Shape;

        let body_id = unsafe {
            sys::JPC_BodyInterface_CreateAndAddBody(
                body_interface,
                &settings,
                sys::JPC_ACTIVATION_ACTIVATE,
            )
        };

        if body_id == INVALID_BODY_ID {
            unsafe { sys::JPC_Shape_Release(shape as *const sys::JPC_Shape) };
            return Err(format!("Jolt CreateAndAddBody failed for collider {}", snapshot.entity));
        }

        Ok(BodyRecord::new(body_id, shape, signature, false))
    }

    fn apply_commands(&mut self, input: &PhysicsFrameInput) -> Result<usize, String> {
        let mut applied = 0usize;
        for command in &input.commands {
            match command.kind {
                PhysicsCommandKindDto::SetBodyPose { entity, position, rotation } => {
                    self.set_pose(entity, position, rotation);
                    applied += 1;
                }
                PhysicsCommandKindDto::SetLinearVelocity { entity, velocity } => {
                    self.set_linear_velocity(entity, velocity);
                    applied += 1;
                }
                PhysicsCommandKindDto::DestroyBody { entity } => {
                    self.destroy_body(entity);
                    applied += 1;
                }
            }
        }
        Ok(applied)
    }

    fn collect_body_outputs(&mut self, output: &mut PhysicsFrameOutput) {
        let Some(body_interface) = self.body_interface_mut().ok() else { return; };
        for (&entity, record) in &self.bodies {
            if !record.produces_output {
                continue;
            }
            let mut position = rvec3([0.0, 0.0, 0.0]);
            let mut rotation = quat([0.0, 0.0, 0.0, 1.0]);
            unsafe {
                sys::JPC_BodyInterface_GetPositionAndRotation(
                    body_interface,
                    record.body_id,
                    &mut position,
                    &mut rotation,
                );
            }
            output.pose_updates.push(PhysicsBodyPoseUpdate {
                entity,
                position: arr_from_rvec3(position),
                rotation: arr_from_quat(rotation),
            });

            let velocity = unsafe { sys::JPC_BodyInterface_GetLinearVelocity(body_interface, record.body_id) };
            output.velocity_updates.push(PhysicsBodyVelocityUpdate {
                entity,
                linear_velocity: arr_from_vec3(velocity),
            });
        }
    }

    fn execute_queries(&mut self, input: &PhysicsFrameInput) -> Vec<PhysicsQueryHitDto> {
        let mut hits = Vec::new();
        for query in &input.queries {
            match query.kind {
                PhysicsQueryKindDto::Ray { origin, dir, max_t } => {
                    if let Some(hit) = self.cast_ray(query.seq, origin, dir, max_t) {
                        hits.push(hit);
                    }
                }
                PhysicsQueryKindDto::Sphere { .. } | PhysicsQueryKindDto::Aabb { .. } => {}
            }
        }
        hits
    }

    fn cast_ray(
        &mut self,
        seq: u64,
        origin: [f32; 3],
        dir: [f32; 3],
        max_t: f32,
    ) -> Option<PhysicsQueryHitDto> {
        if max_t <= 0.0 {
            return None;
        }
        let system = self.world.system_raw();
        let query = unsafe { sys::JPC_PhysicsSystem_GetNarrowPhaseQuery(system as *const sys::JPC_PhysicsSystem) };
        if query.is_null() {
            return None;
        }

        let direction = [dir[0] * max_t, dir[1] * max_t, dir[2] * max_t];
        let mut args = sys::JPC_NarrowPhaseQuery_CastRayArgs {
            Ray: sys::JPC_RRayCast {
                Origin: rvec3(origin),
                Direction: vec3(direction),
            },
            Result: sys::JPC_RayCastResult {
                BodyID: INVALID_BODY_ID,
                Fraction: 0.0,
                SubShapeID2: 0,
            },
            BroadPhaseLayerFilter: ptr::null(),
            ObjectLayerFilter: ptr::null(),
            BodyFilter: ptr::null(),
        };

        let hit = unsafe { sys::JPC_NarrowPhaseQuery_CastRay(query, &mut args) };
        if !hit {
            return None;
        }
        let entity = self.body_to_entity.get(&args.Result.BodyID).copied()?;
        let fraction = args.Result.Fraction.clamp(0.0, 1.0);
        let distance = fraction * max_t;
        Some(PhysicsQueryHitDto {
            seq,
            entity,
            position: [
                origin[0] + direction[0] * fraction,
                origin[1] + direction[1] * fraction,
                origin[2] + direction[2] * fraction,
            ],
            normal: [0.0, 0.0, 0.0],
            distance,
        })
    }

    fn set_pose(&mut self, entity: u64, position: [f32; 3], rotation: [f32; 4]) {
        let Some(body_id) = self.bodies.get(&entity).map(|record| record.body_id) else { return; };
        let Ok(body_interface) = self.body_interface_mut() else { return; };
        unsafe {
            sys::JPC_BodyInterface_SetPositionAndRotation(
                body_interface,
                body_id,
                rvec3(position),
                quat(rotation),
                sys::JPC_ACTIVATION_ACTIVATE,
            );
        }
    }

    fn set_linear_velocity(&mut self, entity: u64, velocity: [f32; 3]) {
        let Some(body_id) = self.bodies.get(&entity).map(|record| record.body_id) else { return; };
        let Ok(body_interface) = self.body_interface_mut() else { return; };
        unsafe {
            sys::JPC_BodyInterface_SetLinearVelocity(body_interface, body_id, vec3(velocity));
        }
    }

    fn destroy_body(&mut self, entity: u64) {
        let Some(record) = self.bodies.remove(&entity) else { return; };
        self.body_to_entity.remove(&record.body_id);
        if let Ok(body_interface) = self.body_interface_mut() {
            unsafe {
                sys::JPC_BodyInterface_RemoveBody(body_interface, record.body_id);
                sys::JPC_BodyInterface_DestroyBody(body_interface, record.body_id);
            }
        }
        if !record.shape.is_null() {
            unsafe { sys::JPC_Shape_Release(record.shape as *const sys::JPC_Shape) };
        }
    }

    fn body_interface_mut(&mut self) -> Result<*mut sys::JPC_BodyInterface, String> {
        let interface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(self.world.system_raw()) };
        if interface.is_null() {
            Err("Jolt PhysicsSystem returned null BodyInterface".to_owned())
        } else {
            Ok(interface)
        }
    }
}

impl Drop for JoltPacketPhysicsBackend {
    fn drop(&mut self) { self.shutdown(); }
}

