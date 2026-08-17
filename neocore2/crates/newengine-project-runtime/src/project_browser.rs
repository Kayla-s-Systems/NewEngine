use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use eframe::egui;
use newengine_loading_api::bootstrap_ui::{north_star_bootstrap_ui_style, BootstrapUiRgb};
use newengine_project_api::{ProjectManifest, RuntimeLaunchProfile, PROJECT_MANIFEST_FILE};

#[derive(Clone, Debug)]
pub struct ProjectBrowserLaunchOption {
    pub id: String,
    pub profile: String,
    pub runtime_profile: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectBrowserEntry {
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub id: String,
    pub name: String,
    pub launcher: Option<String>,
    pub runtime_profile: Option<String>,
    pub launch_profile: Option<String>,
    pub launch_ids: Vec<String>,
    pub launch_options: Vec<ProjectBrowserLaunchOption>,
    pub default_launch: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectBrowserSelection {
    pub manifest_path: Option<PathBuf>,
    pub launch_id: Option<String>,
    pub cancelled: bool,
}

pub fn default_projects_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NEWENGINE_PROJECTS_ROOT") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }

    let mut seeds = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            seeds.push(parent.to_path_buf());
        }
    }
    for seed in seeds {
        for ancestor in seed.ancestors().take(8) {
            let candidate = ancestor.join("Projects");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn discover_projects(root: &Path) -> Vec<ProjectBrowserEntry> {
    let mut out = Vec::new();
    discover_recursive(root, 0, &mut out);
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

pub fn discover_game_projects(root: &Path) -> Vec<ProjectBrowserEntry> {
    discover_projects(root)
        .into_iter()
        .filter_map(|mut entry| {
            let source = fs::read_to_string(&entry.manifest_path).ok()?;
            let manifest = toml::from_str::<ProjectManifest>(&source).ok()?;
            let launch_id = game_launch_id(&manifest)?;
            entry.launch_profile = Some(RuntimeLaunchProfile::Game.id().to_owned());
            entry.launch_ids = vec![launch_id.clone()];
            entry.launch_options =
                manifest
                    .resolve_launch(Some(&launch_id))
                    .ok()
                    .map(|resolved| {
                        vec![ProjectBrowserLaunchOption {
                            id: launch_id.clone(),
                            profile: resolved.profile.id().to_owned(),
                            runtime_profile: resolved.runtime_profile,
                        }]
                    })?;
            entry.default_launch = launch_id;
            Some(entry)
        })
        .collect()
}

fn preferred_launch_id(manifest: &ProjectManifest) -> String {
    if let Some(default_launch) = manifest
        .default_launch
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return default_launch.to_owned();
    }
    if let Some(id) = manifest.launch_ids().into_iter().next() {
        return id;
    }
    manifest
        .launch_profile
        .map(|profile| profile.id().to_owned())
        .unwrap_or_else(|| "game".to_owned())
}

fn game_launch_id(manifest: &ProjectManifest) -> Option<String> {
    let profile_for = |id: &str| {
        manifest
            .launch
            .get(id)
            .and_then(|preset| preset.profile)
            .or(manifest.launch_profile)
            .unwrap_or_default()
    };

    if manifest.launch.contains_key("game") && profile_for("game") == RuntimeLaunchProfile::Game {
        return Some("game".to_owned());
    }
    if let Some(default_launch) = manifest
        .default_launch
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if manifest.launch.contains_key(default_launch)
            && profile_for(default_launch) == RuntimeLaunchProfile::Game
        {
            return Some(default_launch.to_owned());
        }
    }
    if let Some((id, _)) = manifest
        .launch
        .iter()
        .find(|(id, _)| profile_for(id) == RuntimeLaunchProfile::Game)
    {
        return Some(id.clone());
    }
    (manifest.launch_profile.unwrap_or_default() == RuntimeLaunchProfile::Game)
        .then(|| "game".to_owned())
}

fn discover_recursive(dir: &Path, depth: usize, out: &mut Vec<ProjectBrowserEntry>) {
    if depth > 6 {
        return;
    }
    let manifest_path = dir.join(PROJECT_MANIFEST_FILE);
    if manifest_path.is_file() {
        if let Ok(text) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = toml::from_str::<ProjectManifest>(&text) {
                if manifest.validate().is_ok() {
                    let default_launch = manifest
                        .default_launch
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .or_else(|| {
                            manifest
                                .launch_profile
                                .map(|profile| profile.id().to_owned())
                        })
                        .unwrap_or_else(|| "game".to_owned());
                    let mut launch_ids = manifest.launch_ids();
                    if !launch_ids.iter().any(|id| id == &default_launch) {
                        launch_ids.push(default_launch.clone());
                    }
                    let launch_options = launch_ids
                        .iter()
                        .filter_map(|id| {
                            manifest.resolve_launch(Some(id)).ok().map(|resolved| {
                                ProjectBrowserLaunchOption {
                                    id: id.clone(),
                                    profile: resolved.profile.id().to_owned(),
                                    runtime_profile: resolved.runtime_profile,
                                }
                            })
                        })
                        .collect();
                    out.push(ProjectBrowserEntry {
                        manifest_path,
                        project_root: dir.to_path_buf(),
                        id: manifest.id.clone(),
                        name: if manifest.name.trim().is_empty() {
                            manifest.id
                        } else {
                            manifest.name
                        },
                        launcher: manifest.launcher,
                        runtime_profile: manifest.runtime_profile,
                        launch_profile: manifest
                            .launch_profile
                            .map(|profile| profile.id().to_owned()),
                        launch_ids,
                        launch_options,
                        default_launch,
                    });
                    return;
                }
            }
        }
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                ".git" | "target" | "intermediate" | "node_modules"
            ) {
                continue;
            }
            discover_recursive(&path, depth + 1, out);
        }
    }
}

