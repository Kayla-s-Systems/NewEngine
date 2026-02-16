#![forbid(unsafe_op_in_unsafe_fn)]

use ahash::AHashMap;

use crate::asset_access::AssetAccess;

#[cfg(all(feature = "egui", feature = "images"))]
use image::GenericImageView;

/// Deterministic UI texture loader for icons and textured widgets.
///
/// Design goals:
/// - No hidden background threads.
/// - Non-blocking: does not wait; relies on `AssetAccess::pump()` + `AssetAccess::state()`.
/// - Stable handles: once loaded, returns a persistent `egui::TextureId::User(u64)`.
///
/// Usage pattern (per-frame):
/// 1) `loader.request(assets, "pm.refresh", "ui/icons/refresh.png")`
/// 2) `loader.pump(ctx, assets, state)`
/// 3) Markup refers to `$tex.pm.refresh` (a u64).
#[derive(Debug, Default)]
pub struct UiImageLoader {
    slots: AHashMap<String, Slot>,
}

#[cfg(all(feature = "egui", feature = "images"))]
type TexHandle = egui::TextureHandle;

#[cfg(not(all(feature = "egui", feature = "images")))]
type TexHandle = ();

#[derive(Debug, Clone)]
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
    pub fn request(&mut self, _assets: &dyn AssetAccess, key: impl Into<String>, path: impl Into<String>) {
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
    #[cfg(all(feature = "egui", feature = "images"))]
    pub fn pump(&mut self, ctx: &egui::Context, assets: &dyn AssetAccess, state: &mut crate::markup::UiState) {
        // 1) Advance pending loads.
        let keys: Vec<String> = self.slots.keys().cloned().collect();
        for k in keys {
            let slot = self.slots.get_mut(&k).unwrap();
            match slot {
                Slot::Empty { path } => {
                    match assets.load(path) {
                        Ok(id_hex32) => {
                            *slot = Slot::Loading {
                                path: path.clone(),
                                id_hex32,
                            };
                            state.set_var(format!("tex.{k}.state"), "loading");
                        }
                        Err(e) => {
                            *slot = Slot::Failed {
                                path: path.clone(),
                                error: e.clone(),
                            };
                            state.set_var(format!("tex.{k}.state"), "failed");
                            state.set_var(format!("tex.{k}.error"), e);
                        }
                    }
                }
                Slot::Loading { path, id_hex32 } => {
                    assets.pump();
                    match assets.state(id_hex32) {
                        Ok(crate::AssetState::Ready) => {
                            match assets.blob_wire_v1(id_hex32) {
                                Ok((_meta, bytes)) => match decode_to_color_image(&bytes) {
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

                                        state.set_var(format!("tex.{k}"), id_u64.to_string());
                                        state.set_var(format!("tex.{k}.w"), w.to_string());
                                        state.set_var(format!("tex.{k}.h"), h.to_string());
                                        state.set_var(format!("tex.{k}.state"), "ready");
                                        *slot = Slot::Ready { handle, w, h };
                                    }
                                    Err(e) => {
                                        *slot = Slot::Failed {
                                            path: path.clone(),
                                            error: e.clone(),
                                        };
                                        state.set_var(format!("tex.{k}.state"), "failed");
                                        state.set_var(format!("tex.{k}.error"), e);
                                    }
                                },
                                Err(e) => {
                                    *slot = Slot::Failed {
                                        path: path.clone(),
                                        error: e.clone(),
                                    };
                                    state.set_var(format!("tex.{k}.state"), "failed");
                                    state.set_var(format!("tex.{k}.error"), e);
                                }
                            }
                        }
                        Ok(crate::AssetState::Loading) | Ok(crate::AssetState::Unloaded) => {
                            state.set_var(format!("tex.{k}.state"), "loading");
                        }
                        Ok(crate::AssetState::Failed) => {
                            let e = format!("asset failed: {path}");
                            *slot = Slot::Failed {
                                path: path.clone(),
                                error: e.clone(),
                            };
                            state.set_var(format!("tex.{k}.state"), "failed");
                            state.set_var(format!("tex.{k}.error"), e);
                        }
                        Err(e) => {
                            *slot = Slot::Failed {
                                path: path.clone(),
                                error: e.clone(),
                            };
                            state.set_var(format!("tex.{k}.state"), "failed");
                            state.set_var(format!("tex.{k}.error"), e);
                        }
                    }
                }
                Slot::Ready { .. } => {
                    state.set_var(format!("tex.{k}.state"), "ready");
                }
                Slot::Failed { error, .. } => {
                    state.set_var(format!("tex.{k}.state"), "failed");
                    state.set_var(format!("tex.{k}.error"), error.clone());
                }
            }
        }
    }
}

#[cfg(all(feature = "egui", feature = "images"))]
fn decode_to_color_image(bytes: &[u8]) -> Result<(egui::ColorImage, u32, u32), String> {
    let dyn_img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = dyn_img.dimensions();

    let mut pixels: Vec<egui::Color32> = Vec::with_capacity((w as usize).saturating_mul(h as usize));
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
    }

    Ok((egui::ColorImage { size: [w as usize, h as usize], pixels }, w, h))
}
