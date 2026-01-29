use crate::CefApiRef;
use log::{error, info, warn};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use parking_lot::Mutex;

use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CefHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CefContentRequest {
    Html(String),
    Url(String),
    Http(CefHttpRequest),
}

pub trait CefContentApi: Send + Sync {
    fn request(&self, request: CefContentRequest);
}

pub type CefContentApiRef = Arc<dyn CefContentApi + Send + Sync>;

struct CefContentApiImpl {
    queue: Arc<Mutex<VecDeque<CefContentRequest>>>,
}

impl CefContentApi for CefContentApiImpl {
    fn request(&self, request: CefContentRequest) {
        let mut q = self.queue.lock();
        q.push_back(request);
    }
}

pub struct CefContentModule {
    queue: Arc<Mutex<VecDeque<CefContentRequest>>>,
    warned_missing_cef: bool,
}

impl CefContentModule {
    #[inline]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            warned_missing_cef: false,
        }
    }

    #[inline]
    fn pop_one(&self) -> Option<CefContentRequest> {
        let mut q = self.queue.lock();
        q.pop_front()
    }

    fn process_request(&self, cef: &CefApiRef, request: CefContentRequest) -> EngineResult<()> {
        match request {
            CefContentRequest::Html(html) => {
                info!("loading HTML content into CEF");
                cef.load_local_html(&html);
                Ok(())
            }
            CefContentRequest::Url(url) => {
                info!("loading URL into CEF: {url}");
                cef.load_url(&url);
                Ok(())
            }
            CefContentRequest::Http(req) => {
                info!("requesting HTTP content for CEF: {}", req.url);

                let mut r = ureq::request(&req.method, &req.url);
                for (k, v) in req.headers.iter() {
                    r = r.set(k, v);
                }

                let resp = if let Some(body) = req.body.as_deref() {
                    r.send_string(body)
                } else {
                    r.call()
                };

                match resp {
                    Ok(ok) => {
                        let text = ok
                            .into_string()
                            .map_err(|e| EngineError::Other(format!("http read failed: {e}")))?;
                        cef.load_local_html(&text);
                        Ok(())
                    }
                    Err(err) => {
                        error!("http request failed: {err}");
                        Err(EngineError::Other(format!("http request failed: {err}")))
                    }
                }
            }
        }
    }
}

impl<E: Send + 'static> Module<E> for CefContentModule {
    fn id(&self) -> &'static str {
        "cef-content"
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let api: CefContentApiRef = Arc::new(CefContentApiImpl {
            queue: self.queue.clone(),
        });

        // ModuleCtx exposes only resources(), so resources() must be interior-mutable.
        ctx.resources().insert_once::<CefContentApiRef>(api)?;
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let cef = match ctx.resources().get::<CefApiRef>() {
            Some(api) => api.clone(),
            None => {
                if !self.warned_missing_cef {
                    self.warned_missing_cef = true;
                    warn!("cef-content: CefApiRef not found; ensure 'cef' module is registered before 'cef-content'");
                }
                return Ok(());
            }
        };

        if !cef.is_ready() {
            return Ok(());
        }

        // Avoid long stalls; keep deterministic per tick.
        const MAX_REQUESTS_PER_TICK: usize = 1;

        for _ in 0..MAX_REQUESTS_PER_TICK {
            let Some(req) = self.pop_one() else { break; };
            self.process_request(&cef, req)?;
        }

        Ok(())
    }
}