pub fn present_project_browser(root: &Path) -> Result<ProjectBrowserSelection, String> {
    let projects = discover_projects(root);
    if projects.is_empty() {
        return Err(format!(
            "no valid {PROJECT_MANIFEST_FILE} projects found under '{}'",
            root.display()
        ));
    }

    let outcome = Arc::new(Mutex::new(ProjectBrowserSelection::default()));
    let outcome_for_app = Arc::clone(&outcome);
    let recent = load_recent_project();
    let selected = recent
        .as_ref()
        .and_then(|recent| {
            projects
                .iter()
                .position(|entry| &entry.manifest_path == recent)
        })
        .unwrap_or(0);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("North Star - Project Browser")
            .with_inner_size([1060.0, 680.0])
            .with_min_inner_size([860.0, 560.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "North Star - Project Browser",
        options,
        Box::new(move |cc| {
            configure_style(&cc.egui_ctx);
            let selected_launch = projects
                .get(selected)
                .map(|entry| entry.default_launch.clone())
                .unwrap_or_else(|| "game".to_owned());
            Ok(Box::new(ProjectBrowserApp {
                projects,
                selected,
                selected_launch,
                manual_path: String::new(),
                remember: true,
                outcome: outcome_for_app,
            }))
        }),
    )
    .map_err(|error| format!("project browser presenter failed: {error}"))?;

    let selection = outcome
        .lock()
        .map(|value| value.clone())
        .unwrap_or(ProjectBrowserSelection {
            manifest_path: None,
            launch_id: None,
            cancelled: true,
        });
    if let Some(path) = selection.manifest_path.as_ref() {
        save_recent_project(path);
    }
    Ok(selection)
}

struct ProjectBrowserApp {
    projects: Vec<ProjectBrowserEntry>,
    selected: usize,
    selected_launch: String,
    manual_path: String,
    remember: bool,
    outcome: Arc<Mutex<ProjectBrowserSelection>>,
}

impl ProjectBrowserApp {
    fn open_selected(&mut self, ctx: &egui::Context) {
        let Some(entry) = self.projects.get(self.selected) else {
            return;
        };
        if self.remember {
            save_recent_project(&entry.manifest_path);
        }
        if let Ok(mut outcome) = self.outcome.lock() {
            outcome.manifest_path = Some(entry.manifest_path.clone());
            outcome.launch_id = Some(self.selected_launch.clone());
            outcome.cancelled = false;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn open_manual(&mut self, ctx: &egui::Context) {
        let value = self.manual_path.trim();
        if value.is_empty() {
            return;
        }
        let mut path = PathBuf::from(value);
        if path.is_dir() {
            path = path.join(PROJECT_MANIFEST_FILE);
        }
        if !path.is_file() {
            return;
        }
        let Some(manifest) = fs::read_to_string(&path)
            .ok()
            .and_then(|text| toml::from_str::<ProjectManifest>(&text).ok())
            .filter(|manifest| manifest.validate().is_ok())
        else {
            return;
        };
        let launch_id = preferred_launch_id(&manifest);
        if let Ok(mut outcome) = self.outcome.lock() {
            outcome.manifest_path = Some(path.clone());
            outcome.launch_id = Some(launch_id);
            outcome.cancelled = false;
        }
        if self.remember {
            save_recent_project(&path);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for ProjectBrowserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let bootstrap = north_star_bootstrap_ui_style();
        let palette = bootstrap.palette;

        egui::TopBottomPanel::top("project_browser_header")
            .exact_height(104.0)
            .frame(
                egui::Frame::none()
                    .fill(ui_color(palette.bg_deep))
                    .inner_margin(egui::Margin::symmetric(24.0, 18.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (symbol_rect, _) =
                        ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        symbol_rect,
                        egui::Rounding::same(10.0),
                        ui_color(palette.panel_active),
                    );
                    ui.painter().text(
                        symbol_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "N",
                        egui::FontId::proportional(24.0),
                        ui_color(palette.blue_bright),
                    );

                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("NORTH STAR ENGINE")
                                .size(22.0)
                                .strong()
                                .color(ui_color(palette.text)),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new("PROJECTS")
                                .size(12.0)
                                .strong()
                                .color(ui_color(palette.blue_bright)),
                        );
                        ui.label(
                            egui::RichText::new("Choose a project to open in NewEngine")
                                .size(12.0)
                                .color(ui_color(palette.text_dim)),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let count = format!("{} PROJECTS", self.projects.len());
                        ui.label(
                            egui::RichText::new(count)
                                .size(11.0)
                                .strong()
                                .color(ui_color(palette.muted)),
                        );
                    });
                });
            });

        egui::TopBottomPanel::bottom("project_browser_footer")
            .exact_height(58.0)
            .frame(
                egui::Frame::none()
                    .fill(ui_color(palette.bg_deep))
                    .inner_margin(egui::Margin::symmetric(22.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.remember, "Remember last project");
                    ui.label(
                        egui::RichText::new("Project mounts are isolated per game.toml")
                            .size(11.0)
                            .color(ui_color(palette.muted)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "OPEN PROJECT", palette.blue).clicked() {
                            self.open_selected(ctx);
                        }
                        if secondary_button(ui, "CANCEL", palette).clicked() {
                            if let Ok(mut outcome) = self.outcome.lock() {
                                outcome.cancelled = true;
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(ui_color(palette.bg))
                    .inner_margin(egui::Margin::same(22.0)),
            )
            .show(ctx, |ui| {
                ui.columns(2, |columns| {
                    columns[0].label(
                        egui::RichText::new("RECENT PROJECTS")
                            .size(12.0)
                            .strong()
                            .color(ui_color(palette.text_dim)),
                    );
                    columns[0].add_space(10.0);

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(&mut columns[0], |ui| {
                            for index in 0..self.projects.len() {
                                let entry = &self.projects[index];
                                let selected = self.selected == index;
                                let fill = if selected {
                                    ui_color(palette.panel_active)
                                } else {
                                    ui_color(palette.panel)
                                };
                                let stroke = if selected {
                                    ui_color(palette.blue)
                                } else {
                                    ui_color(palette.edge_soft)
                                };

                                let response = egui::Frame::none()
                                    .fill(fill)
                                    .stroke(egui::Stroke::new(1.0, stroke))
                                    .rounding(egui::Rounding::same(8.0))
                                    .inner_margin(egui::Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let (icon_rect, _) = ui.allocate_exact_size(
                                                egui::vec2(40.0, 40.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                icon_rect,
                                                egui::Rounding::same(7.0),
                                                ui_color(if selected {
                                                    palette.blue
                                                } else {
                                                    palette.bg_deep
                                                }),
                                            );
                                            let initial = entry
                                                .name
                                                .chars()
                                                .next()
                                                .unwrap_or('N')
                                                .to_uppercase()
                                                .to_string();
                                            ui.painter().text(
                                                icon_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                initial,
                                                egui::FontId::proportional(18.0),
                                                ui_color(palette.text),
                                            );
                                            ui.add_space(10.0);
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&entry.name)
                                                        .size(15.0)
                                                        .strong()
                                                        .color(ui_color(palette.text)),
                                                );
                                                ui.label(
                                                    egui::RichText::new(&entry.id)
                                                        .size(11.0)
                                                        .color(ui_color(palette.blue_bright)),
                                                );
                                            });
                                        });
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(
                                                entry.project_root.to_string_lossy(),
                                            )
                                            .size(10.5)
                                            .color(ui_color(palette.muted)),
                                        );
                                    })
                                    .response
                                    .interact(egui::Sense::click());

                                if response.clicked() {
                                    self.selected = index;
                                    self.selected_launch = entry.default_launch.clone();
                                }
                                if response.double_clicked() {
                                    self.selected = index;
                                    self.selected_launch = entry.default_launch.clone();
                                    self.open_selected(ctx);
                                }
                                ui.add_space(8.0);
                            }
                        });

                    columns[1].add_space(0.0);
                    egui::Frame::none()
                        .fill(ui_color(palette.panel))
                        .stroke(egui::Stroke::new(1.0, ui_color(palette.edge_soft)))
                        .rounding(egui::Rounding::same(8.0))
                        .inner_margin(egui::Margin::same(18.0))
                        .show(&mut columns[1], |ui| {
                            ui.label(
                                egui::RichText::new("PROJECT DETAILS")
                                    .size(12.0)
                                    .strong()
                                    .color(ui_color(palette.text_dim)),
                            );
                            ui.add_space(14.0);

                            if let Some(entry) = self.projects.get(self.selected) {
                                ui.label(
                                    egui::RichText::new(&entry.name)
                                        .size(24.0)
                                        .strong()
                                        .color(ui_color(palette.text)),
                                );
                                ui.label(
                                    egui::RichText::new(&entry.id)
                                        .size(12.0)
                                        .color(ui_color(palette.blue_bright)),
                                );
                                ui.add_space(18.0);

                                let launch_options = entry.launch_options.clone();
                                let selected_profile = launch_options
                                    .iter()
                                    .find(|option| option.id == self.selected_launch)
                                    .map(|option| option.profile.to_ascii_uppercase())
                                    .unwrap_or_else(|| self.selected_launch.to_ascii_uppercase());
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [86.0, 20.0],
                                        egui::Label::new(
                                            egui::RichText::new("PROFILE")
                                                .size(10.5)
                                                .strong()
                                                .color(ui_color(palette.muted)),
                                        ),
                                    );
                                    egui::ComboBox::from_id_salt("project_launch_profile")
                                        .selected_text(selected_profile)
                                        .width(220.0)
                                        .show_ui(ui, |ui| {
                                            for option in &launch_options {
                                                let profile = option.profile.to_ascii_uppercase();
                                                let label = if option.id == option.profile {
                                                    profile
                                                } else {
                                                    format!("{profile} · {}", option.id)
                                                };
                                                ui.selectable_value(
                                                    &mut self.selected_launch,
                                                    option.id.clone(),
                                                    label,
                                                );
                                            }
                                        });
                                });
                                ui.add_space(6.0);
                                detail_row(
                                    ui,
                                    "RUNTIME",
                                    selected_runtime_label(entry, &self.selected_launch),
                                    palette,
                                );
                                detail_row(ui, "DEFAULT", &entry.default_launch, palette);
                                ui.add_space(12.0);
                                detail_row(
                                    ui,
                                    "ROOT",
                                    &entry.project_root.to_string_lossy(),
                                    palette,
                                );
                                detail_row(
                                    ui,
                                    "MANIFEST",
                                    &entry.manifest_path.to_string_lossy(),
                                    palette,
                                );
                            }

                            ui.add_space(22.0);
                            ui.separator();
                            ui.add_space(14.0);
                            ui.label(
                                egui::RichText::new("OPEN ANOTHER PROJECT")
                                    .size(11.0)
                                    .strong()
                                    .color(ui_color(palette.text_dim)),
                            );
                            ui.add_space(7.0);
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [ui.available_width() - 112.0, 34.0],
                                    egui::TextEdit::singleline(&mut self.manual_path)
                                        .hint_text("Path to project directory or game.toml"),
                                );
                                if secondary_button(ui, "BROWSE", palette).clicked() {
                                    self.open_manual(ctx);
                                }
                            });
                            ui.add_space(18.0);
                            if primary_button(ui, "OPEN SELECTED PROJECT", palette.blue).clicked() {
                                self.open_selected(ctx);
                            }
                        });
                });
            });
    }
}

