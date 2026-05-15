#![forbid(unsafe_op_in_unsafe_fn)]

use crate::asset::AssetAccess;
// -------------------------------------------------------------------------------------------------
// Stub (default): no image decoding feature.
//
// We still expose the type so downstream code can keep a single codepath, but the implementation is
// intentionally a no-op to keep the crate warning-free under default features.
// -------------------------------------------------------------------------------------------------

#[cfg(not(all(feature = "egui", feature = "images")))]
#[derive(Debug, Default)]
pub struct UiImageLoader;

#[cfg(not(all(feature = "egui", feature = "images")))]
impl UiImageLoader {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn request(
        &mut self,
        _assets: &dyn AssetAccess,
        _key: impl Into<String>,
        _path: impl Into<String>,
    ) {}

    #[cfg(feature = "egui")]
    #[inline]
    pub fn pump(&mut self, _ctx: &egui::Context, _assets: &dyn AssetAccess) {}

    #[cfg(feature = "egui")]
    #[inline]
    pub fn pump_into_state(
        &mut self,
        _ctx: &egui::Context,
        _assets: &dyn AssetAccess,
        _state: &mut crate::markup::UiState,
    ) {}

    #[inline]
    pub fn tex_id_u64(&self, _key: &str) -> Option<u64> {
        None
    }
}

// -------------------------------------------------------------------------------------------------
// Full implementation: egui + image decoding.
// -------------------------------------------------------------------------------------------------

#[cfg(all(feature = "egui", feature = "images"))]
mod with_images {
    use super::*;
    use newengine_assets_api::AssetState;
    use newengine_math::collections::FxHashMap;

    type TexHandle = egui::TextureHandle;

    /// Deterministic UI texture loader for icons and textured widgets.
    ///
    /// Design goals:
    /// - No hidden background threads.
    /// - Non-blocking: does not wait; relies on `AssetAccess::pump()` + `AssetAccess::state()`.
    /// - Stable handles: once loaded, returns a persistent `egui::TextureId::User(u64)`.
    ///
    /// Usage pattern (per-frame):
    /// 1) `loader.request(assets, "pm.refresh", "ui/icons/builtin_icons.neytd@refresh")`
    /// 2) `loader.pump(ctx, assets, state)`
    /// 3) Markup refers to `$tex.pm.refresh` (a u64).
    #[derive(Default)]
    pub struct UiImageLoader {
        slots: FxHashMap<String, Slot>,
    }

    #[derive(Clone)]
    #[allow(dead_code)]
    enum Slot {
        Empty { path: String },
        Loading { path: String, id_hex32: String },
        Ready { handle: TexHandle, w: u32, h: u32 },
        Failed { path: String, error: String },
    }

    impl UiImageLoader {
        #[inline]
        pub fn new() -> Self {
            Self::default()
        }

        /// Schedule a texture load for `key`.
        ///
        /// `key` is a stable logical name used in markup via `$tex.<key>`.
        pub fn request(
            &mut self,
            _assets: &dyn AssetAccess,
            key: impl Into<String>,
            path: impl Into<String>,
        ) {
            let key = key.into();
            let path = path.into();
            self.slots.entry(key).or_insert(Slot::Empty { path });
        }

        /// Pump loader state and upload newly ready textures into egui.
        ///
        /// On success, writes these vars into `UiState`:
        /// - `tex.<key>` = u64 (egui TextureId::User)
        /// - `tex.<key>.w` / `tex.<key>.h` = u32 (pixels)
        /// - `tex.<key>.state` = "ready" | "loading" | "failed"
        pub fn pump(&mut self, ctx: &egui::Context, assets: &dyn AssetAccess) {
            self.pump_internal(ctx, assets, None);
        }

        /// Same as `pump`, but also publishes `$tex.*` vars into `UiState` for markup-driven UIs.
        pub fn pump_into_state(
            &mut self,
            ctx: &egui::Context,
            assets: &dyn AssetAccess,
            state: &mut crate::markup::UiState,
        ) {
            self.pump_internal(ctx, assets, Some(state));
        }

        /// Returns `egui::TextureId::User(u64)` for a ready texture.
        ///
        /// Note: `u64 == 0` is a valid id in egui.
        #[inline]
        pub fn tex_id_u64(&self, key: &str) -> Option<u64> {
            match self.slots.get(key)? {
                Slot::Ready { handle, .. } => match handle.id() {
                    egui::TextureId::User(u) => Some(u),
                    _ => None,
                },
                _ => None,
            }
        }

