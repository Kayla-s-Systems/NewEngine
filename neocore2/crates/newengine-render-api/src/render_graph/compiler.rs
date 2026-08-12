use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    RenderGraphBarrierStats, RenderGraphCompileReport, RenderGraphDesc, RenderGraphLifetimeStats,
    RenderGraphPassId, RenderGraphResourceId, RenderGraphResourceLifetime,
    RenderGraphValidationIssue, RenderGraphValidationReport,
};

pub fn validate_and_compile_render_graph(graph: &RenderGraphDesc) -> RenderGraphValidationReport {
    match compile_render_graph(graph) {
        Ok(report) => RenderGraphValidationReport {
            ok: true,
            errors: Vec::new(),
            warnings: report.warnings.clone(),
            compile: Some(report),
        },
        Err(errors) => RenderGraphValidationReport {
            ok: false,
            errors,
            warnings: Vec::new(),
            compile: None,
        },
    }
}

pub fn compile_render_graph(
    graph: &RenderGraphDesc,
) -> Result<RenderGraphCompileReport, Vec<RenderGraphValidationIssue>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut resource_ids = BTreeSet::new();
    let mut pass_ids = BTreeSet::new();
    let mut lifetime = RenderGraphLifetimeStats::default();

    for resource in &graph.resources {
        if !resource_ids.insert(resource.id) {
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

    for pass in &graph.passes {
        if !pass_ids.insert(pass.id) {
            errors.push(
                RenderGraphValidationIssue::new(
                    "render_graph.duplicate_pass",
                    "render graph contains duplicate pass id",
                )
                .with_pass(pass.id),
            );
        }

        for access in pass.reads.iter().chain(pass.writes.iter()) {
            if !resource_ids.contains(&access.resource) {
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

    let mut last_writer: BTreeMap<RenderGraphResourceId, RenderGraphPassId> = BTreeMap::new();
    let mut last_readers: BTreeMap<RenderGraphResourceId, BTreeSet<RenderGraphPassId>> =
        BTreeMap::new();
    let mut dependencies: BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>> = graph
        .passes
        .iter()
        .map(|p| (p.id, BTreeSet::new()))
        .collect();
    let mut reverse_edges: BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>> = graph
        .passes
        .iter()
        .map(|p| (p.id, BTreeSet::new()))
        .collect();
    let mut barriers = RenderGraphBarrierStats::default();

    for pass in &graph.passes {
        for created in &pass.creates {
            last_writer.insert(*created, pass.id);
        }

        for read in &pass.reads {
            if let Some(writer) = last_writer.get(&read.resource).copied() {
                if writer != pass.id {
                    dependencies.entry(pass.id).or_default().insert(writer);
                    reverse_edges.entry(writer).or_default().insert(pass.id);
                    barriers.read_after_write = barriers.read_after_write.saturating_add(1);
                }
            } else {
                barriers.external_imports = barriers.external_imports.saturating_add(1);
            }
            last_readers
                .entry(read.resource)
                .or_default()
                .insert(pass.id);
        }

        for write in &pass.writes {
            if let Some(writer) = last_writer.get(&write.resource).copied() {
                if writer != pass.id {
                    dependencies.entry(pass.id).or_default().insert(writer);
                    reverse_edges.entry(writer).or_default().insert(pass.id);
                    barriers.write_after_write = barriers.write_after_write.saturating_add(1);
                }
            }
            if let Some(readers) = last_readers.remove(&write.resource) {
                for reader in readers {
                    if reader != pass.id {
                        dependencies.entry(pass.id).or_default().insert(reader);
                        reverse_edges.entry(reader).or_default().insert(pass.id);
                        barriers.write_after_read = barriers.write_after_read.saturating_add(1);
                    }
                }
            }
            last_writer.insert(write.resource, pass.id);
        }
    }

    let mut indegree: BTreeMap<RenderGraphPassId, usize> = dependencies
        .iter()
        .map(|(pass, deps)| (*pass, deps.len()))
        .collect();
    let mut ready: VecDeque<RenderGraphPassId> = indegree
        .iter()
        .filter_map(|(pass, count)| (*count == 0).then_some(*pass))
        .collect();
    let mut execution_order = Vec::with_capacity(graph.passes.len());

    while let Some(pass) = ready.pop_front() {
        execution_order.push(pass);
        let Some(edges) = reverse_edges.get(&pass) else {
            continue;
        };
        for next in edges {
            let Some(count) = indegree.get_mut(next) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                let pos = ready
                    .iter()
                    .position(|queued| queued > next)
                    .unwrap_or(ready.len());
                ready.insert(pos, *next);
            }
        }
    }

    if execution_order.len() != graph.passes.len() {
        return Err(vec![RenderGraphValidationIssue::new(
            "render_graph.cycle",
            "render graph has a dependency cycle",
        )]);
    }

    Ok(RenderGraphCompileReport {
        pass_count: graph.passes.len() as u32,
        resource_count: graph.resources.len() as u32,
        execution_order,
        lifetime,
        barriers,
        warnings,
    })
}
