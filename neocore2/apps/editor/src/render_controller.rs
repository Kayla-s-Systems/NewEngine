use newengine_core::render::{require_render_api, BeginFrameDesc};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_platform_winit::WinitWindowInitSize;
use newengine_ui::draw::UiDrawList;

pub struct EditorRenderController {
    clear_color: [f32; 4],
    last_w: u32,
    last_h: u32,
}

impl EditorRenderController {
    #[inline]
    pub fn new(clear_color: [f32; 4]) -> Self {
        Self {
            clear_color,
            last_w: 0,
            last_h: 0,
        }
    }
}

impl<E: Send + 'static> Module<E> for EditorRenderController {
    fn id(&self) -> &'static str {
        "app.render_controller"
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let ui: Option<UiDrawList> = ctx.resources_mut().remove::<UiDrawList>();

        let (w, h) = ctx
            .resources()
            .get::<WinitWindowInitSize>()
            .map(|s| (s.width, s.height))
            .unwrap_or((0, 0));

        let api = match require_render_api(ctx) {
            Ok(api) => api,
            Err(_) => return Ok(()),
        };

        let mut r = api.lock();

        if let Some(ui) = ui {
            r.set_ui_draw_list(ui);
        }

        if w != self.last_w || h != self.last_h {
            self.last_w = w;
            self.last_h = h;
            r.resize(w, h)?;
        }

        r.begin_frame(BeginFrameDesc::new(self.clear_color))?;
        r.end_frame()?;

        Ok(())
    }
}