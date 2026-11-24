use crate::theme::*;
use egui::{CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke, Vec2};
use nanxing_core::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Paths,
    Lamello,
    Drilling,
    MachineClamps,
    Viewport,
}

impl SettingsCategory {
    pub const ALL: [SettingsCategory; 5] = [
        SettingsCategory::Paths,
        SettingsCategory::Lamello,
        SettingsCategory::Drilling,
        SettingsCategory::MachineClamps,
        SettingsCategory::Viewport,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Paths => "Ścieżki",
            Self::Lamello => "Lamello P14",
            Self::Drilling => "Wiercenie",
            Self::MachineClamps => "Maszyna i Mocowanie",
            Self::Viewport => "Podgląd",
        }
    }
}

pub enum SettingsAction {
    None,
    ChangedLive,
    SaveAndClose,
    CancelAndRevert,
}

pub struct SettingsDialogState {
    pub open: bool,
    pub category: SettingsCategory,
    pub snapshot: AppConfig,
}

impl Default for SettingsDialogState {
    fn default() -> Self {
        Self {
            open: false,
            category: SettingsCategory::Paths,
            snapshot: AppConfig::default(),
        }
    }
}

impl SettingsDialogState {
    pub fn open_with(&mut self, current: &AppConfig, cat: SettingsCategory) {
        self.snapshot = current.clone();
        self.category = cat;
        self.open = true;
    }

