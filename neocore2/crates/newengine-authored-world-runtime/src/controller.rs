use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use parking_lot::Mutex;

use crate::{
    prepare_authored_map_cell, AuthoredMapDefinitionCache, AuthoredMapStreamingSpec,
    PreparedAuthoredMapCell,
};

pub type AuthoredMapCellCoord = newengine_assets_api::MapCellCoordV1;

type CellLoadResult = Arc<Mutex<Option<Result<PreparedAuthoredMapCell, String>>>>;

struct CellLoadJob {
    ticket: TaskTicket,
    result: CellLoadResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthoredMapCellDomain {
    Render,
    Simulation,
}

impl AuthoredMapCellDomain {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Simulation => "simulation",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredMapStreamingRuntimeTuning {
    pub max_pending_jobs: usize,
    pub read_ahead_sec: f32,
    pub max_read_ahead_cells: i32,
    pub render_predict_radius: i32,
    pub simulation_predict_radius: i32,
    pub cell_jobs_limit: usize,
}

impl AuthoredMapStreamingRuntimeTuning {
    pub fn from_host_environment(spec: &AuthoredMapStreamingSpec) -> Self {
        let host = newengine_plugin_host::current_host_context();
        let var_usize = |name: &str, default: usize, min: usize, max: usize| {
            host.environment_var(name)
                .and_then(|value| value.trim().parse::<usize>().ok())
                .map(|value| value.clamp(min, max))
                .unwrap_or(default)
        };
        let var_i32 = |name: &str, default: i32, min: i32, max: i32| {
            host.environment_var(name)
                .and_then(|value| value.trim().parse::<i32>().ok())
                .map(|value| value.clamp(min, max))
                .unwrap_or(default)
        };
        let var_f32 = |name: &str, default: f32, min: f32, max: f32| {
            host.environment_var(name)
                .and_then(|value| value.trim().parse::<f32>().ok())
                .map(|value| value.clamp(min, max))
                .unwrap_or(default)
        };

        let default_max_read_ahead_cells = spec
            .render_radius
            .max(spec.simulation_radius)
            .saturating_add(1)
            .clamp(1, 4);
        Self {
            max_pending_jobs: var_usize(
                "NEWENGINE_AUTHORED_MAP_MAX_PENDING_JOBS",
                spec.max_cells_per_tick.saturating_mul(4).max(4),
                1,
                64,
            ),
            read_ahead_sec: var_f32("NEWENGINE_AUTHORED_MAP_READ_AHEAD_SEC", 0.75, 0.0, 3.0),
            max_read_ahead_cells: var_i32(
                "NEWENGINE_AUTHORED_MAP_MAX_READ_AHEAD_CELLS",
                default_max_read_ahead_cells,
                0,
                8,
            ),
            render_predict_radius: var_i32(
                "NEWENGINE_AUTHORED_MAP_PREDICT_RADIUS",
                1.min(spec.render_radius),
                0,
                spec.render_radius.max(0),
            ),
            simulation_predict_radius: var_i32(
                "NEWENGINE_AUTHORED_MAP_SIMULATION_PREDICT_RADIUS",
                1.min(spec.simulation_radius),
                0,
                spec.simulation_radius.max(0),
            ),
            cell_jobs_limit: var_usize(
                "NEWENGINE_AUTHORED_MAP_CELL_JOBS",
                spec.max_cells_per_tick.max(1).min(4),
                1,
                8,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredMapStreamingFocus {
    pub center: AuthoredMapCellCoord,
    pub predicted_center: AuthoredMapCellCoord,
    pub speed_mps: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthoredMapResidencyPlan {
    pub unload_render: Vec<AuthoredMapCellCoord>,
    pub unload_simulation: Vec<AuthoredMapCellCoord>,
}

#[derive(Clone, Debug)]
pub struct AuthoredPreparedCellAdmission {
    pub coord: AuthoredMapCellCoord,
    pub prepared: PreparedAuthoredMapCell,
    pub wants_render: bool,
    pub wants_simulation: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredMapStreamingDiagnostics {
    pub render_resident: usize,
    pub simulation_resident: usize,
    pub queued: usize,
    pub jobs: usize,
    pub ready: usize,
    pub prepared: usize,
    pub failed: usize,
}

pub struct AuthoredMapStreamingController {
    map_ref: String,
    logical_map_ref: String,
    index: newengine_assets_api::MapIndexV1,
    render_radius: i32,
    simulation_radius: i32,
    render_unload_radius: i32,
    simulation_unload_radius: i32,
    max_cells_per_tick: usize,
    tuning: AuthoredMapStreamingRuntimeTuning,
    render_cells: BTreeSet<AuthoredMapCellCoord>,
    simulation_cells: BTreeSet<AuthoredMapCellCoord>,
    desired_render: BTreeSet<AuthoredMapCellCoord>,
    desired_simulation: BTreeSet<AuthoredMapCellCoord>,
    resident_prepared: BTreeMap<AuthoredMapCellCoord, PreparedAuthoredMapCell>,
    pending_cells: VecDeque<AuthoredMapCellCoord>,
    pending_set: BTreeSet<AuthoredMapCellCoord>,
    load_jobs: BTreeMap<AuthoredMapCellCoord, CellLoadJob>,
    ready_cells: BTreeMap<AuthoredMapCellCoord, PreparedAuthoredMapCell>,
    failed_cells: BTreeMap<AuthoredMapCellCoord, String>,
    definition_cache: AuthoredMapDefinitionCache,
    last_center: Option<AuthoredMapCellCoord>,
    last_predicted_center: Option<AuthoredMapCellCoord>,
}

impl AuthoredMapStreamingController {
    pub fn new(spec: &AuthoredMapStreamingSpec, tuning: AuthoredMapStreamingRuntimeTuning) -> Self {
        let render_cells = spec
            .initial_render_cells
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let simulation_cells = spec
            .initial_simulation_cells
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let logical_map_ref = spec
            .map_ref
            .split('@')
            .next()
            .unwrap_or(&spec.map_ref)
            .to_owned();
        Self {
            map_ref: spec.map_ref.clone(),
            logical_map_ref,
            index: spec.index.clone(),
            render_radius: spec.render_radius,
            simulation_radius: spec.simulation_radius,
            render_unload_radius: spec.render_unload_radius,
            simulation_unload_radius: spec.simulation_unload_radius,
            max_cells_per_tick: spec.max_cells_per_tick.max(1),
            tuning,
            desired_render: render_cells.clone(),
            desired_simulation: simulation_cells.clone(),
            render_cells,
            simulation_cells,
            resident_prepared: BTreeMap::new(),
            pending_cells: VecDeque::new(),
            pending_set: BTreeSet::new(),
            load_jobs: BTreeMap::new(),
            ready_cells: BTreeMap::new(),
            failed_cells: BTreeMap::new(),
            definition_cache: AuthoredMapDefinitionCache::new(),
            last_center: None,
            last_predicted_center: None,
        }
    }

    #[inline]
    pub fn map_ref(&self) -> &str {
        &self.map_ref
    }

    #[inline]
    pub fn logical_map_ref(&self) -> &str {
        &self.logical_map_ref
    }

    #[inline]
    pub fn max_cells_per_tick(&self) -> usize {
        self.max_cells_per_tick
    }

    #[inline]
    pub fn read_ahead_sec(&self) -> f32 {
        self.tuning.read_ahead_sec
    }

    #[inline]
    pub fn max_read_ahead_cells(&self) -> i32 {
        self.tuning.max_read_ahead_cells
    }

    #[inline]
    pub fn focus_changed(&self, focus: AuthoredMapStreamingFocus) -> bool {
        self.last_center != Some(focus.center)
            || self.last_predicted_center != Some(focus.predicted_center)
    }

    #[inline]
    pub fn render_is_resident(&self, coord: AuthoredMapCellCoord) -> bool {
        self.render_cells.contains(&coord)
    }

    #[inline]
    pub fn simulation_is_resident(&self, coord: AuthoredMapCellCoord) -> bool {
        self.simulation_cells.contains(&coord)
    }

    #[inline]
    pub fn render_is_desired(&self, coord: AuthoredMapCellCoord) -> bool {
        self.desired_render.contains(&coord)
    }

    #[inline]
    pub fn simulation_is_desired(&self, coord: AuthoredMapCellCoord) -> bool {
        self.desired_simulation.contains(&coord)
    }

    #[inline]
    pub fn has_any_resident_domain(&self, coord: AuthoredMapCellCoord) -> bool {
        self.render_cells.contains(&coord) || self.simulation_cells.contains(&coord)
    }

    pub fn diagnostics(&self) -> AuthoredMapStreamingDiagnostics {
        AuthoredMapStreamingDiagnostics {
            render_resident: self.render_cells.len(),
            simulation_resident: self.simulation_cells.len(),
            queued: self.pending_cells.len(),
            jobs: self.load_jobs.len(),
            ready: self.ready_cells.len(),
            prepared: self.resident_prepared.len(),
            failed: self.failed_cells.len(),
        }
    }

    pub fn focus_for_world_motion(
        &self,
        position: [f32; 3],
        velocity: [f32; 3],
    ) -> Option<AuthoredMapStreamingFocus> {
        let center = self.index.world_to_cell(position)?;
        let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
        let predicted_position = [
            position[0] + velocity[0] * self.tuning.read_ahead_sec,
            position[1],
            position[2] + velocity[2] * self.tuning.read_ahead_sec,
        ];
        let raw_prediction = self
            .index
            .world_to_cell(predicted_position)
            .unwrap_or(center);
        let dx = (raw_prediction.x - center.x).clamp(
            -self.tuning.max_read_ahead_cells,
            self.tuning.max_read_ahead_cells,
        );
        let dz = (raw_prediction.z - center.z).clamp(
            -self.tuning.max_read_ahead_cells,
            self.tuning.max_read_ahead_cells,
        );
        Some(AuthoredMapStreamingFocus {
            center,
            predicted_center: AuthoredMapCellCoord::new(
                center.x.saturating_add(dx),
                center.z.saturating_add(dz),
            ),
            speed_mps: speed,
        })
    }

    pub fn replan(&mut self, focus: AuthoredMapStreamingFocus) -> AuthoredMapResidencyPlan {
        let (desired_render, desired_simulation) =
            self.desired_domains(focus.center, focus.predicted_center);
        self.desired_render = desired_render;
        self.desired_simulation = desired_simulation;

        let plan = AuthoredMapResidencyPlan {
            unload_render: self
                .render_cells
                .iter()
                .copied()
                .filter(|coord| cell_distance(*coord, focus.center) > self.render_unload_radius)
                .collect(),
            unload_simulation: self
                .simulation_cells
                .iter()
                .copied()
                .filter(|coord| cell_distance(*coord, focus.center) > self.simulation_unload_radius)
                .collect(),
        };

        let stale_jobs = self
            .load_jobs
            .keys()
            .copied()
            .filter(|coord| !self.cell_is_desired(*coord))
            .collect::<Vec<_>>();
        for coord in stale_jobs {
            if let Some(job) = self.load_jobs.remove(&coord) {
                let _ = job.ticket.cancel();
            }
        }

        let desired_render = &self.desired_render;
        let desired_simulation = &self.desired_simulation;
        self.ready_cells.retain(|coord, _| {
            desired_render.contains(coord) || desired_simulation.contains(coord)
        });
        self.failed_cells.retain(|coord, _| {
            desired_render.contains(coord) || desired_simulation.contains(coord)
        });
        self.pending_cells
            .retain(|coord| desired_render.contains(coord) || desired_simulation.contains(coord));
        self.pending_set = self.pending_cells.iter().copied().collect();

        let mut candidates = self
            .desired_render
            .union(&self.desired_simulation)
            .copied()
            .filter(|coord| self.cell_needs_prepare(*coord))
            .filter(|coord| !self.resident_prepared.contains_key(coord))
            .filter(|coord| !self.pending_set.contains(coord))
            .filter(|coord| !self.load_jobs.contains_key(coord))
            .filter(|coord| !self.ready_cells.contains_key(coord))
            .collect::<Vec<_>>();
        for coord in &candidates {
            self.failed_cells.remove(coord);
        }
        candidates.sort_by_key(|coord| {
            self.prepared_priority(*coord, focus.center, focus.predicted_center)
        });
        for coord in candidates {
            if self.pending_set.insert(coord) {
                self.pending_cells.push_back(coord);
            }
        }

        self.last_center = Some(focus.center);
        self.last_predicted_center = Some(focus.predicted_center);
        plan
    }

    pub fn process_preparation(&mut self, thread_pool: Option<&ThreadPoolHandle>) {
        self.poll_cell_jobs();
        if let Some(thread_pool) = thread_pool {
            self.submit_cell_jobs(thread_pool);
            self.poll_cell_jobs();
        } else {
            self.prepare_cells_synchronously();
        }
    }

    pub fn take_next_prepared(&mut self) -> Option<AuthoredPreparedCellAdmission> {
        let center = self.last_center?;
        let predicted_center = self.last_predicted_center.unwrap_or(center);
        let coord = self
            .resident_prepared
            .keys()
            .chain(self.ready_cells.keys())
            .copied()
            .filter(|coord| self.cell_needs_prepare(*coord))
            .min_by_key(|coord| self.prepared_priority(*coord, center, predicted_center))?;

        let prepared = if let Some(existing) = self.resident_prepared.get(&coord).cloned() {
            existing
        } else {
            let ready = self.ready_cells.remove(&coord)?;
            self.resident_prepared.insert(coord, ready.clone());
            ready
        };
        Some(AuthoredPreparedCellAdmission {
            coord,
            wants_render: self.render_is_desired(coord) && !self.render_is_resident(coord),
            wants_simulation: self.simulation_is_desired(coord)
                && !self.simulation_is_resident(coord),
            prepared,
        })
    }

    pub fn mark_domain_resident(
        &mut self,
        coord: AuthoredMapCellCoord,
        domain: AuthoredMapCellDomain,
    ) {
        match domain {
            AuthoredMapCellDomain::Render => {
                self.render_cells.insert(coord);
            }
            AuthoredMapCellDomain::Simulation => {
                self.simulation_cells.insert(coord);
            }
        }
    }

    pub fn mark_domain_unloaded(
        &mut self,
        coord: AuthoredMapCellCoord,
        domain: AuthoredMapCellDomain,
    ) {
        match domain {
            AuthoredMapCellDomain::Render => {
                self.render_cells.remove(&coord);
            }
            AuthoredMapCellDomain::Simulation => {
                self.simulation_cells.remove(&coord);
            }
        }
        if !self.has_any_resident_domain(coord) {
            self.resident_prepared.remove(&coord);
        }
    }

    fn desired_domains(
        &self,
        center: AuthoredMapCellCoord,
        predicted_center: AuthoredMapCellCoord,
    ) -> (
        BTreeSet<AuthoredMapCellCoord>,
        BTreeSet<AuthoredMapCellCoord>,
    ) {
        let mut render = BTreeSet::new();
        let mut simulation = BTreeSet::new();
        append_existing_cells(&self.index, center, self.render_radius, &mut render);
        append_existing_cells(&self.index, center, self.simulation_radius, &mut simulation);

        if predicted_center != center {
            if self.tuning.render_predict_radius > 0 {
                let mut predicted = BTreeSet::new();
                append_existing_cells(
                    &self.index,
                    predicted_center,
                    self.tuning.render_predict_radius,
                    &mut predicted,
                );
                render.extend(
                    predicted
                        .into_iter()
                        .filter(|coord| cell_distance(*coord, center) <= self.render_unload_radius),
                );
            }
            if self.tuning.simulation_predict_radius > 0 {
                let mut predicted = BTreeSet::new();
                append_existing_cells(
                    &self.index,
                    predicted_center,
                    self.tuning.simulation_predict_radius,
                    &mut predicted,
                );
                simulation.extend(predicted.into_iter().filter(|coord| {
                    cell_distance(*coord, center) <= self.simulation_unload_radius
                }));
            }
        }

        (render, simulation)
    }

    #[inline]
    fn cell_is_desired(&self, coord: AuthoredMapCellCoord) -> bool {
        self.desired_render.contains(&coord) || self.desired_simulation.contains(&coord)
    }

    #[inline]
    fn cell_needs_prepare(&self, coord: AuthoredMapCellCoord) -> bool {
        (self.desired_render.contains(&coord) && !self.render_cells.contains(&coord))
            || (self.desired_simulation.contains(&coord) && !self.simulation_cells.contains(&coord))
    }

    fn prepared_priority(
        &self,
        coord: AuthoredMapCellCoord,
        center: AuthoredMapCellCoord,
        predicted_center: AuthoredMapCellCoord,
    ) -> (usize, i32, i32, i32, i32) {
        let simulation_needed =
            self.desired_simulation.contains(&coord) && !self.simulation_cells.contains(&coord);
        let primary_distance = cell_distance(coord, center);
        let predicted_distance = cell_distance(coord, predicted_center);
        (
            usize::from(!simulation_needed),
            primary_distance.min(predicted_distance),
            primary_distance,
            coord.x,
            coord.z,
        )
    }

    fn cell_load_concurrency(&self, thread_pool: &ThreadPoolHandle) -> usize {
        thread_pool
            .worker_threads()
            .saturating_sub(1)
            .max(1)
            .min(self.tuning.cell_jobs_limit)
            .min(self.tuning.max_pending_jobs)
    }

    fn submit_cell_jobs(&mut self, thread_pool: &ThreadPoolHandle) {
        let concurrency = self.cell_load_concurrency(thread_pool);
        let free_slots = concurrency.saturating_sub(self.load_jobs.len());
        for _ in 0..free_slots {
            let Some(coord) = self.pending_cells.pop_front() else {
                break;
            };
            self.pending_set.remove(&coord);
            if !self.cell_needs_prepare(coord)
                || self.resident_prepared.contains_key(&coord)
                || self.load_jobs.contains_key(&coord)
                || self.ready_cells.contains_key(&coord)
            {
                continue;
            }

            let map_ref = self.map_ref.clone();
            let logical_map_ref = self.logical_map_ref.clone();
            let definition_cache = self.definition_cache.clone();
            let result = Arc::new(Mutex::new(None));
            let result_out = Arc::clone(&result);
            let request = TaskRequest::new("authored.map.cell.prepare")
                .with_source("engine.authored-world.streaming")
                .with_owner("engine.authored-world")
                .with_category("world-streaming")
                .with_lane(TaskLane::AssetIo)
                .with_priority(TaskPriority::Interactive)
                .with_task_id(format!(
                    "authored-world.cell.{}.{}.{:016x}",
                    coord.x,
                    coord.z,
                    newengine_primitives::fnv1a_64(&map_ref),
                ));
            let host_context = newengine_plugin_host::current_host_context();
            let ticket = thread_pool.submit_request(request, move || {
                let prepared = newengine_plugin_host::with_host_context(&host_context, || {
                    prepare_authored_map_cell(&map_ref, &logical_map_ref, coord, &definition_cache)
                });
                *result_out.lock() = Some(prepared);
            });
            self.load_jobs.insert(coord, CellLoadJob { ticket, result });
        }
    }

    fn poll_cell_jobs(&mut self) {
        let complete = self
            .load_jobs
            .iter()
            .filter(|(_, job)| job.ticket.is_complete())
            .map(|(coord, _)| *coord)
            .collect::<Vec<_>>();
        for coord in complete {
            let Some(job) = self.load_jobs.remove(&coord) else {
                continue;
            };
            let job_result = job.result.lock().take();
            match job_result {
                Some(Ok(prepared)) if self.cell_needs_prepare(coord) => {
                    self.failed_cells.remove(&coord);
                    self.ready_cells.insert(coord, prepared);
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    self.failed_cells.insert(coord, error);
                }
                None => {
                    self.failed_cells.insert(
                        coord,
                        "authored map cell task completed without result".to_owned(),
                    );
                }
            }
        }
    }

    fn prepare_cells_synchronously(&mut self) {
        let budget = self.max_cells_per_tick;
        for _ in 0..budget {
            if self.ready_cells.len() >= budget {
                break;
            }
            let Some(coord) = self.pending_cells.pop_front() else {
                break;
            };
            self.pending_set.remove(&coord);
            if !self.cell_needs_prepare(coord) || self.resident_prepared.contains_key(&coord) {
                continue;
            }
            match prepare_authored_map_cell(
                &self.map_ref,
                &self.logical_map_ref,
                coord,
                &self.definition_cache,
            ) {
                Ok(prepared) => {
                    self.failed_cells.remove(&coord);
                    self.ready_cells.insert(coord, prepared);
                }
                Err(error) => {
                    self.failed_cells.insert(coord, error);
                }
            }
        }
    }
}

#[inline]
fn cell_distance(a: AuthoredMapCellCoord, b: AuthoredMapCellCoord) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

fn append_existing_cells(
    index: &newengine_assets_api::MapIndexV1,
    center: AuthoredMapCellCoord,
    radius: i32,
    output: &mut BTreeSet<AuthoredMapCellCoord>,
) {
    let radius = radius.max(0);
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let coord =
                AuthoredMapCellCoord::new(center.x.saturating_add(dx), center.z.saturating_add(dz));
            if index.cell(coord).is_some() {
                output.insert(coord);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_with_cells(coords: &[(i32, i32)]) -> newengine_assets_api::MapIndexV1 {
        let mut index = newengine_assets_api::MapIndexV1 {
            map_id: "test".to_owned(),
            cell_size: 64.0,
            cells: coords
                .iter()
                .map(|(x, z)| {
                    newengine_assets_api::MapCellRefV1::canonical(AuthoredMapCellCoord::new(*x, *z))
                })
                .collect(),
            ..Default::default()
        };
        index.normalize();
        index
    }

    fn spec_with_cells(coords: &[(i32, i32)]) -> AuthoredMapStreamingSpec {
        AuthoredMapStreamingSpec {
            map_ref: "maps/test.ymap@map".to_owned(),
            index: index_with_cells(coords),
            initial_render_cells: vec![AuthoredMapCellCoord::new(0, 0)],
            initial_simulation_cells: vec![AuthoredMapCellCoord::new(0, 0)],
            initial_placement_ids: BTreeMap::new(),
            render_radius: 2,
            simulation_radius: 1,
            render_unload_radius: 3,
            simulation_unload_radius: 2,
            max_cells_per_tick: 2,
        }
    }

    fn deterministic_tuning() -> AuthoredMapStreamingRuntimeTuning {
        AuthoredMapStreamingRuntimeTuning {
            max_pending_jobs: 4,
            read_ahead_sec: 0.75,
            max_read_ahead_cells: 2,
            render_predict_radius: 1,
            simulation_predict_radius: 1,
            cell_jobs_limit: 2,
        }
    }

    #[test]
    fn desired_cell_generation_scales_with_radius_not_world_cell_count() {
        let spec = spec_with_cells(&[(-100, -100), (-1, -1), (0, 0), (1, 0), (1, 1), (100, 100)]);
        let controller = AuthoredMapStreamingController::new(&spec, deterministic_tuning());
        let (render, _) = controller.desired_domains(
            AuthoredMapCellCoord::new(0, 0),
            AuthoredMapCellCoord::new(0, 0),
        );
        assert!(render.contains(&AuthoredMapCellCoord::new(0, 0)));
        assert!(render.contains(&AuthoredMapCellCoord::new(1, 0)));
        assert!(render.contains(&AuthoredMapCellCoord::new(1, 1)));
        assert!(!render.contains(&AuthoredMapCellCoord::new(100, 100)));
    }

    #[test]
    fn controller_owns_prediction_and_dual_domain_desire() {
        let spec = spec_with_cells(&[(-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0), (0, 1)]);
        let mut controller = AuthoredMapStreamingController::new(&spec, deterministic_tuning());
        let focus = controller
            .focus_for_world_motion([1.0, 0.0, 1.0], [96.0, 0.0, 0.0])
            .expect("focus");
        assert_eq!(focus.center, AuthoredMapCellCoord::new(0, 0));
        assert!(focus.predicted_center.x >= focus.center.x);
        controller.replan(focus);
        assert!(controller.render_is_desired(AuthoredMapCellCoord::new(1, 0)));
        assert!(controller.simulation_is_desired(AuthoredMapCellCoord::new(0, 1)));
    }

    #[test]
    fn controller_emits_unload_plan_without_touching_ecs() {
        let mut spec = spec_with_cells(&[(-3, 0), (3, 0)]);
        spec.initial_render_cells = vec![AuthoredMapCellCoord::new(-3, 0)];
        spec.initial_simulation_cells = vec![AuthoredMapCellCoord::new(-3, 0)];
        let mut controller = AuthoredMapStreamingController::new(&spec, deterministic_tuning());
        let focus = AuthoredMapStreamingFocus {
            center: AuthoredMapCellCoord::new(3, 0),
            predicted_center: AuthoredMapCellCoord::new(3, 0),
            speed_mps: 0.0,
        };
        let plan = controller.replan(focus);
        assert_eq!(plan.unload_render, vec![AuthoredMapCellCoord::new(-3, 0)]);
        assert_eq!(
            plan.unload_simulation,
            vec![AuthoredMapCellCoord::new(-3, 0)]
        );
    }
}
