use newengine_render_api::{RenderGraphCompilation, RenderGraphDesc, RenderGraphValidationIssue};

/// Phase-3 frame graph compiler: validation + resource hazard DAG + stable
/// topological order + pass culling + live resource lifetime intervals. GPU barriers,
/// transient allocation/aliasing and queue scheduling remain later phases.
#[inline]
pub fn compile_frame_graph_v2(
    graph: &RenderGraphDesc,
) -> Result<RenderGraphCompilation, Vec<RenderGraphValidationIssue>> {
    newengine_render_api::compile_render_graph_v2(graph)
}

#[cfg(test)]
mod tests {
    use crate::{standard_runtime_frame, StandardRuntimePipelineDesc, RG_SCENE_HDR_COLOR};
    use newengine_render_api::{
        Extent2D, RenderGraphDependencyKind, RenderGraphPassDesc, RenderGraphPassId,
        RenderGraphPassKind, RenderGraphResourceDesc, RenderGraphResourceId,
        RenderGraphResourceUsage, TextureFormat,
    };

    #[test]
    fn frame_plan_compile_v2_culls_opt_in_dead_pass() {
        let mut plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(72, Extent2D::new(640, 360), Extent2D::new(640, 360))
                .deferred(false)
                .postfx(true)
                .ui(false)
                .debug_overlay(false),
        );
        let dead_resource = RenderGraphResourceId(9_001);
        let dead_pass = RenderGraphPassId(9_002);
        plan.graph
            .resources
            .push(RenderGraphResourceDesc::transient_texture(
                dead_resource,
                "dead_branch",
                RenderGraphResourceUsage::StorageTexture,
                Extent2D::new(16, 16),
                TextureFormat::Rgba8Unorm,
            ));
        plan.graph.passes.push(
            RenderGraphPassDesc::new(dead_pass, "dead_branch", RenderGraphPassKind::Custom)
                .writes(dead_resource, RenderGraphResourceUsage::StorageTexture)
                .cullable(),
        );

        let compiled = plan.compile_v2().expect("frame graph must compile");
        assert!(compiled.culling.culled_passes.contains(&dead_pass));
        assert!(!compiled.dag.execution_order.contains(&dead_pass));
    }

    #[test]
    fn standard_forward_postfx_graph_exposes_scene_color_live_interval() {
        let plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(
                73,
                Extent2D::new(1280, 720),
                Extent2D::new(1280, 720),
            )
            .deferred(false)
            .postfx(true)
            .ui(false)
            .debug_overlay(false),
        );

        let compiled = plan
            .compile_v2()
            .expect("standard frame graph must compile");
        let lifetime = compiled
            .resource_lifetimes
            .get(RG_SCENE_HDR_COLOR)
            .expect("scene HDR lifetime missing");
        assert!(lifetime.write_count >= 1);
        assert!(lifetime.read_count >= 1);
        assert!(lifetime.last_execution_index >= lifetime.first_execution_index);
    }

    #[test]
    fn standard_forward_postfx_graph_compiles_raw_dependency_dag() {
        let plan = standard_runtime_frame(
            StandardRuntimePipelineDesc::new(
                71,
                Extent2D::new(1280, 720),
                Extent2D::new(1280, 720),
            )
            .deferred(false)
            .postfx(true)
            .ui(false)
            .debug_overlay(false),
        );

        let forward = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == RenderGraphPassKind::ForwardOpaque)
            .expect("forward pass missing")
            .id;
        let particle_simulation = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == RenderGraphPassKind::ParticleSimulation)
            .expect("particle simulation pass missing");
        assert_eq!(
            particle_simulation.queue,
            newengine_render_api::RenderGraphQueueKind::Compute
        );
        let particle_simulation_id = particle_simulation.id;
        let particle_state = plan
            .graph
            .resources
            .iter()
            .find(|resource| resource.label.as_deref() == Some("vfx_particle_state"))
            .expect("VFX particle-state resource missing")
            .id;
        let particle_accum = plan
            .graph
            .resources
            .iter()
            .find(|resource| resource.label.as_deref() == Some("particle_accum"))
            .expect("particle accumulation resource missing")
            .id;
        let particle_gbuffer = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == RenderGraphPassKind::ParticleGBuffer)
            .expect("particle gbuffer pass missing")
            .id;
        let particle_composite = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == RenderGraphPassKind::ParticleComposite)
            .expect("particle composite pass missing")
            .id;
        let transparent = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == RenderGraphPassKind::Transparent)
            .expect("transparent pass missing")
            .id;
        let postfx = plan
            .graph
            .passes
            .iter()
            .find(|pass| pass.kind == RenderGraphPassKind::PostFx)
            .expect("postfx pass missing")
            .id;
        let compiled = plan
            .compile_v2()
            .expect("standard frame graph must compile");

        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == particle_simulation_id
                && edge.consumer == particle_gbuffer
                && edge.resource == particle_state
                && edge.kind == RenderGraphDependencyKind::ReadAfterWrite
        }));
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == particle_gbuffer
                && edge.consumer == particle_composite
                && edge.resource == particle_accum
                && edge.kind == RenderGraphDependencyKind::ReadAfterWrite
        }));
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == forward
                && edge.consumer == particle_composite
                && edge.resource == RG_SCENE_HDR_COLOR
                && edge.kind == RenderGraphDependencyKind::WriteAfterWrite
        }));
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == particle_composite
                && edge.consumer == transparent
                && edge.resource == RG_SCENE_HDR_COLOR
                && edge.kind == RenderGraphDependencyKind::WriteAfterWrite
        }));
        assert!(compiled.dag.edges.iter().any(|edge| {
            edge.producer == transparent
                && edge.consumer == postfx
                && edge.resource == RG_SCENE_HDR_COLOR
                && edge.kind == RenderGraphDependencyKind::ReadAfterWrite
        }));
        assert_eq!(
            compiled.dag.execution_order,
            compiled.report.execution_order
        );
    }
}