    pub fn draw(&mut self, ctx: &egui::Context, cfg: &mut AppConfig) -> SettingsAction {
        if !self.open {
            return SettingsAction::None;
        }

        let mut action = SettingsAction::None;
        let mut window_open = true;
        let before = cfg.clone();
        let def = AppConfig::default();

        egui::Window::new("Ustawienia — Nanxing Converter")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size([720.0_f32, 500.0_f32])
            .open(&mut window_open)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // Ciało podzielone na lewą nawigację kategorii + prawy formularz
                let body_h = 446.0_f32;
                ui.allocate_ui(Vec2::new(ui.available_width(), body_h), |ui| {
                    ui.horizontal_top(|ui| {
                        // Lewy pasek kategorii (165 px)
                        Frame::NONE
                            .fill(BG_0)
                            .stroke(Stroke::new(1.0_f32, LINE))
                            .show(ui, |ui| {
                                ui.set_width(168.0_f32);
                                ui.set_height(body_h);
                                ui.vertical(|ui| {
                                    ui.add_space(6.0_f32);
                                    for cat in SettingsCategory::ALL {
                                        let on = self.category == cat;
                                        let btn = egui::Button::new(
                                            RichText::new(cat.label())
                                                .size(12.0_f32)
                                                .color(if on { PRI_TX } else { TX_2 }),
                                        )
                                        .fill(if on { BG_SEL } else { BG_0 })
                                        .stroke(Stroke::NONE)
                                        .corner_radius(CornerRadius::ZERO);
                                        let resp = ui.add_sized([168.0_f32, 30.0_f32], btn);
                                        if on {
                                            ui.painter().line_segment(
                                                [resp.rect.left_top(), resp.rect.left_bottom()],
                                                Stroke::new(2.0_f32, ACC_2),
                                            );
                                        }
                                        if resp.clicked() {
                                            self.category = cat;
                                        }
                                    }
                                    ui.add_space(16.0_f32);
                                    Frame::NONE
                                        .inner_margin(Margin::symmetric(12, 6))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(
                                                    "Zmiany działają na żywo w podglądzie.\n\nOK zapisuje do nanxing_config.ini, Anuluj cofa.",
                                                )
                                                .size(10.5_f32)
                                                .color(TX_3),
                                            );
                                        });
                                });
                            });

                        // Prawa kolumna z polami
                        Frame::NONE
                            .fill(BG_1)
                            .inner_margin(Margin::same(14))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.set_height(body_h - 28.0_f32);
                                ui.label(
                                    RichText::new(self.category.label().to_uppercase())
                                        .size(11.0_f32)
                                        .strong()
                                        .color(ACC_2),
                                );
                                ui.add_space(8.0_f32);

                                ScrollArea::vertical()
                                    .id_salt("settings_form_scroll")
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing.y = 8.0_f32;
                                        match self.category {
                                            SettingsCategory::Paths => {
                                                path_row(
                                                    ui,
                                                    "Domyślny folder otwierania STEP",
                                                    "Katalog startowy okna wyboru plików lub folderów .step/.stp.",
                                                    &mut cfg.default_open_dir,
                                                    &def.default_open_dir,
                                                );
                                                path_row(
                                                    ui,
                                                    "Domyślny folder zapisu SCX",
                                                    "Jeśli pusty, program zapisuje do podfolderu obok pliku STEP.",
                                                    &mut cfg.default_save_dir,
                                                    &def.default_save_dir,
                                                );
                                                path_row(
                                                    ui,
                                                    "Domyślny folder CSV obrzeży",
                                                    "Katalog startowy przy wczytywaniu pliku rozkroju .csv.",
                                                    &mut cfg.default_csv_dir,
                                                    &def.default_csv_dir,
                                                );
                                                text_row(
                                                    ui,
                                                    "Podfolder wyjściowy (gdy folder zapisu pusty)",
                                                    "Nazwa podfolderu tworzonego obok plików STEP (np. SCX_NANXING).",
                                                    &mut cfg.output_subfolder,
                                                    &def.output_subfolder,
                                                );
                                            }
                                            SettingsCategory::Lamello => {
                                                num_row(
                                                    ui,
                                                    "Gniazdo w powierzchni (Face 5) — szerokość",
                                                    "Domyślna szerokość dla frezowania Lamello P14 od góry (frez).",
                                                    "mm",
                                                    &mut cfg.lamello_surface_width,
                                                    def.lamello_surface_width,
                                                    0.05,
                                                    5.0..=20.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Gniazdo w powierzchni (Face 5) — głębokość",
                                                    "Domyślna głębokość MaxDepth dla Lamello P14 od góry.",
                                                    "mm",
                                                    &mut cfg.lamello_surface_depth,
                                                    def.lamello_surface_depth,
                                                    0.1,
                                                    5.0..=25.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Gniazdo w krawędzi (Face 1..4) — szerokość",
                                                    "Domyślna szerokość dla gniazda bocznego Lamello P14 (tarcza).",
                                                    "mm",
                                                    &mut cfg.lamello_edge_width,
                                                    def.lamello_edge_width,
                                                    0.05,
                                                    5.0..=20.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Gniazdo w krawędzi (Face 1..4) — głębokość",
                                                    "Domyślna głębokość MaxDepth dla gniazda bocznego Lamello P14.",
                                                    "mm",
                                                    &mut cfg.lamello_edge_depth,
                                                    def.lamello_edge_depth,
                                                    0.1,
                                                    5.0..=25.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Długość gniazda Lamello P14",
                                                    "Parametr Length wpisywany w operacji Type=\"1002\".",
                                                    "mm",
                                                    &mut cfg.lamello_slot_length,
                                                    def.lamello_slot_length,
                                                    1.0,
                                                    30.0..=120.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Otwór techniczny pod klucz imbus — średnica",
                                                    "Średnica otworu na Face 5 dla każdego gniazda bocznego.",
                                                    "mm",
                                                    &mut cfg.lamello_imbus_diameter,
                                                    def.lamello_imbus_diameter,
                                                    0.5,
                                                    3.0..=12.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Otwór pod imbus — odsunięcie od krawędzi",
                                                    "Odległość środka otworu Ø6 od brzegu formatki (domyślnie 7.5 mm).",
                                                    "mm",
                                                    &mut cfg.lamello_imbus_edge_offset,
                                                    def.lamello_imbus_edge_offset,
                                                    0.5,
                                                    3.0..=25.0,
                                                );
                                            }
                                            SettingsCategory::Drilling => {
                                                bool_row(
                                                    ui,
                                                    "Kompensacja stożka wiertła 118°",
                                                    "Dolicza (Ø/2)/tan(59°) dla nieprzelotowych wierteł krętych.",
                                                    &mut cfg.compensate_drill_cone,
                                                    def.compensate_drill_cone,
                                                );
                                                num_row(
                                                    ui,
                                                    "Maks. średnica objęta stożkiem 118°",
                                                    "Otwory płaskodenne powyżej tej średnicy (Ø15, Ø35) bez naddatku.",
                                                    "mm",
                                                    &mut cfg.drill_tip_118_max_dia,
                                                    def.drill_tip_118_max_dia,
                                                    0.5,
                                                    0.0..=20.0,
                                                );
                                                bool_row(
                                                    ui,
                                                    "Limit głębokości wiercenia bocznego",
                                                    "Skraca otwory w krawędziach (Face 1..4) do max długości wiertła.",
                                                    &mut cfg.limit_side_drill,
                                                    def.limit_side_drill,
                                                );
                                                num_row(
                                                    ui,
                                                    "Maksymalna głębokość wiercenia bocznego",
                                                    "Bezwzględny limit dla Face 1..4 (domyślnie 50 mm).",
                                                    "mm",
                                                    &mut cfg.max_side_drill_depth,
                                                    def.max_side_drill_depth,
                                                    1.0,
                                                    10.0..=80.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Naddatek dla otworu poziomego bez dna",
                                                    "Np. trzpień mimośrodu Ø8 wpadający w pionowy otwór Ø15 (+2 mm).",
                                                    "mm",
                                                    &mut cfg.horizontal_no_bottom_allowance,
                                                    def.horizontal_no_bottom_allowance,
                                                    0.5,
                                                    0.0..=10.0,
                                                );
                                                bool_row(
                                                    ui,
                                                    "Uwzględniaj częściowe otwory (< 360°)",
                                                    "Domyślnie WYŁ., aby zaokrąglenia R12 nie tworzyły fałszywych wierceń.",
                                                    &mut cfg.include_partial_holes,
                                                    def.include_partial_holes,
                                                );
                                                bool_row(
                                                    ui,
                                                    "Zamieniaj gzymki Ø11 na Ø12",
                                                    "Automatyczna zamiana otworów Ø11 na pogłębienie proste Ø12.",
                                                    &mut cfg.swap_d11_to_d12,
                                                    def.swap_d11_to_d12,
                                                );
                                                bool_row(
                                                    ui,
                                                    "Automatyczny obrót góra/dół (oś Y)",
                                                    "Obraca formatkę na Face 5, gdy ślepe wiercenia/Lamello są od spodu.",
                                                    &mut cfg.auto_flip_top_bottom,
                                                    def.auto_flip_top_bottom,
                                                );
                                                bool_row(
                                                    ui,
                                                    "Pomijaj puste formatki przy eksporcie",
                                                    "Nie tworzy pliku .scx dla gładkich formatek bez żadnych operacji.",
                                                    &mut cfg.skip_empty_panels,
                                                    def.skip_empty_panels,
                                                );
                                            }
                                            SettingsCategory::MachineClamps => {
                                                num_row(
                                                    ui,
                                                    "Minimalna szerokość formatki (Pr90011)",
                                                    "Poniżej tej wartości program zgłasza błąd limitu maszyny.",
                                                    "mm",
                                                    &mut cfg.min_panel_width,
                                                    def.min_panel_width,
                                                    1.0,
                                                    20.0..=150.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Ostrzeżenie przy łapach (Pr90056)",
                                                    "Alarm przytrzaśnięcia dla wąskich formatek (< 100 mm).",
                                                    "mm",
                                                    &mut cfg.anti_pinch_warning_width,
                                                    def.anti_pinch_warning_width,
                                                    5.0,
                                                    45.0..=250.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Grubość tarczy piły nutującej",
                                                    "Fizyczna grubość piły w osi X (domyślnie 4 mm).",
                                                    "mm",
                                                    &mut cfg.saw_blade_thickness,
                                                    def.saw_blade_thickness,
                                                    0.5,
                                                    2.0..=10.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Domyślna szerokość nuty",
                                                    "Szerokość rowka wpisywana w Type=\"4\" (domyślnie 6 mm).",
                                                    "mm",
                                                    &mut cfg.default_groove_width,
                                                    def.default_groove_width,
                                                    0.5,
                                                    3.0..=20.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Dolny frez stały — średnica",
                                                    "Średnica narzędzia zamontowanego na stałe na dolnym wrzecionie.",
                                                    "mm",
                                                    &mut cfg.bottom_spindle_dia,
                                                    def.bottom_spindle_dia,
                                                    1.0,
                                                    6.0..=25.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Położenie pierwszej łapy",
                                                    "Środek Łapy 1 od lewej (Łapa 2 jest odbiciem symetrycznym).",
                                                    "% dł.",
                                                    &mut cfg.claw_pos_pct,
                                                    def.claw_pos_pct,
                                                    1.0,
                                                    5.0..=45.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Zachodzenie szczęki na formatkę",
                                                    "Wizualne zachodzenie szczęki na krawędź formatki w podglądzie 2D.",
                                                    "mm",
                                                    &mut cfg.claw_grip_mm,
                                                    def.claw_grip_mm,
                                                    1.0,
                                                    1.0..=30.0,
                                                );
                                                num_row(
                                                    ui,
                                                    "Bolec pozycjonujący — średnica",
                                                    "Średnica rysowanego bolca w prawym górnym narożniku.",
                                                    "mm",
                                                    &mut cfg.pin_dia_mm,
                                                    def.pin_dia_mm,
                                                    1.0,
                                                    6.0..=30.0,
                                                );
                                            }
                                            SettingsCategory::Viewport => {
                                                num_row(
                                                    ui,
                                                    "Siatka podglądu — oczko",
                                                    "Odstęp linii siatki CAD (linie główne co 5 oczek).",
                                                    "mm",
                                                    &mut cfg.grid_step_mm,
                                                    def.grid_step_mm,
                                                    10.0,
                                                    10.0..=500.0,
                                                );
                                            }
                                        }
                                    });
                            });
                    });
                });

                // Stopka modala (.df)
                Frame::NONE
                    .fill(BG_2)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Przywróć domyślne (kategoria)").clicked() {
                                reset_category(cfg, self.category);
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let ok_btn = egui::Button::new(
                                        RichText::new("OK · Zapisz").color(PRI_TX),
                                    )
                                    .fill(BG_PRI)
                                    .stroke(Stroke::new(1.0_f32, PRI_BORDER))
                                    .corner_radius(CornerRadius::ZERO);
                                    if ui.add_sized([110.0_f32, 28.0_f32], ok_btn).clicked() {
                                        action = SettingsAction::SaveAndClose;
                                    }
                                    ui.add_space(6.0_f32);
                                    if ui
                                        .add_sized([84.0_f32, 28.0_f32], egui::Button::new("Anuluj"))
                                        .clicked()
                                    {
                                        action = SettingsAction::CancelAndRevert;
                                    }
                                },
                            );
                        });
                    });
            });

        if !window_open {
            action = SettingsAction::CancelAndRevert;
        }

        match action {
            SettingsAction::SaveAndClose => {
                self.open = false;
                SettingsAction::SaveAndClose
            }
            SettingsAction::CancelAndRevert => {
                *cfg = self.snapshot.clone();
                self.open = false;
                SettingsAction::CancelAndRevert
            }
            _ => {
                if *cfg != before {
                    SettingsAction::ChangedLive
                } else {
                    SettingsAction::None
                }
            }
        }
    }
}

