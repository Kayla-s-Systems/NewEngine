use crossbeam_channel::unbounded;
use log::{error, info, warn};

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

    EditorStartupConfig::from_env().dispatch(&tx_cmd);

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

struct EditorStartupConfig {
    command: Option<EditorEvent>,
}

impl EditorStartupConfig {
    fn from_env() -> Self {
        if let Ok(url) = std::env::var("NEO_CEF_URL") {
            info!("editor startup: loading URL from NEO_CEF_URL");
            return Self {
                command: Some(EditorEvent::LoadUrl(url)),
            };
        }

        if let Ok(path) = std::env::var("NEO_CEF_HTML_PATH") {
            match std::fs::read_to_string(&path) {
                Ok(html) => {
                    info!("editor startup: loading HTML from NEO_CEF_HTML_PATH");
                    return Self {
                        command: Some(EditorEvent::LoadHtml(html)),
                    };
                }
                Err(err) => {
                    error!("editor startup: failed to read HTML file {path}: {err}");
                    return Self { command: None };
                }
            }
        }

        if let Ok(url) = std::env::var("NEO_CEF_HTTP_URL") {
            info!("editor startup: fetching HTTP content from NEO_CEF_HTTP_URL");
            return Self {
                command: Some(EditorEvent::LoadHttp(CefHttpRequest {
                    method: std::env::var("NEO_CEF_HTTP_METHOD").unwrap_or_else(|_| "GET".into()),
                    url,
                    headers: Vec::new(),
                    body: std::env::var("NEO_CEF_HTTP_BODY").ok(),
                })),
            };
        }

        if let Ok(exit) = std::env::var("NEO_EDITOR_EXIT") {
            if exit == "1" {
                warn!("editor startup: exit requested via NEO_EDITOR_EXIT");
                return Self {
                    command: Some(EditorEvent::Exit),
                };
            }
        }

        warn!("editor startup: no startup content configured; set NEO_CEF_URL, NEO_CEF_HTML_PATH, or NEO_CEF_HTTP_URL");
        Self { command: None }
    }

    fn dispatch(&self, tx: &crossbeam_channel::Sender<EditorEvent>) {
        if let Some(command) = self.command.clone() {
            let _ = tx.send(command);
        }
    }
}
