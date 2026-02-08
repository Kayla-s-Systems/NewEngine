#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "egui")]
use crate::markup::substitute::substitute_vars;
#[cfg(feature = "egui")]
use crate::markup::theme::{UiDensity, UiThemeDesc, UiVisuals};
#[cfg(feature = "egui")]
use crate::markup::ui_node::UiNode;
#[cfg(feature = "egui")]
use crate::markup::{UiEvent, UiEventKind, UiMarkupDoc, UiState};

#[cfg(feature = "egui")]
pub(crate) fn render_doc(doc: &UiMarkupDoc, ctx: &egui::Context, state: &mut UiState) {
    apply_theme(ctx, &doc.theme);
    render_root(&doc.root, ctx, state);
}

#[cfg(feature = "egui")]
fn render_root(root: &UiNode, ctx: &egui::Context, state: &mut UiState) {
    match root {
        UiNode::Ui { children } => {
            for c in children {
                render_root(c, ctx, state);
            }
        }
        UiNode::TopBar { children } => {
            egui::TopBottomPanel::top("ui_topbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for c in children {
                        render_in_ui(c, ui, state);
                    }
                });
            });
        }
        UiNode::Window {
            title,
            open,
            children,
        } => {
            let mut is_open = *open;
            egui::Window::new(title).open(&mut is_open).show(ctx, |ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        _ => {}
    }
}

#[cfg(feature = "egui")]
fn render_in_ui(node: &UiNode, ui: &mut egui::Ui, state: &mut UiState) {
    match node {
        UiNode::Row { children } => {
            ui.horizontal(|ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Column { children } => {
            ui.vertical(|ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Label { id, text } => {
            let base = if let Some(id) = id.as_deref() {
                state
                    .strings
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or(text.as_str())
            } else {
                text.as_str()
            };
            let s = substitute_vars(base, &state.vars);
            ui.label(s.as_ref());
        }
        UiNode::Button { id, text, on_click } => {
            let s = substitute_vars(text, &state.vars);
            if ui.button(s.as_ref()).clicked() {
                state.clicked.insert(id.clone(), true);

                if !on_click.is_empty() {
                    state.push_event(UiEvent {
                        kind: UiEventKind::Click,
                        target_id: id.clone(),
                        value: None,
                        actions: on_click.clone(),
                    });
                }
            }
        }
        UiNode::TextBox {
            id,
            hint,
            bind,
            multiline,
            on_change,
            on_submit,
        } => {
            let hint = substitute_vars(hint, &state.vars);

            let (changed, submit_now, value_snapshot) = {
                let entry = state.strings.entry(bind.clone()).or_default();

                let resp = if *multiline {
                    ui.add(
                        egui::TextEdit::multiline(entry)
                            .hint_text(hint.as_ref())
                            .desired_width(f32::INFINITY),
                    )
                } else {
                    ui.add(
                        egui::TextEdit::singleline(entry)
                            .hint_text(hint.as_ref())
                            .desired_width(f32::INFINITY),
                    )
                };

                let changed = resp.changed();
                let submit_now = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                (changed, submit_now, entry.clone())
            };

            if changed {
                state.vars.insert(id.clone(), value_snapshot.clone());

                if !on_change.is_empty() {
                    state.push_event(UiEvent {
                        kind: UiEventKind::Change,
                        target_id: id.clone(),
                        value: Some(value_snapshot.clone()),
                        actions: on_change.clone(),
                    });
                }
            }

            if submit_now && !on_submit.is_empty() {
                state.push_event(UiEvent {
                    kind: UiEventKind::Submit,
                    target_id: id.clone(),
                    value: Some(value_snapshot),
                    actions: on_submit.clone(),
                });
            }
        }
        UiNode::Spacer => ui.add_space(8.0),
        UiNode::TopBar { children } => {
            ui.horizontal(|ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Window { .. } => {}
        UiNode::Ui { children } => {
            for c in children {
                render_in_ui(c, ui, state);
            }
        }
        UiNode::Unknown { tag, children } => {
            *state.unknown_tags.entry(tag.clone()).or_insert(0) += 1;
            for c in children {
                render_in_ui(c, ui, state);
            }
        }
    }
}

#[cfg(feature = "egui")]
fn apply_theme(ctx: &egui::Context, theme: &UiThemeDesc) {
    let mut style = (*ctx.style()).clone();

    match theme.visuals {
        UiVisuals::Auto => {}
        UiVisuals::Dark => style.visuals = egui::Visuals::dark(),
        UiVisuals::Light => style.visuals = egui::Visuals::light(),
    }

    let s = theme.scale;
    style.spacing.item_spacing *= s;
    style.spacing.window_margin *= s;
    style.spacing.button_padding *= s;
    style.spacing.indent *= s;
    style.spacing.interact_size *= s;

    match theme.density {
        UiDensity::Default => {}
        UiDensity::Compact => {
            style.spacing.item_spacing *= 0.85;
            style.spacing.button_padding *= 0.90;
        }
        UiDensity::Dense => {
            style.spacing.item_spacing *= 0.75;
            style.spacing.button_padding *= 0.85;
        }
        UiDensity::Tight => {
            style.spacing.item_spacing *= 0.65;
            style.spacing.button_padding *= 0.80;
        }
    }

    style.override_font_id = Some(egui::FontId::proportional(theme.font_size));
    ctx.set_style(style);
}