        fn pump_internal(
            &mut self,
            ctx: &egui::Context,
            assets: &dyn AssetAccess,
            mut state: Option<&mut crate::markup::UiState>,
        ) {
            // 1) Advance pending loads.
            // Deterministic pumping order.
            assets.pump();
            let mut keys: Vec<String> = self.slots.keys().cloned().collect();
            keys.sort();
            for k in keys {
                let Some(slot) = self.slots.get_mut(&k) else {
                    continue;
                };
                match slot {
                    Slot::Empty { path } => match assets.import_v1(path) {
                        Ok(id_hex32) => {
                            *slot = Slot::Loading {
                                path: path.clone(),
                                id_hex32,
                            };
                            if let Some(s) = state.as_deref_mut() {
                                s.set_var(format!("tex.{k}.state"), "loading");
                            }
                        }
                        Err(e) => {
                            *slot = Slot::Failed {
                                path: path.clone(),
                                error: e.clone(),
                            };
                            if let Some(s) = state.as_deref_mut() {
                                s.set_var(format!("tex.{k}.state"), "failed");
                                s.set_var(format!("tex.{k}.error"), e);
                            }
                        }
                    },
                    Slot::Loading { path, id_hex32 } => {
                        match assets.state(id_hex32) {
                            Ok(crate::AssetState::Ready) => match assets.texture_rgba8_v1(id_hex32) {
                                Ok(texture) => match rgba8_to_color_image(
                                    texture.width,
                                    texture.height,
                                    &texture.rgba,
                                ) {
                                    Ok((img, w, h)) => {
                                        let handle = ctx.load_texture(
                                            format!("ui:{k}"),
                                            img,
                                            egui::TextureOptions::LINEAR,
                                        );

                                        let id_u64 = match handle.id() {
                                            egui::TextureId::User(u) => u,
                                            _ => 0,
                                        };

                                        if let Some(s) = state.as_deref_mut() {
                                            s.set_var(format!("tex.{k}"), id_u64.to_string());
                                            s.set_var(format!("tex.{k}.w"), w.to_string());
                                            s.set_var(format!("tex.{k}.h"), h.to_string());
                                            s.set_var(format!("tex.{k}.state"), "ready");
                                        }
                                        *slot = Slot::Ready { handle, w, h };
                                    }
                                    Err(e) => {
                                        *slot = Slot::Failed {
                                            path: path.clone(),
                                            error: e.clone(),
                                        };
                                        if let Some(s) = state.as_deref_mut() {
                                            s.set_var(format!("tex.{k}.state"), "failed");
                                            s.set_var(format!("tex.{k}.error"), e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    *slot = Slot::Failed {
                                        path: path.clone(),
                                        error: e.clone(),
                                    };
                                    if let Some(s) = state.as_deref_mut() {
                                        s.set_var(format!("tex.{k}.state"), "failed");
                                        s.set_var(format!("tex.{k}.error"), e);
                                    }
                                }
                            },
                            Ok(crate::AssetState::Loading) | Ok(crate::AssetState::Unloaded) => {
                                if let Some(s) = state.as_deref_mut() {
                                    s.set_var(format!("tex.{k}.state"), "loading");
                                }
                            }
                            Ok(crate::AssetState::Failed) => {
                                let e = format!("asset failed: {path}");
                                *slot = Slot::Failed {
                                    path: path.clone(),
                                    error: e.clone(),
                                };
                                if let Some(s) = state.as_deref_mut() {
                                    s.set_var(format!("tex.{k}.state"), "failed");
                                    s.set_var(format!("tex.{k}.error"), e);
                                }
                            }
                            Err(e) => {
                                *slot = Slot::Failed {
                                    path: path.clone(),
                                    error: e.clone(),
                                };
                                if let Some(s) = state.as_deref_mut() {
                                    s.set_var(format!("tex.{k}.state"), "failed");
                                    s.set_var(format!("tex.{k}.error"), e);
                                }
                            }
                            Ok(AssetState::Unknown) => {}
                        }
                    }
                    Slot::Ready { .. } => {
                        if let Some(s) = state.as_deref_mut() {
                            s.set_var(format!("tex.{k}.state"), "ready");
                        }
                    }
                    Slot::Failed { error, .. } => {
                        if let Some(s) = state.as_deref_mut() {
                            s.set_var(format!("tex.{k}.state"), "failed");
                            s.set_var(format!("tex.{k}.error"), error.clone());
                        }
                    }
                }
            }
        }
    }

    fn rgba8_to_color_image(
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(egui::ColorImage, u32, u32), String> {
        if width == 0 || height == 0 {
            return Err(format!("rgba8 texture has zero extent {width}x{height}"));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|px| px.checked_mul(4))
            .ok_or_else(|| "rgba8 texture dimensions overflow".to_string())?;
        if rgba.len() != expected {
            return Err(format!(
                "rgba8 payload size mismatch bytes={} expected={} extent={}x{}",
                rgba.len(),
                expected,
                width,
                height
            ));
        }

        let mut pixels: Vec<egui::Color32> =
            Vec::with_capacity((width as usize).saturating_mul(height as usize));
        for px in rgba.chunks_exact(4) {
            pixels.push(egui::Color32::from_rgba_unmultiplied(
                px[0], px[1], px[2], px[3],
            ));
        }

        Ok((
            egui::ColorImage {
                size: [width as usize, height as usize],
                source_size: Default::default(),
                pixels,
            },
            width,
            height,
        ))
    }
}

#[cfg(all(feature = "egui", feature = "images"))]
pub use with_images::UiImageLoader;
