use std::collections::{BTreeMap, BTreeSet};

use super::{
    CompiledRenderGraphPass, CompiledResourceLifetime, RenderGraphBarrierStats,
    RenderGraphCompilation, RenderGraphCompileReport, RenderGraphCompiledDag,
    RenderGraphCullingReport, RenderGraphDependencyEdge, RenderGraphDependencyKind,
    RenderGraphDesc, RenderGraphLifetimeStats, RenderGraphPassId, RenderGraphResourceId,
    RenderGraphResourceLifetime, RenderGraphResourceLifetimeReport, RenderGraphResourceUse,
    RenderGraphResourceUseKind, RenderGraphValidationIssue, RenderGraphValidationReport,
};

pub fn validate_and_compile_render_graph(graph: &RenderGraphDesc) -> RenderGraphValidationReport {
    match compile_render_graph_v2(graph) {
        Ok(compilation) => RenderGraphValidationReport {
            ok: true,
            errors: Vec::new(),
            warnings: compilation.report.warnings.clone(),
            compile: Some(compilation.report),
        },
        Err(errors) => RenderGraphValidationReport {
            ok: false,
            errors,
            warnings: Vec::new(),
            compile: None,
        },
    }
}

/// Compatibility entry point for existing render providers.
///
/// New frame-graph code should use [`compile_render_graph_v2`] when it needs the
/// compiled dependency DAG in addition to the summary report.
pub fn compile_render_graph(
    graph: &RenderGraphDesc,
) -> Result<RenderGraphCompileReport, Vec<RenderGraphValidationIssue>> {
    compile_render_graph_v2(graph).map(|compilation| compilation.report)
}

