#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::dock;
use super::panels;
use super::EditorUiBuild;

#[inline]
pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    panels::menubar::draw(me, ctx);
    panels::top_toolbar::draw(me, ctx);
    panels::status_bar::draw(me, ctx);
    dock::draw(me, ctx);
    panels::asset_manager::draw(me, ctx);
    panels::scene_io::draw(me, ctx);
    panels::command_palette::draw(me, ctx);
}
