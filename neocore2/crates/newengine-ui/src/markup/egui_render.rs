#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "egui")]
use crate::markup::substitute::substitute_vars;
#[cfg(feature = "egui")]
use crate::markup::theme::{UiDensity, UiThemeDesc, UiVisuals};
#[cfg(feature = "egui")]
use crate::markup::ui_node::{UiIconSide, UiNode};
#[cfg(feature = "egui")]
use crate::markup::{UiEvent, UiEventKind, UiMarkupDoc, UiState};
#[cfg(feature = "egui")]
use serde_json::Value;

#[cfg(feature = "egui")]
fn resolve_tex_id(spec: &str) -> Option<egui::TextureId> {
    let s = spec.trim();
    if s.is_empty() {
        return None;
    }

    let s = s.strip_prefix("user:").unwrap_or(s);
    let s = s.strip_prefix("tex:").unwrap_or(s);
    let id = s.parse::<u64>().ok()?;
    Some(egui::TextureId::User(id))
}

#[cfg(feature = "egui")]
fn parse_tint_rgba(tint: &str) -> Option<egui::Color32> {
    let t = tint.trim();
    if t.is_empty() {
        return None;
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(egui::Color32::from_rgba_unmultiplied(r, g, b, 255));
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
        }
        _ => return None,
    }
}

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
        UiNode::Label {
            id,
            text,
            icon,
            icon_side,
            icon_size,
        } => {
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

            let icon_spec = icon.as_deref().map(|v| substitute_vars(v, &state.vars));
            let icon_tex = icon_spec
                .as_ref()
                .and_then(|v| resolve_tex_id(v.as_ref()));
            let size = icon_size.unwrap_or(14.0).clamp(8.0, 64.0);

            if let Some(tex_id) = icon_tex {
                ui.horizontal(|ui| {
                    match icon_side {
                        UiIconSide::Left => {
                            ui.add(egui::Image::new((tex_id, egui::vec2(size, size))));
                            ui.label(s.as_ref());
                        }
                        UiIconSide::Right => {
                            ui.label(s.as_ref());
                            ui.add(egui::Image::new((tex_id, egui::vec2(size, size))));
                        }
                    }
                });
            } else {
                ui.label(s.as_ref());
            }
        }
        UiNode::Button {
            id,
            text,
            icon,
            icon_side,
            icon_size,
            on_click,
        } => {
            let s = substitute_vars(text, &state.vars);

            let icon_spec = icon.as_deref().map(|v| substitute_vars(v, &state.vars));
            let icon_tex = icon_spec
                .as_ref()
                .and_then(|v| resolve_tex_id(v.as_ref()));
            let size = icon_size.unwrap_or(14.0).clamp(8.0, 64.0);

            let clicked = if let Some(tex_id) = icon_tex {
                ui.horizontal(|ui| {
                    match icon_side {
                        UiIconSide::Left => {
                            ui.add(egui::Image::new((tex_id, egui::vec2(size, size))));
                            ui.button(s.as_ref()).clicked()
                        }
                        UiIconSide::Right => {
                            let c = ui.button(s.as_ref()).clicked();
                            ui.add(egui::Image::new((tex_id, egui::vec2(size, size))));
                            c
                        }
                    }
                })
                    .inner
            } else {
                ui.button(s.as_ref()).clicked()
            };

            if clicked {
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
        UiNode::Image { id, tex, size, tint } => {
            let spec = substitute_vars(tex, &state.vars);
            let Some(tex_id) = resolve_tex_id(spec.as_ref()) else {
                if let Some(id) = id.as_deref() {
                    *state.unknown_tags.entry(format!("image:missing_tex:{id}")).or_insert(0) +=
                        1;
                }
                return;
            };

            let size = size.unwrap_or([16.0, 16.0]);
            let tint = tint
                .as_deref()
                .map(|t| substitute_vars(t, &state.vars))
                .and_then(|t| parse_tint_rgba(t.as_ref()))
                .unwrap_or(egui::Color32::WHITE);

            ui.add(egui::Image::new((tex_id, egui::vec2(size[0], size[1]))).tint(tint));
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
        UiNode::Checkbox {
            id,
            text,
            bind,
            on_change,
        } => {
            let text = substitute_vars(text, &state.vars);

            let cur = state
                .vars
                .get(bind)
                .map(|s| {
                    let v = s.trim().to_ascii_lowercase();
                    v == "1" || v == "true" || v == "yes" || v == "on"
                })
                .unwrap_or(false);

            let mut v = cur;
            let changed = ui.checkbox(&mut v, text.as_ref()).changed();
            if changed {
                let v_str = if v { "true" } else { "false" };
                state.vars.insert(bind.clone(), v_str.to_string());

                if !on_change.is_empty() {
                    state.push_event(UiEvent {
                        kind: UiEventKind::Change,
                        target_id: id.clone(),
                        value: Some(v_str.to_string()),
                        actions: on_change.clone(),
                    });
                }
            }
        }
        UiNode::Select {
            id,
            bind,
            options,
            on_change,
        } => {
            let cur = state.vars.get(bind).cloned().unwrap_or_default();
            let mut selected = cur;

            let selected_label = options
                .iter()
                .find(|(v, _)| *v == selected)
                .map(|(_, l)| l.as_str())
                .unwrap_or("<select>");

            let mut changed = false;
            egui::ComboBox::from_id_salt(id)
                .selected_text(selected_label)
                .show_ui(ui, |ui| {
                    for (v, l) in options.iter() {
                        if ui.selectable_value(&mut selected, v.clone(), l).clicked() {
                            changed = true;
                        }
                    }
                });

            if changed {
                state.vars.insert(bind.clone(), selected.clone());

                if !on_change.is_empty() {
                    state.push_event(UiEvent {
                        kind: UiEventKind::Change,
                        target_id: id.clone(),
                        value: Some(selected),
                        actions: on_change.clone(),
                    });
                }
            }
        }
        UiNode::Separator => {
            ui.separator();
        }
        UiNode::Scroll { id, children } => {
            let mut sa = egui::ScrollArea::vertical().auto_shrink([false; 2]);
            if let Some(id) = id.as_deref() {
                sa = sa.id_salt(id);
            }
            sa.show(ui, |ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Repeat {
            items,
            as_name,
            children,
        } => {
            // items must be a JSON array of objects.
            let Some(src) = state.vars.get(items).cloned() else {
                return;
            };

            let parsed: Value = match serde_json::from_str(&src) {
                Ok(v) => v,
                Err(_) => return,
            };
            let Some(arr) = parsed.as_array() else {
                return;
            };

            // Backup vars once, then overlay per item.
            let base_vars = state.vars.clone();

            for it in arr.iter() {
                state.vars = base_vars.clone();

                // Inject object fields as "$as_name.key".
                if let Some(obj) = it.as_object() {
                    for (k, v) in obj.iter() {
                        let key = format!("{as}.{k}", as = as_name);
                        let val = match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => v.to_string(),
                        };
                        state.vars.insert(key, val);
                    }
                }

                for c in children {
                    render_in_ui(c, ui, state);
                }
            }

            // Restore.
            state.vars = base_vars;
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