fn reset_category(cfg: &mut AppConfig, cat: SettingsCategory) {
    let d = AppConfig::default();
    match cat {
        SettingsCategory::Paths => {
            cfg.default_open_dir = d.default_open_dir;
            cfg.default_save_dir = d.default_save_dir;
            cfg.default_csv_dir = d.default_csv_dir;
            cfg.output_subfolder = d.output_subfolder;
        }
        SettingsCategory::Lamello => {
            cfg.lamello_surface_width = d.lamello_surface_width;
            cfg.lamello_surface_depth = d.lamello_surface_depth;
            cfg.lamello_edge_width = d.lamello_edge_width;
            cfg.lamello_edge_depth = d.lamello_edge_depth;
            cfg.lamello_slot_length = d.lamello_slot_length;
            cfg.lamello_imbus_diameter = d.lamello_imbus_diameter;
            cfg.lamello_imbus_edge_offset = d.lamello_imbus_edge_offset;
        }
        SettingsCategory::Drilling => {
            cfg.compensate_drill_cone = d.compensate_drill_cone;
            cfg.drill_tip_118_max_dia = d.drill_tip_118_max_dia;
            cfg.limit_side_drill = d.limit_side_drill;
            cfg.max_side_drill_depth = d.max_side_drill_depth;
            cfg.horizontal_no_bottom_allowance = d.horizontal_no_bottom_allowance;
            cfg.include_partial_holes = d.include_partial_holes;
            cfg.swap_d11_to_d12 = d.swap_d11_to_d12;
            cfg.auto_flip_top_bottom = d.auto_flip_top_bottom;
            cfg.skip_empty_panels = d.skip_empty_panels;
        }
        SettingsCategory::MachineClamps => {
            cfg.min_panel_width = d.min_panel_width;
            cfg.anti_pinch_warning_width = d.anti_pinch_warning_width;
            cfg.saw_blade_thickness = d.saw_blade_thickness;
            cfg.default_groove_width = d.default_groove_width;
            cfg.bottom_spindle_dia = d.bottom_spindle_dia;
            cfg.claw_pos_pct = d.claw_pos_pct;
            cfg.claw_width_pct = d.claw_width_pct;
            cfg.claw_min_mm = d.claw_min_mm;
            cfg.claw_max_mm = d.claw_max_mm;
            cfg.claw_grip_mm = d.claw_grip_mm;
            cfg.claw_depth_mm = d.claw_depth_mm;
            cfg.pin_dx_mm = d.pin_dx_mm;
            cfg.pin_dy_mm = d.pin_dy_mm;
            cfg.pin_dia_mm = d.pin_dia_mm;
        }
        SettingsCategory::Viewport => {
            cfg.grid_step_mm = d.grid_step_mm;
        }
    }
}

