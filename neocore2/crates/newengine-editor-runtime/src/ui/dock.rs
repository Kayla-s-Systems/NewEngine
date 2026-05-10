#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use egui_dock::{DockArea, DockState, NodeIndex, Style, SurfaceIndex, TabViewer};

use super::panels;
use super::theme;
use super::{EditorUiBuild, WorkspacePreset};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum EditorDockTab {
    Hierarchy,
    Viewport,
    Inspector,
    AssetBrowser,
    Console,
    Profiler,
}

impl EditorDockTab {
    pub(crate) const ALL: [Self; 6] = [
        Self::Hierarchy,
        Self::Viewport,
        Self::Inspector,
        Self::AssetBrowser,
        Self::Console,
        Self::Profiler,
    ];

    #[inline]
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Hierarchy => "Scene Hierarchy",
            Self::Viewport => "Viewport",
            Self::Inspector => "Inspector",
            Self::AssetBrowser => "Asset Browser",
            Self::Console => "Console",
            Self::Profiler => "Profiler",
        }
    }
}

#[inline]
pub(crate) fn dock_state_for_preset(preset: WorkspacePreset) -> DockState<EditorDockTab> {
    let mut dock = DockState::new(vec![EditorDockTab::Viewport]);
    let surface = dock.main_surface_mut();
    let [main_node, _left_node] = surface.split_left(NodeIndex::root(), 0.22, vec![EditorDockTab::Hierarchy]);
    let [main_node, _right_node] = surface.split_right(main_node, 0.28, vec![EditorDockTab::Inspector]);

    match preset {
        WorkspacePreset::Minimal => {}
        WorkspacePreset::Editing => {
            surface.split_below(
                main_node,
                0.72,
                vec![EditorDockTab::AssetBrowser, EditorDockTab::Console, EditorDockTab::Profiler],
            );
        }
        WorkspacePreset::Debug => {
            surface.split_below(
                main_node,
                0.68,
                vec![EditorDockTab::Console, EditorDockTab::Profiler, EditorDockTab::AssetBrowser],
            );
        }
    }

    dock
}

#[inline]
fn closed_tabs(dock_state: &DockState<EditorDockTab>) -> Vec<EditorDockTab> {
    let open: newengine_math::collections_prelude::NeHashSet<_> = dock_state
        .iter_all_tabs()
        .map(|(_, tab)| *tab)
        .collect();
    EditorDockTab::ALL
        .into_iter()
        .filter(|tab| !open.contains(tab))
        .collect()
}

struct EditorDockViewer<'a> {
    me: &'a mut EditorUiBuild,
    closed_tabs: Vec<EditorDockTab>,
    pending_open: &'a mut Option<(SurfaceIndex, NodeIndex, EditorDockTab)>,
    ctx: &'a egui::Context,
}

impl TabViewer for EditorDockViewer<'_> {
    type Tab = EditorDockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            EditorDockTab::Hierarchy => panels::hierarchy::draw_content(self.me, ui),
            EditorDockTab::Viewport => panels::viewport::draw_content(self.me, self.ctx, ui),
            EditorDockTab::Inspector => panels::inspector::draw_content(self.me, ui),
            EditorDockTab::AssetBrowser => panels::asset_browser::draw_content(self.me, ui),
            EditorDockTab::Console => panels::console::draw_content(self.me, ui),
            EditorDockTab::Profiler => panels::profiler::draw_content(self.me, ui),
        }
    }

    fn add_popup(&mut self, ui: &mut egui::Ui, surface: SurfaceIndex, node: NodeIndex) {
        let tabs = self.closed_tabs.clone();
        if tabs.is_empty() {
            ui.label("All core panels are already open.");
            return;
        }

        for tab in tabs {
            if ui.button(tab.title()).clicked() {
                *self.pending_open = Some((surface, node, tab));
                ui.close();
            }
        }
    }
}

#[inline]
pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    let preset = me.workspace_preset;
    let mut dock_state = std::mem::replace(&mut me.dock_state, dock_state_for_preset(preset));
    let closed = closed_tabs(&dock_state);
    let mut pending_open = None;
    egui::CentralPanel::default().show(ctx, |ui| {
        let mut style = Style::from_egui(ui.style().as_ref());
        theme::tune_dock_style(&mut style, ui.visuals());
        DockArea::new(&mut dock_state)
            .id(egui::Id::new("newengine.editor.dock"))
            .style(style)
            .show_add_buttons(true)
            .show_add_popup(true)
            .show_close_buttons(true)
            .show_tab_name_on_hover(true)
            .show_leaf_collapse_buttons(true)
            .show_inside(
                ui,
                &mut EditorDockViewer {
                    me,
                    closed_tabs: closed.clone(),
                    pending_open: &mut pending_open,
                    ctx,
                },
            );
    });
    if let Some((surface, node, tab)) = pending_open {
        dock_state[surface].set_focused_node(node);
        dock_state[surface].push_to_focused_leaf(tab);
    }
    me.dock_state = dock_state;
}
