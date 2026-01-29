use crossbeam_channel::unbounded;

use newengine_core::{
    Bus, Engine, EngineError, EngineResult, Module, ModuleCtx, Services, ShutdownToken,
};
use newengine_modules_cef::{
    CefContentApiRef, CefContentModule, CefContentRequest, CefHttpRequest, CefModule,
};
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
}

fn main() -> EngineResult<()> {
    let (tx, rx) = unbounded::<EditorEvent>();
    let bus: Bus<EditorEvent> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();

    let mut engine: Engine<EditorEvent> = Engine::new(16, services, bus, shutdown)?;

    engine.register_module(Box::new(ConsoleLoggerModule::new(
        ConsoleLoggerConfig::default(),
    )))?;
    engine.register_module(Box::new(CefModule::new()))?;
    engine.register_module(Box::new(CefContentModule::new()))?;
    engine.register_module(Box::new(EditorCefBootstrap::new()))?;

    engine.start()?;
    run_winit_app(engine)
}

struct EditorCefBootstrap {
    requested: bool,
}

impl EditorCefBootstrap {
    #[inline]
    fn new() -> Self {
        Self { requested: false }
    }

    fn build_request() -> EngineResult<CefContentRequest> {
        if let Ok(url) = std::env::var("NEO_CEF_HTTP_URL") {
            let method = std::env::var("NEO_CEF_HTTP_METHOD").unwrap_or_else(|_| "GET".to_string());
            let body = std::env::var("NEO_CEF_HTTP_BODY").ok();
            let headers = std::env::var("NEO_CEF_HTTP_HEADERS")
                .ok()
                .map(Self::parse_headers)
                .transpose()?
                .unwrap_or_default();
            return Ok(CefContentRequest::Http(CefHttpRequest {
                method,
                url,
                headers,
                body,
            }));
        }

        if let Ok(url) = std::env::var("NEO_CEF_URL") {
            return Ok(CefContentRequest::Url(url));
        }

        if let Ok(path) = std::env::var("NEO_CEF_HTML_PATH") {
            let html = std::fs::read_to_string(&path)
                .map_err(|e| EngineError::Other(format!("failed to read HTML file {path}: {e}")))?;
            return Ok(CefContentRequest::Html(html));
        }

        if let Ok(html) = std::env::var("NEO_CEF_HTML_INLINE") {
            return Ok(CefContentRequest::Html(html));
        }

        Err(EngineError::Other(
            "CEF content source is not configured. Set NEO_CEF_HTTP_URL, NEO_CEF_URL, NEO_CEF_HTML_PATH, or NEO_CEF_HTML_INLINE."
                .to_string(),
        ))
    }

    fn parse_headers(raw: String) -> EngineResult<Vec<(String, String)>> {
        let mut headers = Vec::new();
        for pair in raw.split(';') {
            let trimmed = pair.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((key, value)) = trimmed.split_once(':') else {
                return Err(EngineError::Other(format!(
                    "invalid header format: {trimmed} (expected Key:Value)"
                )));
            };
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
        Ok(headers)
    }
}

impl<E: Send + 'static> Module<E> for EditorCefBootstrap {
    fn id(&self) -> &'static str {
        "editor-cef-bootstrap"
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["cef-content"]
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let api = ctx
            .resources()
            .get::<CefContentApiRef>()
            .cloned()
            .ok_or_else(|| EngineError::Other("CefContentApi not available".to_string()))?;

        let request = Self::build_request()?;
        api.request(request);
        self.requested = true;
        Ok(())
    }

    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.requested {
            return Ok(());
        }
        Ok(())
    }
}