fn path_row(ui: &mut egui::Ui, label: &str, hint: &str, value: &mut String, default_val: &str) {
    let changed = value != default_val;
    Frame::NONE
        .fill(BG_0)
        .stroke(Stroke::new(1.0_f32, if changed { ACC } else { LINE }))
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(label).size(11.5_f32).color(TX));
            ui.label(RichText::new(hint).size(10.0_f32).color(TX_3));
            ui.add_space(4.0_f32);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(value)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(ui.available_width() - 62.0_f32),
                );
                if ui
                    .add_sized([28.0_f32, 22.0_f32], egui::Button::new("…"))
                    .on_hover_text("Wybierz folder")
                    .clicked()
                {
                    let mut dlg = rfd::FileDialog::new();
                    if !value.trim().is_empty() {
                        let p = std::path::Path::new(value.as_str());
                        if p.exists() {
                            dlg = dlg.set_directory(p);
                        }
                    }
                    if let Some(folder) = dlg.pick_folder() {
                        *value = folder.display().to_string();
                    }
                }
                if ui
                    .add_enabled(changed, egui::Button::new("↺"))
                    .on_hover_text(format!("Domyślnie: {default_val}"))
                    .clicked()
                {
                    *value = default_val.to_string();
                }
            });
        });
}

fn text_row(ui: &mut egui::Ui, label: &str, hint: &str, value: &mut String, default_val: &str) {
    let changed = value != default_val;
    Frame::NONE
        .fill(BG_0)
        .stroke(Stroke::new(1.0_f32, if changed { ACC } else { LINE }))
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).size(11.5_f32).color(TX));
                    ui.label(RichText::new(hint).size(10.0_f32).color(TX_3));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(changed, egui::Button::new("↺"))
                        .on_hover_text(format!("Domyślnie: {default_val}"))
                        .clicked()
                    {
                        *value = default_val.to_string();
                    }
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(140.0_f32),
                    );
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn num_row(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    unit: &str,
    value: &mut f64,
    default_val: f64,
    step: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    let changed = (*value - default_val).abs() > 1e-4;
    Frame::NONE
        .fill(BG_0)
        .stroke(Stroke::new(1.0_f32, if changed { ACC } else { LINE }))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).size(11.5_f32).color(TX));
                    ui.label(RichText::new(hint).size(10.0_f32).color(TX_3));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(changed, egui::Button::new("↺"))
                        .on_hover_text(format!("Domyślnie: {default_val} {unit}"))
                        .clicked()
                    {
                        *value = default_val;
                    }
                    ui.label(RichText::new(unit).monospace().size(10.5_f32).color(TX_3));
                    ui.add(
                        egui::DragValue::new(value)
                            .speed(step)
                            .range(range)
                            .max_decimals(2),
                    );
                });
            });
        });
}

fn bool_row(ui: &mut egui::Ui, label: &str, hint: &str, value: &mut bool, default_val: bool) {
    let changed = *value != default_val;
    Frame::NONE
        .fill(BG_0)
        .stroke(Stroke::new(1.0_f32, if changed { ACC } else { LINE }))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).size(11.5_f32).color(TX));
                    ui.label(RichText::new(hint).size(10.0_f32).color(TX_3));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(changed, egui::Button::new("↺"))
                        .on_hover_text(format!(
                            "Domyślnie: {}",
                            if default_val { "WŁ." } else { "WYŁ." }
                        ))
                        .clicked()
                    {
                        *value = default_val;
                    }
                    ui.checkbox(value, "");
                });
            });
        });
}
