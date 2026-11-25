use crate::settings::{SettingsAction, SettingsCategory, SettingsDialogState};
use crate::theme::*;
use crate::viewport::{Selection, ViewMode, ViewportAction, ViewportState, draw_cad_viewport};
use egui::{Color32, CornerRadius, Frame, Key, Margin, RichText, ScrollArea, Stroke, Vec2};
use nanxing_core::config::AppConfig;
use nanxing_core::csv_edges::apply_csv_edges_from_file;
use nanxing_core::model::{Operation, PanelProgram};
use nanxing_core::scx_writer::render_scx_xml;
use nanxing_core::transforms::format_transforms_chip;
use nanxing_core::{LoadedStepPanel, format_num, load_step_panel, save_program_scx};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Operations,
    ProgramCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveModal {
    ExportAll,
    About,
}

pub struct NanxingStudioApp {
    pub cfg_path: PathBuf,
    pub cfg: AppConfig,
    pub settings_dlg: SettingsDialogState,
    pub panels: Vec<LoadedStepPanel>,
    pub selected_idx: usize,
    pub selection: Option<Selection>,
    pub hover_edge_from_inspector: Option<u8>,
    pub search_query: String,
    pub viewport: ViewportState,
    pub bottom_tab: BottomTab,
    pub active_folder_path: String,
    pub status_msg: String,
    pub status_highlight: bool,
    active_modal: Option<ActiveModal>,
}

impl NanxingStudioApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_industrial_theme(&cc.egui_ctx);

        let cfg_path = resolve_config_path();
        let cfg = AppConfig::load_or_create(&cfg_path);
        let start_path = if !cfg.default_open_dir.trim().is_empty() {
            cfg.default_open_dir.clone()
        } else {
            "Brak wczytanego folderu STEP".to_string()
        };

        Self {
            cfg_path,
            cfg,
            settings_dlg: SettingsDialogState::default(),
            panels: Vec::new(),
            selected_idx: 0,
            selection: None,
            hover_edge_from_inspector: None,
            search_query: String::new(),
            viewport: ViewportState::default(),
            bottom_tab: BottomTab::Operations,
            active_folder_path: start_path,
            status_msg: "Gotowy — otwórz plik lub folder STEP, aby wygenerować .scx".to_string(),
            status_highlight: false,
            active_modal: None,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = msg.into();
        self.status_highlight = true;
    }

    fn save_config(&mut self) {
        if let Err(err) = self.cfg.save(&self.cfg_path) {
            self.set_status(format!("Uwaga: {err}"));
        }
    }

    fn redetect_all(&mut self) {
        let opts = self.cfg.to_convert_options();
        for p in &mut self.panels {
            p.redetect(&opts);
        }
    }

    fn is_panel_skipped(&self, p: &LoadedStepPanel) -> bool {
        p.manual_skip || (self.cfg.skip_empty_panels && p.program.operations.is_empty())
    }

    fn exportable_count(&self) -> usize {
        self.panels
            .iter()
            .filter(|p| !self.is_panel_skipped(p))
            .count()
    }

    fn open_step_folder_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new().set_title("Wybierz folder z plikami .STEP / .STP");
        if let Some(dir) = existing_dir(&self.cfg.default_open_dir) {
            dlg = dlg.set_directory(dir);
        }
        let Some(folder) = dlg.pick_folder() else {
            return;
        };

        let mut paths: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let p = entry.path();
                if is_step_file(&p) {
                    paths.push(p);
                }
            }
        }
        paths.sort();

        if paths.is_empty() {
            self.set_status("Nie znaleziono plików .step / .stp w wybranym folderze");
            return;
        }

        self.load_step_paths(&paths, Some(folder));
    }

    fn open_step_files_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new()
            .set_title("Wybierz pliki .STEP / .STP")
            .add_filter("STEP 3D CAD (*.step, *.stp)", &["step", "stp", "STEP", "STP"]);
        if let Some(dir) = existing_dir(&self.cfg.default_open_dir) {
            dlg = dlg.set_directory(dir);
        }
        let Some(files) = dlg.pick_files() else {
            return;
        };
        let parent = files.first().and_then(|f| f.parent()).map(Path::to_path_buf);
        self.load_step_paths(&files, parent);
    }

    fn load_step_paths(&mut self, paths: &[PathBuf], folder: Option<PathBuf>) {
        let opts = self.cfg.to_convert_options();
        let mut loaded = Vec::new();
        let mut errors = 0usize;
        let mut last_err = String::new();

        for p in paths {
            match load_step_panel(p, &opts) {
                Ok(panel) => loaded.push(panel),
                Err(e) => {
                    errors += 1;
                    last_err = e;
                }
            }
        }

        if loaded.is_empty() {
            self.set_status(format!("Błąd odczytu STEP: {last_err}"));
            return;
        }

        if let Some(dir) = folder {
            self.active_folder_path = dir.display().to_string();
        }
        let count = loaded.len();
        let total_ops: usize = loaded.iter().map(|p| p.program.operations.len()).sum();
        self.panels = loaded;
        self.selected_idx = 0;
        self.selection = None;
        self.viewport.reset_camera();

        if errors > 0 {
            self.set_status(format!(
                "Wczytano {count} plików STEP ({total_ops} operacji) · błędów: {errors}"
            ));
        } else {
            self.set_status(format!("Wczytano {count} plików STEP · wykryto {total_ops} operacji"));
        }
    }

    fn open_csv_edges_dialog(&mut self) {
        if self.panels.is_empty() {
            self.set_status("Najpierw wczytaj pliki STEP, aby przypisać obrzeża z CSV");
            return;
        }
        let mut dlg = rfd::FileDialog::new()
            .set_title("Wybierz plik .CSV obrzeży (F1..F4)")
            .add_filter("CSV (*.csv)", &["csv", "CSV"]);
        if let Some(dir) = existing_dir(&self.cfg.default_csv_dir) {
            dlg = dlg.set_directory(dir);
        }
        let Some(file) = dlg.pick_file() else {
            return;
        };

        let mut programs: Vec<PanelProgram> =
            self.panels.iter().map(|p| p.program.clone()).collect();
        match apply_csv_edges_from_file(&file, &mut programs) {
            Ok(matched) => {
                for (dst, prog) in self.panels.iter_mut().zip(programs.into_iter()) {
                    dst.program.edges = prog.edges;
                    dst.edge_manual = [false; 4];
                }
                self.set_status(format!(
                    "Obrzeża zaktualizowane z CSV (dopasowano {matched} z {})",
                    self.panels.len()
                ));
            }
            Err(err) => self.set_status(format!("Błąd CSV: {err}")),
        }
    }

    fn output_dir_for_panel(&self, panel: &LoadedStepPanel) -> PathBuf {
        if !self.cfg.default_save_dir.trim().is_empty() {
            return PathBuf::from(self.cfg.default_save_dir.trim());
        }
        let sub = if self.cfg.output_subfolder.trim().is_empty() {
            "SCX_NANXING"
        } else {
            self.cfg.output_subfolder.trim()
        };
        if let Some(src) = &panel.source_path {
            if let Some(parent) = src.parent() {
                return parent.join(sub);
            }
        }
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(sub)
    }

    fn preview_output_dir(&self) -> PathBuf {
        if let Some(first) = self.panels.first() {
            self.output_dir_for_panel(first)
        } else if !self.cfg.default_save_dir.trim().is_empty() {
            PathBuf::from(self.cfg.default_save_dir.trim())
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&self.cfg.output_subfolder)
        }
    }

    fn export_current_scx(&mut self) {
        let Some(panel) = self.panels.get(self.selected_idx) else {
            self.set_status("Brak wybranej formatki do zapisu");
            return;
        };
        if panel.manual_skip {
            self.set_status("Pominięto — formatka oznaczona jako pomijana");
            return;
        }
        let out_dir = self.output_dir_for_panel(panel);
        match save_program_scx(
            &panel.program,
            &panel.file_stem,
            &out_dir,
            self.cfg.skip_empty_panels,
        ) {
            Ok(Some(path)) => self.set_status(format!("Zapisano: {}", path.display())),
            Ok(None) => self.set_status("Pominięto — formatka pusta (0 operacji)"),
            Err(err) => self.set_status(format!("Błąd zapisu SCX: {err}")),
        }
    }

    fn export_all_scx(&mut self) {
        if self.panels.is_empty() {
            self.set_status("Brak wczytanych formatek STEP");
            return;
        }
        let mut saved = 0usize;
        let mut skipped = 0usize;
        let mut last_dir = PathBuf::new();

        for panel in &self.panels {
            let out_dir = self.output_dir_for_panel(panel);
            last_dir = out_dir.clone();
            if panel.manual_skip {
                skipped += 1;
                continue;
            }
            match save_program_scx(
                &panel.program,
                &panel.file_stem,
                &out_dir,
                self.cfg.skip_empty_panels,
            ) {
                Ok(Some(_)) => saved += 1,
                Ok(None) => skipped += 1,
                Err(err) => {
                    self.set_status(format!("Błąd zapisu {}: {err}", panel.file_stem));
                    return;
                }
            }
        }
        self.set_status(format!(
            "Zapisano {saved} plików SCX → {} (pominięte: {skipped})",
            last_dir.display()
        ));
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                self.active_modal = None;
                return;
            }
            if i.modifiers.ctrl {
                if i.key_pressed(Key::O) {
                    self.open_step_folder_dialog();
                } else if i.key_pressed(Key::L) {
                    self.open_csv_edges_dialog();
                } else if i.key_pressed(Key::S) && !self.panels.is_empty() {
                    self.active_modal = Some(ActiveModal::ExportAll);
                } else if i.key_pressed(Key::Comma) {
                    self.settings_dlg
                        .open_with(&self.cfg, SettingsCategory::Paths);
                }
            } else if !i.modifiers.alt {
                if i.key_pressed(Key::F5) {
                    self.redetect_all();
                    self.set_status("Przeliczono geometrię B-Rep dla wszystkich formatek");
                } else if i.key_pressed(Key::Num1) {
                    self.viewport.view_mode = ViewMode::XRay;
                } else if i.key_pressed(Key::Num2) {
                    self.viewport.view_mode = ViewMode::TopF5;
                } else if i.key_pressed(Key::Num3) {
                    self.viewport.view_mode = ViewMode::BottomF6;
                } else if i.key_pressed(Key::F) {
                    self.viewport.reset_camera();
                } else if i.key_pressed(Key::G) {
                    self.viewport.show_grid = !self.viewport.show_grid;
                } else if i.key_pressed(Key::ArrowDown) && !self.panels.is_empty() {
                    self.selected_idx = (self.selected_idx + 1).min(self.panels.len() - 1);
                    self.selection = None;
                    self.viewport.reset_camera();
                } else if i.key_pressed(Key::ArrowUp) && self.selected_idx > 0 {
                    self.selected_idx -= 1;
                    self.selection = None;
                    self.viewport.reset_camera();
                }
            }
        });
    }
}