/// Compiles the declarative render graph into a validated dependency DAG.
///
/// Phase 3 computes live resource lifetime intervals after conservative pass culling.
/// Phase 4 then derives a deterministic transient allocation/alias plan from those
/// intervals. Backend allocation, barrier generation and queue scheduling remain later
/// phases. Raw hazard edges are retained for diagnostics and later compilers.
pub fn compile_render_graph_v2(
    graph: &RenderGraphDesc,
) -> Result<RenderGraphCompilation, Vec<RenderGraphValidationIssue>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut resources = BTreeMap::new();
    let mut pass_ids = BTreeSet::new();
    let mut pass_order = BTreeMap::new();
    let mut lifetime = RenderGraphLifetimeStats::default();

    for resource in &graph.resources {
        if resources.insert(resource.id, resource).is_some() {
            errors.push(
                RenderGraphValidationIssue::new(
                    "render_graph.duplicate_resource",
                    "render graph contains duplicate resource id",
                )
                .with_resource(resource.id),
            );
        }
        match resource.lifetime {
            RenderGraphResourceLifetime::Persistent | RenderGraphResourceLifetime::External => {
                lifetime.persistent = lifetime.persistent.saturating_add(1);
            }
            RenderGraphResourceLifetime::TransientFrame
            | RenderGraphResourceLifetime::Frames(_) => {
                lifetime.transient = lifetime.transient.saturating_add(1);
            }
        }
    }

    let mut creator_by_resource = BTreeMap::<RenderGraphResourceId, RenderGraphPassId>::new();

    for (index, pass) in graph.passes.iter().enumerate() {
        if !pass_ids.insert(pass.id) {
            errors.push(
                RenderGraphValidationIssue::new(
                    "render_graph.duplicate_pass",
                    "render graph contains duplicate pass id",
                )
                .with_pass(pass.id),
            );
        } else {
            pass_order.insert(pass.id, index);
        }

        for created in &pass.creates {
            if !resources.contains_key(created) {
                errors.push(
                    RenderGraphValidationIssue::new(
                        "render_graph.unknown_created_resource",
                        "render graph pass creates a resource that is not declared",
                    )
                    .with_pass(pass.id)
                    .with_resource(*created),
                );
                continue;
            }
            if let Some(previous) = creator_by_resource.insert(*created, pass.id) {
                errors.push(
                    RenderGraphValidationIssue::new(
                        "render_graph.duplicate_create",
                        format!(
                            "render graph resource is created by more than one pass (previous producer {:?})",
                            previous
                        ),
                    )
                    .with_pass(pass.id)
                    .with_resource(*created),
                );
            }
        }

        for access in pass.reads.iter().chain(pass.writes.iter()) {
            if !resources.contains_key(&access.resource) {
                errors.push(
                    RenderGraphValidationIssue::new(
                        "render_graph.unknown_resource",
                        "render graph pass references a resource that is not declared",
                    )
                    .with_pass(pass.id)
                    .with_resource(access.resource),
                );
            }
        }

        for draw_list in &pass.draw_lists {
            if !draw_list.is_compatible_with_pass(pass.kind) {
                warnings.push(
                    RenderGraphValidationIssue::new(
                        "render_graph.draw_list_route_mismatch",
                        format!(
                            "draw-list '{}' is unusual for render pass kind {:?}",
                            draw_list.label(),
                            pass.kind
                        ),
                    )
                    .with_pass(pass.id),
                );
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut last_writer = BTreeMap::<RenderGraphResourceId, RenderGraphPassId>::new();
    let mut last_readers = BTreeMap::<RenderGraphResourceId, BTreeSet<RenderGraphPassId>>::new();
    let mut dependencies: BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>> = graph
        .passes
        .iter()
        .map(|pass| (pass.id, BTreeSet::new()))
        .collect();
    let mut reverse_edges: BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>> = graph
        .passes
        .iter()
        .map(|pass| (pass.id, BTreeSet::new()))
        .collect();
    let mut edge_keys = BTreeSet::new();
    let mut edges = Vec::new();
    let mut barriers = RenderGraphBarrierStats::default();

    for pass in &graph.passes {
        for created in &pass.creates {
            if let Some(previous_writer) = last_writer.get(created).copied() {
                if previous_writer != pass.id {
                    errors.push(
                        RenderGraphValidationIssue::new(
                            "render_graph.recreate_resource",
                            "render graph resource is created after it was already produced",
                        )
                        .with_pass(pass.id)
                        .with_resource(*created),
                    );
                }
            }
            last_writer.insert(*created, pass.id);
            last_readers.remove(created);
        }

        for read in &pass.reads {
            if let Some(writer) = last_writer.get(&read.resource).copied() {
                if writer != pass.id {
                    add_dependency(
                        writer,
                        pass.id,
                        read.resource,
                        RenderGraphDependencyKind::ReadAfterWrite,
                        &mut dependencies,
                        &mut reverse_edges,
                        &mut edge_keys,
                        &mut edges,
                    );
                    barriers.read_after_write = barriers.read_after_write.saturating_add(1);
                }
            } else {
                let resource = resources
                    .get(&read.resource)
                    .expect("validated render graph resource disappeared");
                if matches!(
                    resource.lifetime,
                    RenderGraphResourceLifetime::TransientFrame
                ) {
                    errors.push(
                        RenderGraphValidationIssue::new(
                            "render_graph.missing_producer",
                            "transient render graph resource is read before any pass produces it",
                        )
                        .with_pass(pass.id)
                        .with_resource(read.resource),
                    );
                } else {
                    barriers.external_imports = barriers.external_imports.saturating_add(1);
                }
            }
            last_readers
                .entry(read.resource)
                .or_default()
                .insert(pass.id);
        }

        for write in &pass.writes {
            if let Some(writer) = last_writer.get(&write.resource).copied() {
                if writer != pass.id {
                    add_dependency(
                        writer,
                        pass.id,
                        write.resource,
                        RenderGraphDependencyKind::WriteAfterWrite,
                        &mut dependencies,
                        &mut reverse_edges,
                        &mut edge_keys,
                        &mut edges,
                    );
                    barriers.write_after_write = barriers.write_after_write.saturating_add(1);
                }
            }
            if let Some(readers) = last_readers.remove(&write.resource) {
                for reader in readers {
                    if reader != pass.id {
                        add_dependency(
                            reader,
                            pass.id,
                            write.resource,
                            RenderGraphDependencyKind::WriteAfterRead,
                            &mut dependencies,
                            &mut reverse_edges,
                            &mut edge_keys,
                            &mut edges,
                        );
                        barriers.write_after_read = barriers.write_after_read.saturating_add(1);
                    }
                }
            }
            last_writer.insert(write.resource, pass.id);
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let raw_execution_order = topological_order(&dependencies, &reverse_edges, &pass_order)?;
    let culling = cull_passes(
        graph,
        &dependencies,
        &last_writer,
        &resources,
        &raw_execution_order,
    );
    let culled = culling
        .culled_passes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let execution_order = culling.live_passes.clone();
    let resource_lifetimes = analyze_resource_lifetimes(graph, &execution_order);
    let transient_allocation_plan =
        super::plan_transient_resource_allocations(graph, &resource_lifetimes);
    let compiled_passes = graph
        .passes
        .iter()
        .enumerate()
        .map(|(index, pass)| CompiledRenderGraphPass {
            id: pass.id,
            declaration_index: index as u32,
            producers: dependencies
                .get(&pass.id)
                .map(|items| items.iter().copied().collect())
                .unwrap_or_default(),
            consumers: reverse_edges
                .get(&pass.id)
                .map(|items| items.iter().copied().collect())
                .unwrap_or_default(),
            culled: culled.contains(&pass.id),
        })
        .collect();

    let dag = RenderGraphCompiledDag {
        passes: compiled_passes,
        edges,
        execution_order: execution_order.clone(),
    };
    let report = RenderGraphCompileReport {
        pass_count: graph.passes.len() as u32,
        resource_count: graph.resources.len() as u32,
        execution_order,
        lifetime,
        barriers,
        warnings,
    };

    Ok(RenderGraphCompilation {
        dag,
        report,
        culling,
        resource_lifetimes,
        transient_allocation_plan,
    })
}

#[derive(Clone)]
struct ResourceLifetimeAccumulator {
    first_pass: RenderGraphPassId,
    last_pass: RenderGraphPassId,
    first_execution_index: u32,
    last_execution_index: u32,
    create_count: u32,
    read_count: u32,
    write_count: u32,
    history: Vec<RenderGraphResourceUse>,
}

impl ResourceLifetimeAccumulator {
    #[inline]
    fn new(pass: RenderGraphPassId, execution_index: u32) -> Self {
        Self {
            first_pass: pass,
            last_pass: pass,
            first_execution_index: execution_index,
            last_execution_index: execution_index,
            create_count: 0,
            read_count: 0,
            write_count: 0,
            history: Vec::new(),
        }
    }

    #[inline]
    fn record(
        &mut self,
        pass: RenderGraphPassId,
        execution_index: u32,
        kind: RenderGraphResourceUseKind,
        usage: Option<super::RenderGraphResourceUsage>,
    ) {
        self.last_pass = pass;
        self.last_execution_index = execution_index;
        match kind {
            RenderGraphResourceUseKind::Create => {
                self.create_count = self.create_count.saturating_add(1)
            }
            RenderGraphResourceUseKind::Read => self.read_count = self.read_count.saturating_add(1),
            RenderGraphResourceUseKind::Write => {
                self.write_count = self.write_count.saturating_add(1)
            }
        }
        self.history.push(RenderGraphResourceUse::new(
            execution_index,
            pass,
            kind,
            usage,
        ));
    }
}

fn analyze_resource_lifetimes(
    graph: &RenderGraphDesc,
    live_execution_order: &[RenderGraphPassId],
) -> RenderGraphResourceLifetimeReport {
    let passes = graph
        .passes
        .iter()
        .map(|pass| (pass.id, pass))
        .collect::<BTreeMap<_, _>>();
    let mut accumulators = BTreeMap::<RenderGraphResourceId, ResourceLifetimeAccumulator>::new();

    for (execution_index, pass_id) in live_execution_order.iter().copied().enumerate() {
        let execution_index = execution_index.min(u32::MAX as usize) as u32;
        let pass = passes
            .get(&pass_id)
            .expect("live render graph pass missing from declarative graph");

        for resource in &pass.creates {
            let entry = accumulators
                .entry(*resource)
                .or_insert_with(|| ResourceLifetimeAccumulator::new(pass_id, execution_index));
            entry.record(
                pass_id,
                execution_index,
                RenderGraphResourceUseKind::Create,
                None,
            );
        }
        for read in &pass.reads {
            let entry = accumulators
                .entry(read.resource)
                .or_insert_with(|| ResourceLifetimeAccumulator::new(pass_id, execution_index));
            entry.record(
                pass_id,
                execution_index,
                RenderGraphResourceUseKind::Read,
                Some(read.usage),
            );
        }
        for write in &pass.writes {
            let entry = accumulators
                .entry(write.resource)
                .or_insert_with(|| ResourceLifetimeAccumulator::new(pass_id, execution_index));
            entry.record(
                pass_id,
                execution_index,
                RenderGraphResourceUseKind::Write,
                Some(write.usage),
            );
        }
    }

    let mut resources = Vec::new();
    let mut unused_resources = Vec::new();
    for resource in &graph.resources {
        if let Some(entry) = accumulators.remove(&resource.id) {
            resources.push(CompiledResourceLifetime {
                resource: resource.id,
                first_pass: entry.first_pass,
                last_pass: entry.last_pass,
                first_execution_index: entry.first_execution_index,
                last_execution_index: entry.last_execution_index,
                create_count: entry.create_count,
                read_count: entry.read_count,
                write_count: entry.write_count,
                history: entry.history,
            });
        } else {
            unused_resources.push(resource.id);
        }
    }

    RenderGraphResourceLifetimeReport {
        resources,
        unused_resources,
    }
}

fn cull_passes(
    graph: &RenderGraphDesc,
    dependencies: &BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>>,
    last_writer: &BTreeMap<RenderGraphResourceId, RenderGraphPassId>,
    resources: &BTreeMap<RenderGraphResourceId, &super::RenderGraphResourceDesc>,
    raw_execution_order: &[RenderGraphPassId],
) -> RenderGraphCullingReport {
    let mut root_set = BTreeSet::new();

    for pass in &graph.passes {
        if !pass.flags.allow_culling {
            root_set.insert(pass.id);
        }
    }

    for resource in resources.values() {
        let observable = !matches!(
            resource.lifetime,
            RenderGraphResourceLifetime::TransientFrame
        ) || resource.semantic.is_surface_color();
        if observable {
            if let Some(writer) = last_writer.get(&resource.id).copied() {
                root_set.insert(writer);
            }
        }
    }

    let roots = raw_execution_order
        .iter()
        .copied()
        .filter(|pass| root_set.contains(pass))
        .collect::<Vec<_>>();
    let mut live = root_set;
    let mut stack = roots.clone();

    while let Some(pass) = stack.pop() {
        if let Some(producers) = dependencies.get(&pass) {
            for producer in producers {
                if live.insert(*producer) {
                    stack.push(*producer);
                }
            }
        }
    }

    let live_passes = raw_execution_order
        .iter()
        .copied()
        .filter(|pass| live.contains(pass))
        .collect::<Vec<_>>();
    let culled_passes = raw_execution_order
        .iter()
        .copied()
        .filter(|pass| !live.contains(pass))
        .collect::<Vec<_>>();

    RenderGraphCullingReport {
        roots,
        live_passes,
        culled_passes,
    }
}

#[allow(clippy::too_many_arguments)]
fn add_dependency(
    producer: RenderGraphPassId,
    consumer: RenderGraphPassId,
    resource: RenderGraphResourceId,
    kind: RenderGraphDependencyKind,
    dependencies: &mut BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>>,
    reverse_edges: &mut BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>>,
    edge_keys: &mut BTreeSet<(
        RenderGraphPassId,
        RenderGraphPassId,
        RenderGraphResourceId,
        RenderGraphDependencyKind,
    )>,
    edges: &mut Vec<RenderGraphDependencyEdge>,
) {
    dependencies.entry(consumer).or_default().insert(producer);
    reverse_edges.entry(producer).or_default().insert(consumer);
    if edge_keys.insert((producer, consumer, resource, kind)) {
        edges.push(RenderGraphDependencyEdge {
            producer,
            consumer,
            resource,
            kind,
        });
    }
}

fn topological_order(
    dependencies: &BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>>,
    reverse_edges: &BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>>,
    declaration_order: &BTreeMap<RenderGraphPassId, usize>,
) -> Result<Vec<RenderGraphPassId>, Vec<RenderGraphValidationIssue>> {
    let mut indegree: BTreeMap<RenderGraphPassId, usize> = dependencies
        .iter()
        .map(|(pass, deps)| (*pass, deps.len()))
        .collect();
    let mut ready = BTreeSet::<(usize, RenderGraphPassId)>::new();

    for (pass, count) in &indegree {
        if *count == 0 {
            ready.insert((*declaration_order.get(pass).unwrap_or(&usize::MAX), *pass));
        }
    }

    let mut execution_order = Vec::with_capacity(indegree.len());
    while let Some(&(index, pass)) = ready.iter().next() {
        ready.remove(&(index, pass));
        execution_order.push(pass);

        if let Some(consumers) = reverse_edges.get(&pass) {
            for consumer in consumers {
                let count = indegree
                    .get_mut(consumer)
                    .expect("dependency graph consumer missing from indegree map");
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ready.insert((
                        *declaration_order.get(consumer).unwrap_or(&usize::MAX),
                        *consumer,
                    ));
                }
            }
        }
    }

    if execution_order.len() != indegree.len() {
        let blocked = indegree
            .into_iter()
            .filter_map(|(pass, count)| (count > 0).then_some(format!("{:?}", pass)))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(vec![RenderGraphValidationIssue::new(
            "render_graph.cycle",
            format!("render graph has a dependency cycle involving passes: {blocked}"),
        )]);
    }

    Ok(execution_order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RenderGraphPassDesc, RenderGraphPassKind, RenderGraphResourceDesc, RenderGraphResourceUsage,
    };

    fn transient(id: u64) -> RenderGraphResourceDesc {
        RenderGraphResourceDesc::transient_texture(
            RenderGraphResourceId(id),
            format!("r{id}"),
            RenderGraphResourceUsage::StorageTexture,
            crate::Extent2D::new(8, 8),
            crate::TextureFormat::Rgba8Unorm,
        )
    }

    fn pass(id: u64, label: &str) -> RenderGraphPassDesc {
        RenderGraphPassDesc::new(RenderGraphPassId(id), label, RenderGraphPassKind::Custom)
    }

    #[test]
    fn compiles_raw_war_and_waw_edges_with_stable_declaration_order() {
        let r1 = RenderGraphResourceId(1);
        let r2 = RenderGraphResourceId(2);
        let graph = RenderGraphDesc::new("hazards")
            .add_resource(transient(1))
            .add_resource(transient(2))
            .add_pass(pass(30, "produce-a").writes(r1, RenderGraphResourceUsage::StorageTexture))
            .add_pass(pass(10, "read-a").reads(r1, RenderGraphResourceUsage::SampledTexture))
            .add_pass(
                pass(20, "rewrite-a")
                    .writes(r1, RenderGraphResourceUsage::StorageTexture)
                    .writes(r2, RenderGraphResourceUsage::StorageTexture),
            )
            .add_pass(pass(5, "read-b").reads(r2, RenderGraphResourceUsage::SampledTexture));

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        assert_eq!(
            compiled.dag.execution_order,
            vec![
                RenderGraphPassId(30),
                RenderGraphPassId(10),
                RenderGraphPassId(20),
                RenderGraphPassId(5),
            ]
        );
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == RenderGraphPassId(30)
                && edge.consumer == RenderGraphPassId(10)
                && edge.kind == RenderGraphDependencyKind::ReadAfterWrite
        }));
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == RenderGraphPassId(10)
                && edge.consumer == RenderGraphPassId(20)
                && edge.kind == RenderGraphDependencyKind::WriteAfterRead
        }));
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == RenderGraphPassId(30)
                && edge.consumer == RenderGraphPassId(20)
                && edge.kind == RenderGraphDependencyKind::WriteAfterWrite
        }));
    }

    #[test]
    fn culls_opt_in_dead_branch_and_keeps_output_dependency_chain() {
        let main = RenderGraphResourceId(1);
        let dead = RenderGraphResourceId(2);
        let output = RenderGraphResourceId(3);
        let graph = RenderGraphDesc::new("culling")
            .add_resource(transient(1))
            .add_resource(transient(2))
            .add_resource(RenderGraphResourceDesc::external(
                output,
                "output",
                RenderGraphResourceUsage::ColorAttachment,
            ))
            .add_pass(
                pass(30, "main")
                    .writes(main, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(10, "dead")
                    .writes(dead, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(20, "present")
                    .reads(main, RenderGraphResourceUsage::SampledTexture)
                    .writes(output, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            );

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        assert_eq!(compiled.culling.roots, vec![RenderGraphPassId(20)]);
        assert_eq!(
            compiled.culling.live_passes,
            vec![RenderGraphPassId(30), RenderGraphPassId(20)]
        );
        assert_eq!(compiled.culling.culled_passes, vec![RenderGraphPassId(10)]);
        assert_eq!(compiled.dag.execution_order, compiled.culling.live_passes);
        assert!(
            compiled
                .dag
                .passes
                .iter()
                .find(|pass| pass.id == RenderGraphPassId(10))
                .expect("dead pass node missing")
                .culled
        );
    }

    #[test]
    fn lifetime_analysis_uses_only_live_execution_order() {
        let main = RenderGraphResourceId(1);
        let dead = RenderGraphResourceId(2);
        let output = RenderGraphResourceId(3);
        let graph = RenderGraphDesc::new("lifetimes-after-culling")
            .add_resource(transient(1))
            .add_resource(transient(2))
            .add_resource(RenderGraphResourceDesc::external(
                output,
                "output",
                RenderGraphResourceUsage::ColorAttachment,
            ))
            .add_pass(
                pass(30, "main")
                    .writes(main, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(10, "dead")
                    .writes(dead, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(20, "present")
                    .reads(main, RenderGraphResourceUsage::SampledTexture)
                    .writes(output, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            );

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        let main_lifetime = compiled
            .resource_lifetimes
            .get(main)
            .expect("main lifetime missing");
        assert_eq!(main_lifetime.first_pass, RenderGraphPassId(30));
        assert_eq!(main_lifetime.last_pass, RenderGraphPassId(20));
        assert_eq!(main_lifetime.first_execution_index, 0);
        assert_eq!(main_lifetime.last_execution_index, 1);
        assert_eq!(main_lifetime.read_count, 1);
        assert_eq!(main_lifetime.write_count, 1);
        assert_eq!(main_lifetime.create_count, 0);
        assert_eq!(main_lifetime.execution_span(), 2);
        assert_eq!(main_lifetime.history.len(), 2);
        assert_eq!(
            main_lifetime.history[0],
            RenderGraphResourceUse::new(
                0,
                RenderGraphPassId(30),
                RenderGraphResourceUseKind::Write,
                Some(RenderGraphResourceUsage::StorageTexture),
            )
        );
        assert_eq!(
            main_lifetime.history[1],
            RenderGraphResourceUse::new(
                1,
                RenderGraphPassId(20),
                RenderGraphResourceUseKind::Read,
                Some(RenderGraphResourceUsage::SampledTexture),
            )
        );
        assert!(compiled.resource_lifetimes.get(dead).is_none());
        assert!(compiled.resource_lifetimes.unused_resources.contains(&dead));

        let output_lifetime = compiled
            .resource_lifetimes
            .get(output)
            .expect("output lifetime missing");
        assert_eq!(output_lifetime.first_execution_index, 1);
        assert_eq!(output_lifetime.last_execution_index, 1);
        assert_eq!(output_lifetime.write_count, 1);
    }

    #[test]
    fn lifetime_analysis_counts_create_as_its_own_event() {
        let resource = RenderGraphResourceId(1);
        let output = RenderGraphResourceId(2);
        let mut creator = pass(1, "create").cullable();
        creator.creates.push(resource);
        let graph = RenderGraphDesc::new("create-lifetime")
            .add_resource(transient(1))
            .add_resource(RenderGraphResourceDesc::external(
                output,
                "output",
                RenderGraphResourceUsage::ColorAttachment,
            ))
            .add_pass(creator)
            .add_pass(
                pass(2, "consume")
                    .reads(resource, RenderGraphResourceUsage::SampledTexture)
                    .writes(output, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            );

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        let lifetime = compiled
            .resource_lifetimes
            .get(resource)
            .expect("created resource lifetime missing");
        assert_eq!(lifetime.first_pass, RenderGraphPassId(1));
        assert_eq!(lifetime.last_pass, RenderGraphPassId(2));
        assert_eq!(lifetime.create_count, 1);
        assert_eq!(lifetime.read_count, 1);
        assert_eq!(lifetime.write_count, 0);
        assert_eq!(lifetime.access_count(), 2);
        assert_eq!(lifetime.history.len(), 2);
        assert_eq!(lifetime.history[0].kind, RenderGraphResourceUseKind::Create);
        assert_eq!(lifetime.history[0].execution_index, 0);
        assert_eq!(lifetime.history[0].usage, None);
        assert_eq!(lifetime.history[1].kind, RenderGraphResourceUseKind::Read);
        assert_eq!(lifetime.history[1].execution_index, 1);
        assert_eq!(lifetime.last_live_use(), lifetime.history.last());
    }

    #[test]
    fn compiler_builds_transient_alias_plan_from_live_lifetimes() {
        let first = RenderGraphResourceId(1);
        let second = RenderGraphResourceId(2);
        let output_a = RenderGraphResourceId(3);
        let output_b = RenderGraphResourceId(4);
        let graph = RenderGraphDesc::new("phase4-transient-plan")
            .add_resource(transient(1))
            .add_resource(transient(2))
            .add_resource(RenderGraphResourceDesc::external(
                output_a,
                "output-a",
                RenderGraphResourceUsage::ColorAttachment,
            ))
            .add_resource(RenderGraphResourceDesc::external(
                output_b,
                "output-b",
                RenderGraphResourceUsage::ColorAttachment,
            ))
            .add_pass(
                pass(1, "produce-first")
                    .writes(first, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(2, "consume-first")
                    .reads(first, RenderGraphResourceUsage::SampledTexture)
                    .writes(output_a, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            )
            .add_pass(
                pass(3, "produce-second")
                    .writes(second, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(4, "consume-second")
                    .reads(second, RenderGraphResourceUsage::SampledTexture)
                    .writes(output_b, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            );

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        let plan = &compiled.transient_allocation_plan;
        assert_eq!(compiled.dag.execution_order.len(), 4);
        assert_eq!(plan.slots.len(), 1);
        assert_eq!(plan.alias_groups.len(), 1);
        assert_eq!(plan.alias_reuse_count(), 1);
        assert_eq!(
            plan.slots[0].resources,
            vec![first, second],
            "disjoint compatible lifetimes should reuse one transient slot"
        );
        assert_eq!(plan.resource_to_slot.get(&first), Some(&0));
        assert_eq!(plan.resource_to_slot.get(&second), Some(&0));
        assert!(plan.ineligible_resources.is_empty());
    }

    #[test]
    fn observable_resource_uses_only_last_writer_as_culling_root() {
        let output = RenderGraphResourceId(1);
        let graph = RenderGraphDesc::new("last-output-writer")
            .add_resource(RenderGraphResourceDesc::external(
                output,
                "output",
                RenderGraphResourceUsage::ColorAttachment,
            ))
            .add_pass(
                pass(1, "first")
                    .writes(output, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            )
            .add_pass(
                pass(2, "last")
                    .writes(output, RenderGraphResourceUsage::ColorAttachment)
                    .cullable(),
            );

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        assert_eq!(compiled.culling.roots, vec![RenderGraphPassId(2)]);
        assert_eq!(
            compiled.culling.live_passes,
            vec![RenderGraphPassId(1), RenderGraphPassId(2)],
            "WAW is conservatively retained until subresource/load semantics exist"
        );
    }

    #[test]
    fn non_cullable_pass_is_root_and_keeps_its_cullable_producer() {
        let resource = RenderGraphResourceId(1);
        let graph = RenderGraphDesc::new("side-effect-root")
            .add_resource(transient(1))
            .add_pass(
                pass(1, "producer")
                    .writes(resource, RenderGraphResourceUsage::StorageTexture)
                    .cullable(),
            )
            .add_pass(
                pass(2, "side-effect").reads(resource, RenderGraphResourceUsage::SampledTexture),
            );

        let compiled = compile_render_graph_v2(&graph).expect("graph should compile");
        assert_eq!(compiled.culling.roots, vec![RenderGraphPassId(2)]);
        assert_eq!(
            compiled.culling.live_passes,
            vec![RenderGraphPassId(1), RenderGraphPassId(2)]
        );
        assert!(compiled.culling.culled_passes.is_empty());
    }

    #[test]
    fn rejects_transient_read_without_producer() {
        let resource = RenderGraphResourceId(1);
        let graph = RenderGraphDesc::new("missing-producer")
            .add_resource(transient(1))
            .add_pass(pass(1, "read").reads(resource, RenderGraphResourceUsage::SampledTexture));

        let errors = compile_render_graph_v2(&graph).expect_err("graph must be rejected");
        assert!(errors
            .iter()
            .any(|issue| issue.code == "render_graph.missing_producer"));
    }

    #[test]
    fn rejects_duplicate_resource_creator() {
        let resource = RenderGraphResourceId(1);
        let mut a = pass(1, "a");
        a.creates.push(resource);
        let mut b = pass(2, "b");
        b.creates.push(resource);
        let graph = RenderGraphDesc::new("duplicate-create")
            .add_resource(transient(1))
            .add_pass(a)
            .add_pass(b);

        let errors = compile_render_graph_v2(&graph).expect_err("graph must be rejected");
        assert!(errors
            .iter()
            .any(|issue| issue.code == "render_graph.duplicate_create"));
    }

    #[test]
    fn rejects_unknown_created_resource() {
        let mut creator = pass(1, "creator");
        creator.creates.push(RenderGraphResourceId(99));
        let graph = RenderGraphDesc::new("unknown-create").add_pass(creator);

        let errors = compile_render_graph_v2(&graph).expect_err("graph must be rejected");
        assert!(errors
            .iter()
            .any(|issue| issue.code == "render_graph.unknown_created_resource"));
    }

    #[test]
    fn topological_order_detects_cycles_defensively() {
        let a = RenderGraphPassId(1);
        let b = RenderGraphPassId(2);
        let dependencies = BTreeMap::from([(a, BTreeSet::from([b])), (b, BTreeSet::from([a]))]);
        let reverse = BTreeMap::from([(a, BTreeSet::from([b])), (b, BTreeSet::from([a]))]);
        let order = BTreeMap::from([(a, 0), (b, 1)]);

        let errors =
            topological_order(&dependencies, &reverse, &order).expect_err("cycle must be rejected");
        assert_eq!(errors[0].code, "render_graph.cycle");
    }
}