fn ui_color(rgb: BootstrapUiRgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

fn selected_runtime_label<'a>(entry: &'a ProjectBrowserEntry, launch_id: &str) -> &'a str {
    entry
        .launch_options
        .iter()
        .find(|option| option.id == launch_id)
        .and_then(|option| option.runtime_profile.as_deref())
        .or(entry.runtime_profile.as_deref())
        .or(entry.launcher.as_deref())
        .unwrap_or("current NewEngine runtime")
}

fn detail_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    palette: newengine_loading_api::bootstrap_ui::BootstrapUiPalette,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(
            [86.0, 20.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(10.5)
                    .strong()
                    .color(ui_color(palette.muted)),
            ),
        );
        ui.label(
            egui::RichText::new(value)
                .size(11.5)
                .color(ui_color(palette.text_dim)),
        );
    });
    ui.add_space(6.0);
}

fn primary_button(ui: &mut egui::Ui, label: &str, color: BootstrapUiRgb) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.0)
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(ui_color(color))
        .rounding(egui::Rounding::same(6.0))
        .min_size(egui::vec2(150.0, 36.0)),
    )
}

fn secondary_button(
    ui: &mut egui::Ui,
    label: &str,
    palette: newengine_loading_api::bootstrap_ui::BootstrapUiPalette,
) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.0)
                .strong()
                .color(ui_color(palette.text_dim)),
        )
        .fill(ui_color(palette.panel))
        .stroke(egui::Stroke::new(1.0, ui_color(palette.edge)))
        .rounding(egui::Rounding::same(6.0))
        .min_size(egui::vec2(96.0, 36.0)),
    )
}