impl eframe::App for NanxingStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);
        self.draw_menubar(ctx);
        self.draw_toolbar(ctx);
        self.draw_statusbar(ctx);
        self.draw_left_panel(ctx);
        self.draw_right_panel(ctx);
        self.draw_center_panel(ctx);
        self.draw_modals(ctx);

        match self.settings_dlg.draw(ctx, &mut self.cfg) {
            SettingsAction::ChangedLive => {
                self.redetect_all();
            }
            SettingsAction::SaveAndClose => {
                self.redetect_all();
                self.save_config();
                self.set_status(format!(
                    "Zapisano ustawienia w {}",
                    self.cfg_path.display()
                ));
            }
            SettingsAction::CancelAndRevert => {
                self.redetect_all();
            }
            SettingsAction::None => {}
        }
    }
}

impl NanxingStudioApp {
    fn draw_menubar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menubar")
            .exact_height(26.0_f32)
            .frame(
                Frame::NONE
                    .fill(BG_1)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(6, 2)),
            )
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("Plik", |ui| {
                        if ui.button("Otwórz folder STEP…   Ctrl+O").clicked() {
                            ui.close_menu();
                            self.open_step_folder_dialog();
                        }
                        if ui.button("Otwórz pojedyncze pliki STEP…").clicked() {
                            ui.close_menu();
                            self.open_step_files_dialog();
                        }
                        if ui.button("Wczytaj CSV obrzeży…  Ctrl+L").clicked() {
                            ui.close_menu();
                            self.open_csv_edges_dialog();
                        }
                        ui.separator();
                        if ui.button("Zapisz / Generuj wszystkie SCX   Ctrl+S").clicked() {
                            ui.close_menu();
                            if self.panels.is_empty() {
                                self.set_status("Brak wczytanych formatek STEP");
                            } else {
                                self.active_modal = Some(ActiveModal::ExportAll);
                            }
                        }
                        ui.separator();
                        if ui.button("Zamknij   Alt+F4").clicked() {
                            ui.close_menu();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    ui.menu_button("Widok", |ui| {
                        let m = self.viewport.view_mode;
                        if ui
                            .button(format!(
                                "{}X-Ray   1",
                                if m == ViewMode::XRay { "● " } else { "   " }
                            ))
                            .clicked()
                        {
                            self.viewport.view_mode = ViewMode::XRay;
                            ui.close_menu();
                        }
                        if ui
                            .button(format!(
                                "{}Góra    2",
                                if m == ViewMode::TopF5 { "● " } else { "   " }
                            ))
                            .clicked()
                        {
                            self.viewport.view_mode = ViewMode::TopF5;
                            ui.close_menu();
                        }
                        if ui
                            .button(format!(
                                "{}Dół     3",
                                if m == ViewMode::BottomF6 { "● " } else { "   " }
                            ))
                            .clicked()
                        {
                            self.viewport.view_mode = ViewMode::BottomF6;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Dopasuj widok   F").clicked() {
                            self.viewport.reset_camera();
                            ui.close_menu();
                        }
                        if ui
                            .button(format!(
                                "{}Siatka CAD   G",
                                if self.viewport.show_grid { "✓ " } else { "   " }
                            ))
                            .clicked()
                        {
                            self.viewport.show_grid = !self.viewport.show_grid;
                            ui.close_menu();
                        }
                    });

                    // Bezpośredni przycisk Ustawienia (zastępuje kaskadowe menu Maszyna)
                    if ui
                        .button("Ustawienia")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Otwórz okno ustawień z grupami (Ctrl+,)")
                        .clicked()
                    {
                        self.settings_dlg
                            .open_with(&self.cfg, SettingsCategory::Paths);
                    }

                    ui.menu_button("Narzędzia", |ui| {
                        if ui.button("Przelicz B-Rep   F5").clicked() {
                            ui.close_menu();
                            self.redetect_all();
                            self.set_status("Przeliczono wszystkie wczytane formatki STEP");
                        }
                    });

                    ui.menu_button("Pomoc", |ui| {
                        if ui.button("O programie").clicked() {
                            self.active_modal = Some(ActiveModal::About);
                            ui.close_menu();
                        }
                    });
                });
            });
    }

    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar")
            .exact_height(42.0_f32)
            .frame(
                Frame::NONE
                    .fill(BG_2)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    if toolbar_btn(ui, "Otwórz folder STEP", false).clicked() {
                        self.open_step_folder_dialog();
                    }
                    if toolbar_btn(ui, "Otwórz pliki STEP", false).clicked() {
                        self.open_step_files_dialog();
                    }
                    if toolbar_btn(ui, "Wczytaj CSV obrzeży", false).clicked() {
                        self.open_csv_edges_dialog();
                    }

                    ui.separator();

                    if toolbar_btn(ui, "Przelicz", false).clicked() {
                        self.redetect_all();
                        let n = self.panels.len();
                        let ops: usize = self
                            .panels
                            .iter()
                            .map(|p| p.program.operations.len())
                            .sum();
                        self.set_status(format!("Przeliczono {n} formatek · {ops} operacji"));
                    }

                    ui.separator();

                    if toolbar_btn(ui, "Zapisz / Generuj wszystkie SCX", true).clicked() {
                        if self.panels.is_empty() {
                            self.set_status("Najpierw wczytaj pliki STEP");
                        } else {
                            self.active_modal = Some(ActiveModal::ExportAll);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.active_folder_path)
                                    .monospace()
                                    .size(11.0_f32)
                                    .color(TX_3),
                            )
                            .truncate(),
                        );
                    });
                });
            });
    }

    fn draw_statusbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(24.0_f32)
            .frame(
                Frame::NONE
                    .fill(BG_STATUS)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(10, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(&self.status_msg)
                            .monospace()
                            .size(10.5_f32)
                            .color(if self.status_highlight { ACC_2 } else { TX }),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let total_ops: usize = self
                            .panels
                            .iter()
                            .map(|p| p.program.operations.len())
                            .sum();

                        ui.label(
                            RichText::new(format!(
                                "Lamello: {}/{} · góra {}, kraw. {} · operacji: {}",
                                format_num(self.cfg.lamello_surface_width),
                                format_num(self.cfg.lamello_edge_width),
                                format_num(self.cfg.lamello_surface_depth),
                                format_num(self.cfg.lamello_edge_depth),
                                total_ops
                            ))
                            .monospace()
                            .size(10.5_f32)
                            .color(TX_2),
                        );

                        ui.add_space(16.0_f32);

                        let xy_text = match self.viewport.cursor_mm {
                            Some((x, y)) => format!("X {:>7.1}   Y {:>7.1}", x, y),
                            None => "X —   Y —".to_string(),
                        };
                        ui.label(
                            RichText::new(xy_text)
                                .monospace()
                                .size(10.5_f32)
                                .color(TX_2),
                        );
                    });
                });
            });
    }

    fn draw_left_panel(&mut self, ctx: &egui::Context) {
        const LEFT_W: f32 = 288.0;
        egui::SidePanel::left("left_panel")
            .exact_width(LEFT_W)
            .min_width(LEFT_W)
            .max_width(LEFT_W)
            .resizable(false)
            .frame(Frame::NONE.fill(BG_1).stroke(Stroke::new(1.0_f32, LINE)))
            .show(ctx, |ui| {
                ui.set_width(LEFT_W);
                ui.set_max_width(LEFT_W);

                let q = self.search_query.trim().to_lowercase();
                let matching_indices: Vec<usize> = self
                    .panels
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        q.is_empty()
                            || p.program.name.to_lowercase().contains(&q)
                            || p.program.id.to_lowercase().contains(&q)
                    })
                    .map(|(i, _)| i)
                    .collect();
                let empty_cnt = self
                    .panels
                    .iter()
                    .filter(|p| p.program.operations.is_empty())
                    .count();

                panel_header(
                    ui,
                    "FORMATKI",
                    &format!(
                        "{}/{} · {} pusta",
                        matching_indices.len(),
                        self.panels.len(),
                        empty_cnt
                    ),
                );

                Frame::NONE
                    .fill(BG_1)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(LEFT_W - 20.0_f32);
                        ui.horizontal(|ui| {
                            let search_w = if self.search_query.is_empty() {
                                ui.available_width()
                            } else {
                                (ui.available_width() - 24.0_f32).max(80.0_f32)
                            };
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Szukaj formatki, ID…")
                                    .desired_width(search_w),
                            );
                            if !self.search_query.is_empty() && ui.small_button("✕").clicked() {
                                self.search_query.clear();
                            }
                        });
                    });

                let exportable = self.exportable_count();
                let list_h = (ui.available_height() - 28.0).max(80.0);
                ScrollArea::vertical()
                    .id_salt("left_panels_scroll")
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let row_outer_w = ui.available_width().min(LEFT_W);
                        ui.set_max_width(row_outer_w);
                        ui.spacing_mut().item_spacing = Vec2::ZERO;

                        if matching_indices.is_empty() {
                            ui.add_space(24.0_f32);
                            ui.vertical_centered(|ui| {
                                let txt = if self.panels.is_empty() {
                                    "Brak wczytanych plików STEP"
                                } else {
                                    "Brak wyników"
                                };
                                ui.label(RichText::new(txt).color(TX_3));
                            });
                        }
                        let mut toggle_skip_idx: Option<usize> = None;
                        for idx in matching_indices {
                            let loaded = &self.panels[idx];
                            let prog = &loaded.program;
                            let is_sel = idx == self.selected_idx;
                            let is_empty = prog.operations.is_empty();
                            let man_skip = loaded.manual_skip;
                            let xf_chip = format_transforms_chip(&loaded.user_transforms);

                            let row_bg = if is_sel { BG_SEL } else { BG_1 };
                            let inner_w = (row_outer_w - 24.0_f32).max(160.0_f32);
                            let mut skip_btn_rect = egui::Rect::NOTHING;

                            let resp = Frame::NONE
                                .fill(row_bg)
                                .inner_margin(Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.set_width(inner_w);
                                    ui.set_max_width(inner_w);
                                    ui.spacing_mut().item_spacing.y = 3.0_f32;

                                    ui.horizontal(|ui| {
                                        let name_w = (inner_w - 86.0_f32).max(80.0_f32);
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(name_w, 18.0_f32),
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                let mut title_rt = RichText::new(&prog.name)
                                                    .monospace()
                                                    .size(11.5_f32)
                                                    .color(if man_skip || is_empty {
                                                        TX_3
                                                    } else {
                                                        TX
                                                    });
                                                if man_skip {
                                                    title_rt = title_rt.strikethrough();
                                                }
                                                ui.add(egui::Label::new(title_rt).truncate());
                                            },
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0_f32;
                                                ui.label(
                                                    RichText::new(format!("{:02}", idx + 1))
                                                        .monospace()
                                                        .size(10.5_f32)
                                                        .color(TX_3),
                                                );

                                                // Przycisk ręcznego pomijania formatki
                                                let btn_txt =
                                                    if man_skip { "Pomijana" } else { "Pomiń" };
                                                let btn_w = if man_skip { 58.0_f32 } else { 44.0_f32 };
                                                let (b_rect, b_resp) = ui.allocate_exact_size(
                                                    Vec2::new(btn_w, 17.0_f32),
                                                    egui::Sense::click(),
                                                );
                                                skip_btn_rect = b_rect;
                                                let b_resp = b_resp
                                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                    .on_hover_text(if man_skip {
                                                        "Formatka pomijana — kliknij, aby włączyć ponownie"
                                                    } else {
                                                        "Kliknij, aby pominąć tę formatkę"
                                                    });
                                                let (bfill, bstroke, bfg) = if man_skip {
                                                    (
                                                        Color32::from_rgb(42, 33, 18),
                                                        WARN,
                                                        WARN,
                                                    )
                                                } else if b_resp.hovered() {
                                                    (
                                                        Color32::from_rgb(32, 36, 40),
                                                        WARN,
                                                        TX,
                                                    )
                                                } else {
                                                    (
                                                        Color32::from_rgb(24, 24, 24),
                                                        LINE_2,
                                                        TX_3,
                                                    )
                                                };
                                                ui.painter().rect_filled(b_rect, 0.0_f32, bfill);
                                                ui.painter().rect_stroke(
                                                    b_rect,
                                                    0.0_f32,
                                                    Stroke::new(1.0_f32, bstroke),
                                                    egui::StrokeKind::Inside,
                                                );
                                                ui.painter().text(
                                                    b_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    btn_txt,
                                                    egui::FontId::proportional(10.0_f32),
                                                    bfg,
                                                );
                                                if b_resp.clicked() {
                                                    toggle_skip_idx = Some(idx);
                                                }
                                            },
                                        );
                                    });

                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!(
                                                "{} × {} × {} mm  ·  ID: {}",
                                                format_num(prog.length),
                                                format_num(prog.width),
                                                format_num(prog.thickness),
                                                prog.id
                                            ))
                                            .monospace()
                                            .size(10.5_f32)
                                            .color(TX_2),
                                        )
                                        .truncate(),
                                    );

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("■")
                                                .size(7.0_f32)
                                                .color(if man_skip { WARN } else { OK }),
                                        );
                                        let orient = if man_skip {
                                            "Formatka pomijana"
                                        } else if prog.flipped_top_bottom {
                                            "Strona z większą liczbą obróbek u góry (Face 5)"
                                        } else {
                                            "Orientacja bazowa"
                                        };
                                        ui.label(RichText::new(orient).size(10.5_f32).color(TX_3));
                                    });

                                    for w in &prog.warnings {
                                        ui.horizontal_top(|ui| {
                                            ui.label(RichText::new("■").size(7.0_f32).color(WARN));
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(
                                                    (inner_w - 16.0_f32).max(80.0_f32),
                                                    14.0_f32,
                                                ),
                                                egui::Layout::top_down(egui::Align::LEFT),
                                                |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(w)
                                                                .size(10.0_f32)
                                                                .color(WARN),
                                                        )
                                                        .wrap(),
                                                    );
                                                },
                                            );
                                        });
                                    }

                                    ui.add_space(2.0_f32);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 3.0);
                                        if man_skip {
                                            draw_flat_tag(ui, "Pomijana", WARN);
                                        }
                                        if is_empty {
                                            let lbl = if self.cfg.skip_empty_panels {
                                                "Pusta · pomijana"
                                            } else {
                                                "Pusta"
                                            };
                                            draw_flat_tag(ui, lbl, TX_3);
                                        } else {
                                            let drills = prog
                                                .operations
                                                .iter()
                                                .filter(|o| {
                                                    matches!(
                                                        o,
                                                        Operation::VerticalDrill { .. }
                                                            | Operation::HorizontalDrill { .. }
                                                    )
                                                })
                                                .count();
                                            let lams = prog
                                                .operations
                                                .iter()
                                                .filter(|o| {
                                                    matches!(o, Operation::LamelloP14 { .. })
                                                })
                                                .count();
                                            let grooves = prog
                                                .operations
                                                .iter()
                                                .filter(|o| matches!(o, Operation::Groove { .. }))
                                                .count();
                                            if drills > 0 {
                                                draw_flat_tag(
                                                    ui,
                                                    &format!("Wiercenia {drills}"),
                                                    COL_VDRILL,
                                                );
                                            }
                                            if lams > 0 {
                                                draw_flat_tag(
                                                    ui,
                                                    &format!("Lamello P14 ×{lams}"),
                                                    COL_LAMELLO,
                                                );
                                            }
                                            if grooves > 0 {
                                                draw_flat_tag(
                                                    ui,
                                                    &format!("Nuta ×{grooves}"),
                                                    COL_GROOVE,
                                                );
                                            }
                                        }
                                        let ec = prog.edges.active_count();
                                        if ec > 0 {
                                            draw_flat_tag(
                                                ui,
                                                &format!("Obrzeża CSV {ec}/4"),
                                                COL_EDGE,
                                            );
                                        }
                                        // Chip informujący o ręcznej zmianie orientacji
                                        if let Some(ref xf_txt) = xf_chip {
                                            draw_flat_tag(
                                                ui,
                                                &format!("Orient.: {xf_txt}"),
                                                ACC_2,
                                            );
                                        }
                                    });
                                })
                                .response;

                            let r = resp.rect;
                            if is_sel {
                                ui.painter().line_segment(
                                    [r.left_top(), r.left_bottom()],
                                    Stroke::new(2.0_f32, ACC_2),
                                );
                            }
                            ui.painter().line_segment(
                                [r.left_bottom(), r.right_bottom()],
                                Stroke::new(1.0_f32, LINE),
                            );

                            let row_click = resp.interact(egui::Sense::click());
                            if row_click.clicked() {
                                let ptr_on_skip = row_click
                                    .interact_pointer_pos()
                                    .is_some_and(|p| skip_btn_rect.expand(2.0).contains(p));
                                if !ptr_on_skip && toggle_skip_idx != Some(idx) {
                                    self.selected_idx = idx;
                                    self.selection = None;
                                    self.viewport.reset_camera();
                                }
                            }
                        }

                        if let Some(s_idx) = toggle_skip_idx {
                            if let Some(p) = self.panels.get_mut(s_idx) {
                                p.manual_skip = !p.manual_skip;
                            }
                        }
                });

                Frame::NONE
                    .fill(BG_2)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(format!(
                                "Do eksportu: {exportable} / {}",
                                self.panels.len()
                            ))
                            .monospace()
                            .size(10.5_f32)
                            .color(TX_3),
                        );
                    });
            });
    }

    fn draw_right_panel(&mut self, ctx: &egui::Context) {
        self.hover_edge_from_inspector = None;

        const RIGHT_W: f32 = 400.0;
        egui::SidePanel::right("right_panel")
            .exact_width(RIGHT_W)
            .min_width(RIGHT_W)
            .max_width(RIGHT_W)
            .resizable(false)
            .frame(Frame::NONE.fill(BG_1).stroke(Stroke::new(1.0_f32, LINE)))
            .show(ctx, |ui| {
                ui.set_width(RIGHT_W);
                ui.set_max_width(RIGHT_W);
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // ── KARTA 1: GABARYTY FORMATKI ──
                if let Some(loaded) = self.panels.get(self.selected_idx).cloned() {
                    let prog = &loaded.program;

                    panel_header(ui, "GABARYTY FORMATKI", &format!("{}.step", loaded.file_stem));
                    Frame::NONE
                        .fill(BG_1)
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.columns(3, |cols| {
                                draw_dim_cell(&mut cols[0], "LENGTH", prog.length);
                                draw_dim_cell(&mut cols[1], "WIDTH", prog.width);
                                draw_dim_cell(&mut cols[2], "THICKNESS", prog.thickness);
                            });
                            ui.add_space(6.0_f32);
                            ui.label(
                                RichText::new(format!(
                                    "ID (skaner): {}   ·   Info: {}",
                                    prog.id, prog.name
                                ))
                                .monospace()
                                .size(10.5_f32)
                                .color(TX_2),
                            );
                            let flip_s = if prog.flipped_top_bottom {
                                " · auto-obrót 180° Y (Face 5 u góry)"
                            } else {
                                ""
                            };
                            ui.label(
                                RichText::new(format!(
                                    "Obrys <Outline>: {} pkt{}",
                                    prog.outline.len(),
                                    flip_s
                                ))
                                .monospace()
                                .size(10.5_f32)
                                .color(TX_3),
                            );
                        });
                    draw_h_line(ui);

                    // ── KARTA 2: OBRZEŻA (<EdgeGroup>: 1=Dół, 2=Góra, 3=Lewo, 4=Prawo) ──
                    panel_header(ui, "OBRZEŻA", "F1..F4");
                    Frame::NONE
                        .fill(BG_1)
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            let faces = [
                                (1u8, "F1", "Dół", 0usize),
                                (2u8, "F2", "Góra", 1usize),
                                (3u8, "F3", "Lewo", 2usize),
                                (4u8, "F4", "Prawo", 3usize),
                            ];
                            let shift_held = ui.input(|i| i.modifiers.shift);
                            ui.columns(4, |cols| {
                                for (i, (face, code_f, side_s, m_idx)) in
                                    faces.into_iter().enumerate()
                                {
                                    let col_ui = &mut cols[i];
                                    let th = self.panels[self.selected_idx].program.edges.get(face);
                                    let edge_code =
                                        self.panels[self.selected_idx].program.edges.code_label(face);
                                    let is_manual =
                                        self.panels[self.selected_idx].edge_manual[m_idx];
                                    let is_sel = self.selection == Some(Selection::Edge(face));

                                    let fill = if is_sel { BG_SEL } else { BG_0 };
                                    let stroke = if is_sel { ACC_2 } else { LINE_2 };
                                    let resp = Frame::NONE
                                        .fill(fill)
                                        .stroke(Stroke::new(1.0_f32, stroke))
                                        .inner_margin(Margin::symmetric(6, 5))
                                        .show(col_ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(code_f)
                                                        .monospace()
                                                        .strong()
                                                        .size(11.0_f32)
                                                        .color(ACC_2),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(side_s)
                                                                .size(9.5_f32)
                                                                .color(TX_3),
                                                        );
                                                    },
                                                );
                                            });
                                            ui.add_space(3.0_f32);
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(edge_code)
                                                        .monospace()
                                                        .size(11.0_f32)
                                                        .color(if th > 0.01 { TX } else { TX_3 }),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new("■")
                                                                .size(6.5_f32)
                                                                .color(if is_manual {
                                                                    WARN
                                                                } else {
                                                                    ACC
                                                                }),
                                                        );
                                                    },
                                                );
                                            });
                                        })
                                        .response;

                                    let interact = resp.interact(egui::Sense::click());
                                    if interact.hovered() {
                                        self.hover_edge_from_inspector = Some(face);
                                    }
                                    if interact.clicked() {
                                        self.panels[self.selected_idx]
                                            .program
                                            .edges
                                            .cycle_face_dir(face, !shift_held);
                                        self.panels[self.selected_idx].edge_manual[m_idx] = true;
                                        self.selection = Some(Selection::Edge(face));
                                    }
                                }
                            });
                            ui.add_space(5.0_f32);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Klik — następny kod · Shift+klik — poprzedni. ")
                                        .size(10.0_f32)
                                        .color(TX_3),
                                );
                                ui.label(RichText::new("■").size(8.0_f32).color(WARN));
                                ui.label(
                                    RichText::new(" ręczna korekta").size(10.0_f32).color(TX_3),
                                );
                            });
                        });
                    draw_h_line(ui);

                    // ── KARTA 3 (ROSNĄCA): Tabela operacji | Kod programu (.scx) ──
                    // (Reguły maszyny przeniesiono do okna Ustawienia → Reguły i Wiercenie)
                    let active_edges_cnt = prog.edges.active_count();
                    let rows_total = prog.operations.len() + active_edges_cnt;

                    Frame::NONE
                        .fill(BG_2)
                        .stroke(Stroke::new(1.0_f32, LINE))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                if tab_button(
                                    ui,
                                    &format!("Tabela operacji  [{rows_total}]"),
                                    self.bottom_tab == BottomTab::Operations,
                                )
                                .clicked()
                                {
                                    self.bottom_tab = BottomTab::Operations;
                                }
                                if tab_button(
                                    ui,
                                    "Kod programu (.scx)",
                                    self.bottom_tab == BottomTab::ProgramCode,
                                )
                                .clicked()
                                {
                                    self.bottom_tab = BottomTab::ProgramCode;
                                }
                            });
                        });

                    let body_h = (ui.available_height() - 26.0).max(160.0);
                    match self.bottom_tab {
                        BottomTab::Operations => {
                            // Stałe szerokości kolumn tabeli (suma 376 px + margines paska przewijania)
                            const W_NO: f32 = 42.0;
                            const W_OP: f32 = 112.0;
                            const W_XY: f32 = 68.0;
                            const W_PM: f32 = 152.0;

                            // Przypięty nagłówek kolumn
                            Frame::NONE
                                .fill(Color32::from_rgb(23, 23, 23))
                                .stroke(Stroke::new(1.0_f32, LINE))
                                .inner_margin(Margin::symmetric(8, 5))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0_f32;
                                        col_header(ui, "#", W_NO);
                                        col_header(ui, "OPERACJA", W_OP);
                                        col_header(ui, "POZ. X / Y", W_XY);
                                        col_header(ui, "PARAMETRY", W_PM);
                                    });
                                });

                            ScrollArea::vertical()
                                .id_salt("ops_table_scroll")
                                .max_height((body_h - 24.0).max(120.0))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    if rows_total == 0 {
                                        ui.add_space(28.0_f32);
                                        ui.vertical_centered(|ui| {
                                            let msg = if self.cfg.skip_empty_panels {
                                                "Formatka bez obróbek.\nZostanie pominięta przy eksporcie."
                                            } else {
                                                "Formatka bez obróbek.\nZostanie wyeksportowana jako pusty program."
                                            };
                                            ui.label(RichText::new(msg).color(TX_3));
                                        });
                                    } else {
                                        for (idx, op) in prog.operations.iter().enumerate() {
                                            let kind = LayerKind::of_op(op);
                                            let is_sel = self.selection == Some(Selection::Op(idx));
                                            let (face_u8, op_col, x_line, y_line, p1, p2) = match op {
                                                Operation::VerticalDrill {
                                                    face,
                                                    x,
                                                    y,
                                                    diameter,
                                                    depth,
                                                    is_through,
                                                } => (
                                                    *face,
                                                    if *is_through {
                                                        COL_VDRILL_THRU
                                                    } else {
                                                        kind.color()
                                                    },
                                                    format!("{:.1}", x),
                                                    format!("{:.1}", y),
                                                    format!(
                                                        "Ø{} · gł. {:.1}",
                                                        format_num(*diameter),
                                                        depth
                                                    ),
                                                    if *is_through {
                                                        "przelot (góra+dół)".to_string()
                                                    } else if *face == 6 {
                                                        "od dołu (pierścień)".to_string()
                                                    } else {
                                                        String::new()
                                                    },
                                                ),
                                                Operation::HorizontalDrill {
                                                    face,
                                                    x,
                                                    y,
                                                    z,
                                                    diameter,
                                                    depth,
                                                } => (
                                                    *face,
                                                    kind.color(),
                                                    format!("{:.1}", x),
                                                    format!("{:.1}", y),
                                                    format!(
                                                        "Ø{} · gł. {:.1}",
                                                        format_num(*diameter),
                                                        depth
                                                    ),
                                                    format!("Z {:.1}", z),
                                                ),
                                                Operation::LamelloP14 {
                                                    face,
                                                    direction,
                                                    x,
                                                    y,
                                                    z,
                                                    width,
                                                    max_depth,
                                                    ..
                                                } => (
                                                    *face,
                                                    if *face >= 5 {
                                                        COL_LAMELLO_FACE
                                                    } else {
                                                        COL_LAMELLO
                                                    },
                                                    format!("{:.1}", x),
                                                    format!("{:.1}", y),
                                                    format!(
                                                        "dir {} · W{}",
                                                        direction,
                                                        format_num(*width)
                                                    ),
                                                    format!(
                                                        "gł. {} · Z {:.1}",
                                                        format_num(*max_depth),
                                                        z
                                                    ),
                                                ),
                                                Operation::Groove {
                                                    face,
                                                    x,
                                                    y,
                                                    width,
                                                    depth,
                                                    ..
                                                } => (
                                                    *face,
                                                    kind.color(),
                                                    format!("{:.1}", x),
                                                    format!("{:.1}", y),
                                                    format!(
                                                        "szer. {} · gł. {:.1}",
                                                        format_num(*width),
                                                        depth
                                                    ),
                                                    String::new(),
                                                ),
                                            };

                                            if draw_ops_row(
                                                ui,
                                                is_sel,
                                                &format!("N{:03}", (idx + 1) * 10),
                                                op_col,
                                                kind.short_name(),
                                                face_short_label(face_u8),
                                                &x_line,
                                                &y_line,
                                                &p1,
                                                &p2,
                                                (W_NO, W_OP, W_XY, W_PM),
                                            )
                                            .clicked()
                                            {
                                                self.selection = Some(Selection::Op(idx));
                                            }
                                        }

                                        for (face, m_idx) in
                                            [(1u8, 0usize), (2, 1), (3, 2), (4, 3)]
                                        {
                                            let th = prog.edges.get(face);
                                            if th <= 0.01 {
                                                continue;
                                            }
                                            let is_sel =
                                                self.selection == Some(Selection::Edge(face));
                                            let manual = loaded.edge_manual[m_idx];
                                            if draw_ops_row(
                                                ui,
                                                is_sel,
                                                "OBRZ",
                                                COL_EDGE,
                                                "OBRZEŻE",
                                                edge_face_label(face),
                                                "—",
                                                "",
                                                prog.edges.code_label(face),
                                                if manual { "ręcznie" } else { "CSV" },
                                                (W_NO, W_OP, W_XY, W_PM),
                                            )
                                            .clicked()
                                            {
                                                self.selection = Some(Selection::Edge(face));
                                            }
                                        }
                                    }
                                });
                        }
                        BottomTab::ProgramCode => {
                            let xml = render_scx_xml(prog);
                            let lines_cnt = xml.lines().count();
                            Frame::NONE
                                .fill(Color32::from_rgb(23, 23, 23))
                                .stroke(Stroke::new(1.0_f32, LINE))
                                .inner_margin(Margin::symmetric(10, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{}.scx · {lines_cnt} linii",
                                                loaded.file_stem
                                            ))
                                            .monospace()
                                            .size(10.5_f32)
                                            .color(TX_2),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("Zapisz .scx").clicked() {
                                                    self.export_current_scx();
                                                }
                                                if ui.small_button("Kopiuj").clicked() {
                                                    ui.ctx().copy_text(xml.clone());
                                                    self.set_status("Skopiowano kod SCX do schowka");
                                                }
                                            },
                                        );
                                    });
                                });

                            ScrollArea::both()
                                .id_salt("code_scroll")
                                .max_height((body_h - 30.0).max(80.0))
                                .show(ui, |ui| {
                                    Frame::NONE.inner_margin(Margin::symmetric(8, 6)).show(
                                        ui,
                                        |ui| {
                                            for (i, line) in xml.lines().enumerate() {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(format!("{:>3} ", i + 1))
                                                            .monospace()
                                                            .size(10.5_f32)
                                                            .color(Color32::from_rgb(68, 68, 68)),
                                                    );
                                                    let lcol = if line.contains("<Machining") {
                                                        ACC_2
                                                    } else if line.contains("<Panel")
                                                        || line.contains("<Root")
                                                    {
                                                        WARN
                                                    } else {
                                                        TX_2
                                                    };
                                                    ui.label(
                                                        RichText::new(line)
                                                            .monospace()
                                                            .size(10.5_f32)
                                                            .color(lcol),
                                                    );
                                                });
                                            }
                                        },
                                    );
                                });
                        }
                    }

                    Frame::NONE
                        .fill(BG_2)
                        .stroke(Stroke::new(1.0_f32, LINE))
                        .inner_margin(Margin::symmetric(12, 5))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(format!(
                                    "{} operacji · {} obrzeża",
                                    prog.operations.len(),
                                    active_edges_cnt
                                ))
                                .monospace()
                                .size(10.5_f32)
                                .color(TX_3),
                            );
                        });
                } else {
                    panel_header(ui, "INSPEKTOR FORMATKI", "brak danych");
                    Frame::NONE.inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.label(
                            RichText::new(
                                "Wczytaj plik lub folder STEP, aby zobaczyć wymiary, wykryte operacje oraz podgląd kodu .scx.",
                            )
                            .color(TX_3),
                        );
                    });
                }
            });
    }

    fn draw_center_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG_0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // Dolna interaktywna legenda (#legend)
                egui::TopBottomPanel::bottom("viewport_legend")
                    .frame(
                        Frame::NONE
                            .fill(BG_1)
                            .stroke(Stroke::new(1.0_f32, LINE))
                            .inner_margin(Margin::symmetric(12, 7)),
                    )
                    .show_inside(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(14.0, 4.0);
                            ui.label(RichText::new("LEGENDA").small().color(TX_3));
                            for kind in LayerKind::ALL {
                                let visible = self.viewport.is_layer_visible(kind);
                                let resp = ui
                                    .horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0_f32;
                                        let swatch_col = if visible {
                                            kind.color()
                                        } else {
                                            Color32::from_rgba_unmultiplied(
                                                kind.color().r(),
                                                kind.color().g(),
                                                kind.color().b(),
                                                80,
                                            )
                                        };
                                        ui.label(
                                            RichText::new("■").size(11.0_f32).color(swatch_col),
                                        );
                                        let mut lbl = RichText::new(kind.full_name())
                                            .size(11.5_f32)
                                            .color(if visible { TX_2 } else { TX_3 });
                                        if !visible {
                                            lbl = lbl.strikethrough();
                                        }
                                        ui.add(egui::Button::new(lbl).frame(false))
                                    })
                                    .inner;
                                if resp
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text("Klik: pokaż / ukryj warstwę")
                                    .clicked()
                                {
                                    self.viewport.toggle_layer(kind);
                                }
                            }

                            ui.separator();
                            // Podgląd symboli otworów pionowych (góra / dół / przelot)
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0_f32;
                                ui.label(RichText::new("●").size(11.0_f32).color(COL_VDRILL));
                                ui.label(RichText::new("Góra").size(10.5_f32).color(TX_3));
                                ui.add_space(4.0_f32);
                                ui.label(RichText::new("◯").size(11.0_f32).color(COL_VDRILL));
                                ui.label(RichText::new("Dół").size(10.5_f32).color(TX_3));
                                ui.add_space(4.0_f32);
                                ui.label(RichText::new("◉").size(11.5_f32).color(COL_VDRILL_THRU));
                                ui.label(RichText::new("Przelot").size(10.5_f32).color(TX_3));
                            });
                        });
                    });

                // Płótno CAD albo delikatny napis na pustym tle (gdy nie wczytano jeszcze STEP)
                let vp_action = if let Some(loaded) = self.panels.get(self.selected_idx) {
                    draw_cad_viewport(
                        ui,
                        &loaded.program,
                        &self.cfg,
                        &mut self.viewport,
                        &mut self.selection,
                        self.hover_edge_from_inspector,
                        !loaded.user_transforms.is_empty(),
                        loaded.manual_skip,
                    )
                } else {
                    Frame::NONE.fill(BG_CANVAS).show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Załaduj plik lub folder STEP, aby wyświetlić podgląd formatki")
                                    .size(12.5_f32)
                                    .color(TX_3),
                            );
                        });
                    });
                    ViewportAction::None
                };

                match vp_action {
                    ViewportAction::Transform(xf) => {
                        let opts = self.cfg.to_convert_options();
                        if let Some(panel) = self.panels.get_mut(self.selected_idx) {
                            panel.apply_transform(xf, &opts);
                            self.selection = None;
                            let name = panel.program.name.clone();
                            self.set_status(format!(
                                "Zmieniono orientację ({}) dla: {}",
                                xf.short_chip_label(),
                                name
                            ));
                        }
                    }
                    ViewportAction::ResetTransforms => {
                        let opts = self.cfg.to_convert_options();
                        if let Some(panel) = self.panels.get_mut(self.selected_idx) {
                            panel.reset_transforms(&opts);
                            self.selection = None;
                            let name = panel.program.name.clone();
                            self.set_status(format!(
                                "Przywrócono orientację domyślną dla: {}",
                                name
                            ));
                        }
                    }
                    ViewportAction::None => {}
                }
            });
    }

    fn draw_modals(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.active_modal else {
            return;
        };

        match modal {
            ActiveModal::ExportAll => {
                let mut open = true;
                let mut close_clicked = false;
                let mut do_export = false;
                let out_dir = self.preview_output_dir();
                let exp_cnt = self.exportable_count();

                egui::Window::new("Generuj wszystkie SCX")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .fixed_size([620.0, 380.0])
                    .open(&mut open)
                    .show(ctx, |ui| {
                        Frame::NONE.inner_margin(Margin::same(12)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Katalog wyjściowy:").color(TX_3));
                                ui.label(
                                    RichText::new(out_dir.display().to_string())
                                        .monospace()
                                        .color(TX),
                                );
                            });
                            ui.add_space(10.0_f32);

                            ScrollArea::vertical().max_height(240.0_f32).show(ui, |ui| {
                                egui::Grid::new("export_summary_grid")
                                    .num_columns(4)
                                    .spacing([14.0, 6.0])
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("PLIK").small().color(TX_3));
                                        ui.label(RichText::new("OP.").small().color(TX_3));
                                        ui.label(RichText::new("LINII").small().color(TX_3));
                                        ui.label(RichText::new("STATUS").small().color(TX_3));
                                        ui.end_row();

                                        for loaded in &self.panels {
                                            let prog = &loaded.program;
                                            let skip = self.is_panel_skipped(loaded);
                                            let lines = if skip {
                                                0
                                            } else {
                                                render_scx_xml(prog).lines().count()
                                            };

                                            ui.label(
                                                RichText::new(format!("{}.scx", loaded.file_stem))
                                                    .monospace()
                                                    .size(11.0_f32),
                                            );
                                            ui.label(
                                                RichText::new(if skip {
                                                    "—".to_string()
                                                } else {
                                                    prog.operations.len().to_string()
                                                })
                                                .monospace(),
                                            );
                                            ui.label(
                                                RichText::new(if skip {
                                                    "—".to_string()
                                                } else {
                                                    lines.to_string()
                                                })
                                                .monospace(),
                                            );

                                            if loaded.manual_skip {
                                                ui.label(RichText::new("pominięta").color(WARN));
                                            } else if skip {
                                                ui.label(
                                                    RichText::new("pominięta (pusta)").color(TX_3),
                                                );
                                            } else if !prog.warnings.is_empty() {
                                                ui.label(
                                                    RichText::new(format!("▲ {}", prog.warnings[0]))
                                                        .color(WARN),
                                                );
                                            } else {
                                                ui.label(RichText::new("OK").color(OK));
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });

                            ui.add_space(10.0_f32);
                            ui.separator();
                            ui.add_space(6.0_f32);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "Do zapisu: {exp_cnt} z {} plików.",
                                        self.panels.len()
                                    ))
                                    .color(TX_3),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if toolbar_btn(
                                            ui,
                                            &format!("Generuj {exp_cnt} plików"),
                                            true,
                                        )
                                        .clicked()
                                        {
                                            do_export = true;
                                        }
                                        if ui.button("Anuluj").clicked() {
                                            close_clicked = true;
                                        }
                                    },
                                );
                            });
                        });
                    });

                if do_export {
                    self.export_all_scx();
                    self.active_modal = None;
                } else if !open || close_clicked {
                    self.active_modal = None;
                }
            }
            ActiveModal::About => {
                let mut open = true;
                let mut close_clicked = false;
                egui::Window::new("O programie")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .fixed_size([440.0, 175.0])
                    .open(&mut open)
                    .show(ctx, |ui| {
                        Frame::NONE.inner_margin(Margin::same(14)).show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Nanxing Converter v{}",
                                    env!("CARGO_PKG_VERSION")
                                ))
                                .strong()
                                .color(TX),
                            );
                            ui.add_space(6.0_f32);
                            ui.label(
                                RichText::new(
                                    "Natywna aplikacja Windows w Rust (egui + bezpośredni silnik B-Rep ISO-10303-21) dla 6-stronnej wiertarki CNC Nanxing.",
                                )
                                .color(TX_2),
                            );
                            ui.add_space(6.0_f32);
                            ui.label(
                                RichText::new(format!("Plik konfiguracji: {}", self.cfg_path.display()))
                                    .monospace()
                                    .size(10.5_f32)
                                    .color(TX_3),
                            );
                            ui.add_space(12.0_f32);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if toolbar_btn(ui, "Zamknij", true).clicked() {
                                        close_clicked = true;
                                    }
                                },
                            );
                        });
                    });
                if !open || close_clicked {
                    self.active_modal = None;
                }
            }
        }
    }
}

