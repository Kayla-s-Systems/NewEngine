use newengine_core::{ApiProvide, ApiVersion, Engine, EngineResult, EventSub, Module, ModuleCtx};
use newengine_game_events_api::{
    GameMessageDescriptor, GameMessageEnvelope, GameMessageReliability, GameMessageScope,
};
use newengine_ui_api::{UiToastStack, ENGINE_UI_NOTIFY_SERVICE_ID, UI_NOTIFY_MESSAGE_IDS};

use crate::service::register_ui_notify_gateway_best_effort;
use crate::state::UiNotifyRuntime;

const UI_NOTIFY_MODULE_ID: &str = "engine.ui.notify.runtime.module";
const UI_NOTIFY_MODULE_API: &[ApiProvide] = &[ApiProvide::new(
    ENGINE_UI_NOTIFY_SERVICE_ID,
    ApiVersion::new(1, 0, 0),
)];

pub struct UiNotifyModule {
    runtime: UiNotifyRuntime,
    messages: Option<EventSub<GameMessageEnvelope>>,
}

impl UiNotifyModule {
    pub fn new(runtime: UiNotifyRuntime) -> Self {
        Self {
            runtime,
            messages: None,
        }
    }

    fn register_message_descriptors(&self, ctx: &ModuleCtx<'_, ()>) {
        let Some(registry) = ctx
            .resources()
            .get::<newengine_game_events_runtime::GameMessageRegistry>()
        else {
            newengine_ulog_api::ulog::warn!(
                "ui notify: engine.game.events registry unavailable; direct engine.ui.notify calls remain active"
            );
            return;
        };

        for &id in UI_NOTIFY_MESSAGE_IDS {
            if registry.descriptor(id).is_some() {
                continue;
            }
            let severity = id.rsplit('.').next().unwrap_or("info");
            let _ = registry.register(GameMessageDescriptor {
                id: id.to_owned(),
                owner: ENGINE_UI_NOTIFY_SERVICE_ID.to_owned(),
                description: format!(
                    "Local UI notification envelope consumed by engine.ui.notify ({severity})"
                ),
                payload_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "toast_id": {"type": "string"},
                        "title": {"type": "string"},
                        "detail": {"type": "string"},
                        "message": {"type": "string"},
                        "severity": {"type": "string"},
                        "duration_ms": {"type": "integer", "minimum": 0},
                        "sticky": {"type": "boolean"},
                        "progress_permille": {"type": "integer", "minimum": 0, "maximum": 1000},
                        "replace_existing": {"type": "boolean"}
                    }
                }),
                scope: GameMessageScope::Local,
                reliability: GameMessageReliability::BestEffort,
                max_payload_bytes: 16 * 1024,
                tags: vec![
                    "ui".to_owned(),
                    "notification".to_owned(),
                    "toast".to_owned(),
                ],
                ..Default::default()
            });
        }
    }
}

impl Module<()> for UiNotifyModule {
    fn id(&self) -> &'static str {
        UI_NOTIFY_MODULE_ID
    }

    fn provides(&self) -> &'static [ApiProvide] {
        UI_NOTIFY_MODULE_API
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        self.messages = Some(
            ctx.events()
                .subscribe_filtered_bounded::<GameMessageEnvelope, _>(
                    256,
                    newengine_core::events::OverflowPolicy::DropNewest,
                    |message| {
                        UI_NOTIFY_MESSAGE_IDS
                            .iter()
                            .any(|message_id| *message_id == message.id)
                    },
                ),
        );
        self.register_message_descriptors(ctx);
        ctx.resources_mut().insert(self.runtime.clone());
        ctx.resources_mut().insert(UiToastStack::default());
        newengine_ulog_api::ulog::info!(
            "ui notify: message pipeline subscribed gateway='{}' message_ids={} capacity={} visible_limit={}",
            ENGINE_UI_NOTIFY_SERVICE_ID,
            UI_NOTIFY_MESSAGE_IDS.len(),
            self.runtime.snapshot().capacity,
            self.runtime.snapshot().visible_limit,
        );
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        if let Some(messages) = self.messages.as_ref() {
            messages.drain(|message| {
                let _ = self.runtime.ingest_message(message.as_ref());
            });
            self.runtime.observe_pipeline_dropped(messages.dropped());
        }

        let frame = ctx.frame().copied();
        let dt_ms = frame
            .map(|frame| {
                let dt = if frame.dt.is_finite() {
                    frame.dt.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (dt * 1_000.0).round() as u64
            })
            .unwrap_or(0);
        self.runtime.advance(dt_ms);
        let stack = self
            .runtime
            .stack(frame.map(|frame| frame.frame_index).unwrap_or_default());
        ctx.resources_mut().insert(stack);
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        let _ = self.runtime.clear(newengine_ui_api::UiNotifyClearRequest {
            source: None,
            include_sticky: true,
        });
        ctx.resources_mut().insert(UiToastStack::default());
        self.messages = None;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiNotifyInstallReport {
    pub gateway_registered: bool,
    pub module_registered: bool,
}

pub fn install_ui_notify_runtime(engine: &mut Engine<()>) -> EngineResult<UiNotifyInstallReport> {
    let runtime = UiNotifyRuntime::default();
    engine.resources_mut().insert(runtime.clone());
    engine.register_module(Box::new(UiNotifyModule::new(runtime.clone())))?;
    let gateway_registered = register_ui_notify_gateway_best_effort(runtime);
    Ok(UiNotifyInstallReport {
        gateway_registered,
        module_registered: true,
    })
}