fn configure_style(ctx: &egui::Context) {
    configure_runtime_fonts(ctx);
    let palette = north_star_bootstrap_ui_style().palette;

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    ctx.set_style(style);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = ui_color(palette.bg);
    visuals.window_fill = ui_color(palette.bg_deep);
    visuals.extreme_bg_color = ui_color(palette.bg_deep);
    visuals.faint_bg_color = ui_color(palette.panel);
    visuals.override_text_color = Some(ui_color(palette.text));
    visuals.hyperlink_color = ui_color(palette.blue_bright);
    visuals.selection.bg_fill = ui_color(palette.panel_active);
    visuals.selection.stroke = egui::Stroke::new(1.0, ui_color(palette.blue));
    visuals.widgets.noninteractive.bg_fill = ui_color(palette.bg);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, ui_color(palette.text_dim));
    visuals.widgets.inactive.bg_fill = ui_color(palette.panel);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, ui_color(palette.edge_soft));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, ui_color(palette.text_dim));
    visuals.widgets.hovered.bg_fill = ui_color(palette.panel_active);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ui_color(palette.blue));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, ui_color(palette.text));
    visuals.widgets.active.bg_fill = ui_color(palette.blue);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ui_color(palette.blue_bright));
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.window_rounding = egui::Rounding::same(8.0);
    visuals.widgets.active.rounding = egui::Rounding::same(6.0);
    visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
    visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
    ctx.set_visuals(visuals);
}