fn resolve_config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("nanxing_config.ini");
        }
    }
    PathBuf::from("nanxing_config.ini")
}

fn existing_dir(s: &str) -> Option<PathBuf> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let p = PathBuf::from(trimmed);
    if p.is_dir() { Some(p) } else { None }
}

fn is_step_file(p: &Path) -> bool {
    p.is_file()
        && p.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "step" | "stp"))
            .unwrap_or(false)
}

fn panel_header(ui: &mut egui::Ui, title: &str, sub: &str) {
    let max_w = ui.available_width();
    Frame::NONE
        .fill(BG_2)
        .stroke(Stroke::new(1.0_f32, LINE))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            let inner_w = (max_w - 24.0_f32).max(80.0_f32);
            ui.set_width(inner_w);
            ui.set_max_width(inner_w);
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(10.5_f32).strong().color(TX_2));
                if !sub.is_empty() {
                    let sub_max = (ui.available_width() - 4.0_f32).max(40.0_f32);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_max_width(sub_max);
                        ui.add(
                            egui::Label::new(
                                RichText::new(sub).monospace().size(10.5_f32).color(TX_3),
                            )
                            .truncate(),
                        );
                    });
                }
            });
        });
}

fn draw_h_line(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter()
        .line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0_f32, LINE));
}

