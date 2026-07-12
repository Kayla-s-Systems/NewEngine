use super::super::Engine;

use crate::log_fmt::{ellipsize, emit_boxed_kv, emit_prefixed_table};
use crate::path_fmt::display_clean;
use newengine_plugin_api::PluginKind;

#[inline]
fn plugin_kind_label(kind: Option<PluginKind>) -> &'static str {
    match kind {
        Some(PluginKind::Runtime) => "runtime",
        Some(PluginKind::Importer) => "importer",
        Some(PluginKind::Editor) => "editor",
        Some(PluginKind::Tool) => "tool",
        Some(PluginKind::Other) => "other",
        None => "-",
    }
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub fn emit_plugins_diagnostics(&self, tag: &'static str) {
        self.log_plugins_diagnostics(tag);
    }

    #[inline]
    pub(crate) fn log_plugins_diagnostics(&self, tag: &'static str) {
        let list = self.plugins.snapshot();
        let n = list.len();

        emit_boxed_kv(
            &format!("Plugins :: Diagnostics [{}]", tag),
            &[("loaded", n.to_string())],
        );

        if list.is_empty() {
            return;
        }

        let debug_tables = newengine_ulog_api::ulog::debug_enabled();
        let rows: Vec<Vec<String>> = list
            .iter()
            .map(|p| {
                if debug_tables {
                    vec![
                        ellipsize(&p.id, 32),
                        ellipsize(&p.version, 12),
                        ellipsize(&p.state, 16),
                        plugin_kind_label(p.kind).to_owned(),
                        p.capabilities.len().to_string(),
                    ]
                } else {
                    vec![
                        ellipsize(&p.id, 32),
                        ellipsize(&p.version, 12),
                        ellipsize(&p.state, 16),
                    ]
                }
            })
            .collect();

        let plugin_headers: &[&str] = if debug_tables {
            &["id", "ver", "state", "kind", "caps"]
        } else {
            &["id", "ver", "state"]
        };
        emit_prefixed_table(
            "",
            &format!("Plugins :: Registered [{}]", tag),
            plugin_headers,
            &rows,
        );

        let gateway_routes = newengine_plugin_host::list_engine_gateway_routes();
        if !gateway_routes.is_empty() {
            let mut hierarchy_routes = gateway_routes.clone();
            hierarchy_routes.sort_by(|a, b| {
                let ar = newengine_service_api::engine_gateway_root_id(&a.gateway_id)
                    .unwrap_or_else(|| a.gateway_id.clone());
                let br = newengine_service_api::engine_gateway_root_id(&b.gateway_id)
                    .unwrap_or_else(|| b.gateway_id.clone());
                ar.cmp(&br)
                    .then_with(|| {
                        newengine_service_api::engine_gateway_depth(&a.gateway_id)
                            .unwrap_or(0)
                            .cmp(
                                &newengine_service_api::engine_gateway_depth(&b.gateway_id)
                                    .unwrap_or(0),
                            )
                    })
                    .then_with(|| a.gateway_id.cmp(&b.gateway_id))
                    .then_with(|| b.active.cmp(&a.active))
            });
            let hierarchy_rows = hierarchy_routes
                .iter()
                .map(|route| {
                    let root = newengine_service_api::engine_gateway_root_id(&route.gateway_id)
                        .unwrap_or_else(|| route.gateway_id.clone());
                    let parent = newengine_service_api::engine_gateway_parent_id(&route.gateway_id)
                        .unwrap_or_else(|| "<root>".to_owned());
                    let attach = if parent == "<root>" { "root" } else { "child" };
                    vec![
                        ellipsize(&root, 28),
                        ellipsize(&parent, 32),
                        ellipsize(&route.gateway_id, 36),
                        attach.to_owned(),
                        route.selection_state.clone(),
                        route.service_kind.clone(),
                        ellipsize(route.provider_route_id.as_deref().unwrap_or("-"), 32),
                        ellipsize(&route.provider_service_id, 28),
                    ]
                })
                .collect::<Vec<_>>();
            emit_prefixed_table(
                "",
                &format!("Plugins :: Gateway Hierarchy [{}]", tag),
                &[
                    "root",
                    "parent",
                    "gateway",
                    "attach",
                    "state",
                    "kind",
                    "provider_route",
                    "provider_service",
                ],
                &hierarchy_rows,
            );

            let route_rows = gateway_routes
                .iter()
                .map(|route| {
                    if debug_tables {
                        vec![
                            ellipsize(&route.gateway_id, 28),
                            route.selection_state.clone(),
                            route.origin.clone(),
                            ellipsize(route.provider_route_id.as_deref().unwrap_or("-"), 28),
                            ellipsize(&route.provider_service_id, 28),
                            ellipsize(&route.provider_owner_id, 32),
                            route.service_kind.clone(),
                            ellipsize(&route.backend_capability_id, 28),
                            route.override_mode.clone(),
                            route.backend_priority.to_string(),
                            route.active_score.to_string(),
                            ellipsize(&route.selection_reason, 48),
                        ]
                    } else {
                        vec![
                            ellipsize(&route.gateway_id, 28),
                            route.selection_state.clone(),
                            route.origin.clone(),
                            ellipsize(route.provider_route_id.as_deref().unwrap_or("-"), 28),
                            ellipsize(&route.provider_service_id, 28),
                            route.backend_priority.to_string(),
                            ellipsize(&route.selection_reason, 42),
                        ]
                    }
                })
                .collect::<Vec<_>>();

            let route_headers: &[&str] = if debug_tables {
                &[
                    "gateway",
                    "state",
                    "source",
                    "provider_route",
                    "provider_service",
                    "owner",
                    "kind",
                    "capability",
                    "mode",
                    "prio",
                    "score",
                    "selection_reason",
                ]
            } else {
                &[
                    "gateway",
                    "state",
                    "source",
                    "provider_route",
                    "provider_service",
                    "prio",
                    "selection_reason",
                ]
            };
            emit_prefixed_table(
                "",
                &format!("Plugins :: Gateway Routes [{}]", tag),
                route_headers,
                &route_rows,
            );
        }

        if newengine_ulog_api::ulog::debug_enabled() {
            for p in &list {
                newengine_ulog_api::ulog::debug!(
                    "plugins: path id='{}' caps={} path='{}'",
                    p.id,
                    p.capabilities.len(),
                    display_clean(&p.path)
                );
            }
        }
    }
}