fn configure_runtime_fonts(ctx: &egui::Context) {
    let Some((name, bytes)) = load_system_ui_font() else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(name.clone(), egui::FontData::from_owned(bytes).into());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push(name.clone());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(name);
    ctx.set_fonts(fonts);
}

fn load_system_ui_font() -> Option<(String, Vec<u8>)> {
    #[cfg(windows)]
    {
        let windows_root = std::env::var_os("WINDIR")
            .or_else(|| std::env::var_os("SystemRoot"))
            .map(PathBuf::from)?;
        for filename in ["segoeui.ttf", "arial.ttf"] {
            let path = windows_root.join("Fonts").join(filename);
            if let Ok(bytes) = fs::read(&path) {
                if !bytes.is_empty() {
                    return Some((format!("system-ui:{filename}"), bytes));
                }
            }
        }
    }

    None
}

fn recent_file_path() -> Option<PathBuf> {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local)
                .join("NorthStar")
                .join("project_browser_recent.txt"),
        );
    }
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(".newengine").join("project_browser_recent.txt"))
}

fn load_recent_project() -> Option<PathBuf> {
    let path = recent_file_path()?;
    let value = fs::read_to_string(path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn save_recent_project(project: &Path) {
    let Some(path) = recent_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, project.to_string_lossy().as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_root_discovery_is_bounded() {
        if let Some(root) = default_projects_root() {
            assert!(root.ends_with("Projects"));
        }
    }

    #[test]
    fn game_launch_selector_rejects_editor_only_manifest() {
        let mut manifest = ProjectManifest::default();
        manifest.launch_profile = Some(RuntimeLaunchProfile::Editor);
        manifest.default_launch = Some("editor".to_owned());
        manifest.launch.insert(
            "editor".to_owned(),
            newengine_project_api::ProjectLaunchPreset {
                profile: Some(RuntimeLaunchProfile::Editor),
                ..Default::default()
            },
        );
        assert_eq!(game_launch_id(&manifest), None);
    }

    #[test]
    fn preferred_launch_selector_accepts_editor_only_manifest() {
        let mut manifest = ProjectManifest::default();
        manifest.launch_profile = Some(RuntimeLaunchProfile::Editor);
        manifest.default_launch = Some("editor".to_owned());
        manifest.launch.insert(
            "editor".to_owned(),
            newengine_project_api::ProjectLaunchPreset {
                profile: Some(RuntimeLaunchProfile::Editor),
                ..Default::default()
            },
        );
        assert_eq!(preferred_launch_id(&manifest), "editor");
    }

    #[test]
    fn game_launch_selector_prefers_declared_game_preset() {
        let mut manifest = ProjectManifest::default();
        manifest.launch_profile = Some(RuntimeLaunchProfile::Editor);
        manifest.default_launch = Some("editor".to_owned());
        manifest.launch.insert(
            "game".to_owned(),
            newengine_project_api::ProjectLaunchPreset {
                profile: Some(RuntimeLaunchProfile::Game),
                ..Default::default()
            },
        );
        assert_eq!(game_launch_id(&manifest).as_deref(), Some("game"));
    }
}