fn toolbar_btn(ui: &mut egui::Ui, label: &str, primary: bool) -> egui::Response {
    let font = egui::FontId::proportional(11.5_f32);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), TX);
    let btn_w = galley.size().x + 22.0_f32;
    let btn_h = 28.0_f32;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(btn_w, btn_h), egui::Sense::click());
    let down = resp.is_pointer_button_down_on();
    let hov = resp.hovered();

    let (fill, border, fg) = if primary {
        if down {
            (Color32::from_rgb(8, 38, 48), ACC_2, Color32::WHITE)
        } else if hov {
            (Color32::from_rgb(18, 78, 98), ACC_2, Color32::WHITE)
        } else {
            (BG_PRI, PRI_BORDER, PRI_TX)
        }
    } else if down {
        (Color32::from_rgb(14, 52, 66), ACC_2, PRI_TX)
    } else if hov {
        (Color32::from_rgb(36, 42, 46), ACC, Color32::WHITE)
    } else {
        (Color32::from_rgb(26, 26, 26), LINE_2, TX)
    };

    ui.painter().rect_filled(rect, 0.0_f32, fill);
    ui.painter().rect_stroke(
        rect,
        0.0_f32,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    let text_center = if down {
        rect.center() + Vec2::new(0.0_f32, 1.0_f32)
    } else {
        rect.center()
    };
    ui.painter().text(
        text_center,
        egui::Align2::CENTER_CENTER,
        label,
        font,
        fg,
    );

    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn tab_button(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let resp = ui.add_sized(
        [0.0, 30.0],
        egui::Button::new(
            RichText::new(label)
                .size(11.5_f32)
                .color(if active { TX } else { TX_2 }),
        )
        .fill(if active { BG_1 } else { BG_2 })
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(CornerRadius::ZERO),
    );
    if active {
        let r = resp.rect;
        ui.painter().line_segment(
            [r.left_bottom(), r.right_bottom()],
            Stroke::new(2.0_f32, ACC_2),
        );
    }
    resp
}

fn draw_dim_cell(ui: &mut egui::Ui, label: &str, val: f64) {
    Frame::NONE
        .fill(BG_0)
        .stroke(Stroke::new(1.0_f32, LINE))
        .inner_margin(Margin::symmetric(9, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(label).size(9.5_f32).color(TX_3));
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format_num(val))
                        .monospace()
                        .size(15.5_f32)
                        .color(TX),
                );
                ui.label(RichText::new("mm").size(10.0_f32).color(TX_3));
            });
        });
}

