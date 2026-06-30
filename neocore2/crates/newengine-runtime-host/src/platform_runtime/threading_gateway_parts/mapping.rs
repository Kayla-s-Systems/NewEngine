use newengine_core::{TaskLane, TaskPriority};

pub(crate) fn lane_from_str(value: &str) -> TaskLane {
    match value.trim().to_ascii_lowercase().as_str() {
        "simulation" => TaskLane::Simulation,
        "render-prep" | "render_prep" | "renderprep" => TaskLane::RenderPrep,
        "streaming" => TaskLane::Streaming,
        "asset-io" | "asset_io" | "asset" => TaskLane::AssetIo,
        "plugin" | "plugins" => TaskLane::Plugin,
        "background" | "bg" => TaskLane::Background,
        _ => TaskLane::Plugin,
    }
}

pub(crate) fn priority_from_str(value: &str) -> TaskPriority {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" => TaskPriority::Critical,
        "interactive" => TaskPriority::Interactive,
        "normal" => TaskPriority::Normal,
        "background" | "bg" => TaskPriority::Background,
        _ => TaskPriority::Background,
    }
}

pub(crate) fn task_domain_from_request(value: &str, fallback_owner: &str) -> &'static str {
    match value.trim() {
        "engine.render" | "engine.render.vulkan" => "engine.render",
        "engine.assets" | "engine.assets.starvault" => "engine.assets",
        "engine.simulation" | "newengine-sim" => "engine.simulation",
        "profiler.api" => "engine.profiler",
        _ if fallback_owner == "engine.render" => "engine.render",
        _ if fallback_owner == "profiler.api" => "engine.profiler",
        _ => "engine.threading",
    }
}

pub(crate) fn task_pass_from_category(category: &str, fallback: &str) -> &'static str {
    match category.trim() {
        "shader.compile" => "shader-compile",
        "shader.validate" => "shader-validate",
        "texture.decode" | "asset-decode" => "texture-decode",
        "profiler.report.flush" => "profiler-flush",
        "service-call" => "service-call",
        "tool.process" => "tool-process",
        _ if fallback == "process" => "process",
        _ => "runtime",
    }
}
