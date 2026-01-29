use crate::CefApiRef;
use log::{error, info};
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
        let mut queue = self.queue.lock();
        queue.push_back(request);
    }
}

pub struct CefContentModule {
    queue: Arc<Mutex<VecDeque<CefContentRequest>>>,
}

impl CefContentModule {
    #[inline]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn process_request(&self, cef: &CefApiRef, request: CefContentRequest) -> EngineResult<()> {
        match request {
            CefContentRequest::Html(html) => {
                info!("loading HTML content into CEF");
                cef.load_local_html(&html);
            }
            CefContentRequest::Url(url) => {
                info!("loading URL into CEF: {url}");
                cef.load_url(&url);
            }
            CefContentRequest::Http(req) => {
                info!("requesting HTTP content for CEF: {}", req.url);
                let mut request = ureq::request(&req.method, &req.url);
                for (k, v) in req.headers.iter() {
                    request = request.set(k, v);
                }
                let response = if let Some(body) = req.body {
                    request.send_string(&body)
                } else {
                    request.call()
                };

                match response {
                    Ok(resp) => {
                        let text = resp
                            .into_string()
                            .map_err(|e| EngineError::Other(format!("http read failed: {e}")))?;
                        cef.load_local_html(&text);
                    }
                    Err(err) => {
                        error!("http request failed: {err}");
                        return Err(EngineError::Other(format!("http request failed: {err}")));
                    }
                }
            }
        }

        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for CefContentModule {
    fn id(&self) -> &'static str {
        "cef-content"
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["cef"]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let api: CefContentApiRef = Arc::new(CefContentApiImpl {
            queue: self.queue.clone(),
        });
        ctx.resources().insert_once::<CefContentApiRef>(api)?;
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let cef = match ctx.resources().get::<CefApiRef>() {
            Some(api) => api.clone(),
            None => return Ok(()),
        };

        if !cef.is_ready() {
            return Ok(());
        }

        let maybe_request = {
            let mut queue = self.queue.lock();
            queue.pop_front()
        };

        if let Some(request) = maybe_request {
            self.process_request(&cef, request)?;
        }

        Ok(())
    }
}