fn draw_flat_tag(ui: &mut egui::Ui, text: &str, accent: Color32) {
    let resp = Frame::NONE
        .fill(Color32::from_rgb(21, 21, 21))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(42, 42, 42)))
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.0_f32).color(TX_2));
        })
        .response;
    let r = resp.rect;
    ui.painter()
        .line_segment([r.left_top(), r.left_bottom()], Stroke::new(2.0_f32, accent));
}

fn col_header(ui: &mut egui::Ui, title: &str, width: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 14.0_f32),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(RichText::new(title).size(9.5_f32).strong().color(TX_3));
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_ops_row(
    ui: &mut egui::Ui,
    selected: bool,
    no: &str,
    col: Color32,
    op_name: &str,
    face_sub: &str,
    x_line: &str,
    y_line: &str,
    param_line1: &str,
    param_line2: &str,
    widths: (f32, f32, f32, f32),
) -> egui::Response {
    let (w_no, w_op, w_xy, w_pm) = widths;
    let bg = if selected { BG_SEL } else { BG_1 };
    let resp = Frame::NONE
        .fill(bg)
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0_f32;

                // Kolumna 1: #
                ui.allocate_ui_with_layout(
                    Vec2::new(w_no, 28.0_f32),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.label(RichText::new(no).monospace().size(10.5_f32).color(TX_3));
                    },
                );

                // Kolumna 2: Operacja + ściana
                ui.allocate_ui_with_layout(
                    Vec2::new(w_op, 28.0_f32),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0_f32;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0_f32;
                            ui.label(RichText::new("■").size(8.5_f32).color(col));
                            ui.label(RichText::new(op_name).monospace().size(10.5_f32).color(col));
                        });
                        ui.label(RichText::new(face_sub).size(10.0_f32).color(TX_3));
                    },
                );

                // Kolumna 3: Poz. X / Y
                ui.allocate_ui_with_layout(
                    Vec2::new(w_xy, 28.0_f32),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0_f32;
                        ui.label(RichText::new(x_line).monospace().size(10.5_f32).color(TX_2));
                        if !y_line.is_empty() {
                            ui.label(RichText::new(y_line).monospace().size(10.5_f32).color(TX_2));
                        }
                    },
                );

                // Kolumna 4: Parametry
                ui.allocate_ui_with_layout(
                    Vec2::new(w_pm, 28.0_f32),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0_f32;
                        ui.label(
                            RichText::new(param_line1)
                                .monospace()
                                .size(10.5_f32)
                                .color(TX_2),
                        );
                        if !param_line2.is_empty() {
                            ui.label(
                                RichText::new(param_line2)
                                    .monospace()
                                    .size(10.0_f32)
                                    .color(TX_3),
                            );
                        }
                    },
                );
            });
        })
        .response;

    let r = resp.rect;
    if selected {
        ui.painter()
            .line_segment([r.left_top(), r.left_bottom()], Stroke::new(2.0_f32, ACC_2));
    }
    ui.painter().line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(1.0_f32, Color32::from_rgb(26, 26, 26)),
    );
    resp.interact(egui::Sense::click())
}
