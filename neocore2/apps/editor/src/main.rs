use crossbeam_channel::unbounded;

use newengine_core::{Bus, Engine, EngineResult, Module, ModuleCtx, Services, ShutdownToken};
use newengine_modules_cef::{CefContentApiRef, CefContentModule, CefContentRequest, CefHttpRequest, CefModule};
use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_platform_winit::run_winit_app;

struct AppServices;

impl AppServices {
    #[inline]
    fn new() -> Self {
        Self
    }
}

impl Services for AppServices {
    fn logger(&self) -> &dyn log::Log {
        log::logger()
    }
}

#[derive(Debug, Clone)]
enum EditorEvent {
    Exit,
    LoadUrl(String),
    LoadHtml(String),
    LoadHttp(CefHttpRequest),
}

fn main() -> EngineResult<()> {
    let (tx, rx) = unbounded::<EditorEvent>();
    let tx_cmd = tx.clone();
    let bus: Bus<EditorEvent> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();

    let mut engine: Engine<EditorEvent> = Engine::new(16, services, bus, shutdown)?;

    // Модули можно регистрировать в любом порядке — Engine сам отсортирует по dependencies().
    engine.register_module(Box::new(ConsoleLoggerModule::new(ConsoleLoggerConfig::default())))?;
    engine.register_module(Box::new(CefContentModule::new()))?;
    engine.register_module(Box::new(CefModule::new()))?;
    engine.register_module(Box::new(EditorOperatorModule::new()))?;

    // Старт — Engine:
    // 1) топосорт
    // 2) init всех модулей по зависимостям
    // 3) start всех модулей по зависимостям
    engine.start()?;

    // Операторские команды приходят из main.rs
    // (можешь заменить на чтение конфига/CLI/скрипта).
    if let Ok(url) = std::env::var("NEO_CEF_URL") {
        tx_cmd.send(EditorEvent::LoadUrl(url)).ok();
    } else {
        tx_cmd.send(EditorEvent::LoadUrl("https://example.com".to_string())).ok();
    }

    run_winit_app(engine)
}

struct EditorOperatorModule;

impl EditorOperatorModule {
    #[inline]
    fn new() -> Self {
        Self
    }
}

impl<E: Send + 'static> Module<E> for EditorOperatorModule {
    fn id(&self) -> &'static str {
        "editor-operator"
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["cef-content"]
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        // Пытаемся получить CefContent API
        let api = match ctx.resources().get::<CefContentApiRef>() {
            Some(v) => v.clone(),
            None => return Ok(()),
        };

        // Дренируем команды, которые main.rs (или кто угодно) отправил в Bus.
        // Важно: этот модуль типизирован на E, поэтому ожидаем, что E == EditorEvent в приложении.
        // В generic-сборках (другие приложения) этот модуль не подключай.
        let mut tmp = Vec::new();
        ctx.bus().drain_into(&mut tmp);

        for ev in tmp {
            // Без downcast: это наш app-level enum, если E другой — просто не используй модуль.
            // Здесь предполагается, что E == EditorEvent.
            // Поэтому этот модуль живёт в apps/editor, а не в core.
            let Some(ev) = (unsafe { any_to_editor_event::<E>(ev) }) else {
                continue;
            };

            match ev {
                EditorEvent::Exit => {
                    ctx.request_exit();
                }
                EditorEvent::LoadUrl(url) => {
                    api.request(CefContentRequest::Url(url));
                }
                EditorEvent::LoadHtml(html) => {
                    api.request(CefContentRequest::Html(html));
                }
                EditorEvent::LoadHttp(req) => {
                    api.request(CefContentRequest::Http(req));
                }
            }
        }

        Ok(())
    }
}

/// Converts E into EditorEvent if (and only if) E is EditorEvent.
/// This keeps Engine generic but allows app-specific operator module.
///
/// Safety: Only sound when E == EditorEvent.
unsafe fn any_to_editor_event<E: Send + 'static>(ev: E) -> Option<EditorEvent> {
    use std::any::TypeId;
    if TypeId::of::<E>() == TypeId::of::<EditorEvent>() {
        // move-cast: E and EditorEvent are identical types
        let boxed: Box<dyn std::any::Any> = Box::new(ev);
        return boxed.downcast::<EditorEvent>().ok().map(|b| *b);
    }
    None
}