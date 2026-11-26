use crate::theme::*;
use egui::{
    Align, Align2, Color32, FontId, Frame, Id, Key, Label, Layout, Margin, Order, Pos2, Rect,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, Vec2,
};
use nanxing_core::config::AppConfig;
use nanxing_core::dxf_writer::DxfContourMode;
use nanxing_core::format_num;
use std::collections::HashMap;

// Kolory z .dlg.sdlg (nanxing-makieta_rev2.html)
const SDLG_BG: Color32 = Color32::from_rgb(20, 20, 20); // #141414 (.dlg)
const SDLG_BORDER: Color32 = Color32::from_rgb(51, 51, 51); // #333333 (.dlg border)
const SDLG_HDR_BG: Color32 = Color32::from_rgb(25, 25, 25); // #191919 (.dh)
const SDLG_CLOSE_HOV: Color32 = Color32::from_rgb(163, 50, 43); // #a3322b (.dx:hover)
const SDLG_NAV_BG: Color32 = Color32::from_rgb(17, 17, 17); // #111111 (.sbody nav)
const SDLG_NAV_HOV: Color32 = Color32::from_rgb(24, 24, 24); // #181818 (.sbody nav button:hover)
const SDLG_ROW_DIV: Color32 = Color32::from_rgb(27, 27, 27); // #1b1b1b (.frow border-bottom)
const SDLG_BTN_BG: Color32 = Color32::from_rgb(26, 26, 26); // #1a1a1a (.dbtn)
const SDLG_BTN_HOV: Color32 = Color32::from_rgb(36, 40, 44); // .dbtn:hover

// Identyczna szerokość i wysokość pola liczbowego oraz przełącznika suwakowego
const CTRL_W: f32 = 88.0;
const CTRL_H: f32 = 24.0;
const RESET_S: f32 = 22.0;
const CTRL_GAP: f32 = 8.0;
const RIGHT_COL_W: f32 = CTRL_W + CTRL_GAP + RESET_S; // 118 px
const COL_GAP: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsCategory {
    Paths,
    DxfBsolid,
    RulesDrilling,
    Lamello,
    MachineClamps,
    Viewport,
}

impl SettingsCategory {
    pub const ALL: [SettingsCategory; 6] = [
        SettingsCategory::Paths,
        SettingsCategory::DxfBsolid,
        SettingsCategory::RulesDrilling,
        SettingsCategory::Lamello,
        SettingsCategory::MachineClamps,
        SettingsCategory::Viewport,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Paths => "Ścieżki i Eksport",
            Self::DxfBsolid => "Eksport DXF",
            Self::RulesDrilling => "Reguły i Wiercenie",
            Self::Lamello => "Lamello P14",
            Self::MachineClamps => "Maszyna i Mocowanie",
            Self::Viewport => "Podgląd",
        }
    }

    pub fn heading_upper(self) -> &'static str {
        match self {
            Self::Paths => "ŚCIEŻKI I EKSPORT (PODFOLDERY SCX / DXF)",
            Self::DxfBsolid => "EKSPORT 2D DXF (BSOLID / NAKLEJKI)",
            Self::RulesDrilling => "REGUŁY I WIERCENIE",
            Self::Lamello => "LAMELLO P14",
            Self::MachineClamps => "MASZYNA I MOCOWANIE",
            Self::Viewport => "PODGLĄD",
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
    edit_buffers: HashMap<String, String>,
}

impl Default for SettingsDialogState {
    fn default() -> Self {
        Self {
            open: false,
            category: SettingsCategory::Paths,
            snapshot: AppConfig::default(),
            edit_buffers: HashMap::new(),
        }
    }
}

impl SettingsDialogState {
    pub fn open_with(&mut self, current: &AppConfig, cat: SettingsCategory) {
        self.snapshot = current.clone();
        self.category = cat;
        self.edit_buffers.clear();
        self.open = true;
    }

    pub fn draw(&mut self, ctx: &egui::Context, cfg: &mut AppConfig) -> SettingsAction {
        if !self.open {
            return SettingsAction::None;
        }

        let mut action = SettingsAction::None;
        let before = cfg.clone();
        let def = AppConfig::default();

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            *cfg = self.snapshot.clone();
            self.edit_buffers.clear();
            self.open = false;
            return SettingsAction::CancelAndRevert;
        }

        // Przyciemnione tło .ov-bg (rgba(0,0,0,0.6))
        let screen_rect = ctx.screen_rect();
        let mut backdrop_clicked_pos: Option<Pos2> = None;
        egui::Area::new(Id::new("settings_ov_backdrop"))
            .order(Order::Foreground)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (bg_rect, bg_resp) =
                    ui.allocate_exact_size(screen_rect.size(), Sense::click());
                ui.painter().rect_filled(
                    bg_rect,
                    0.0_f32,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 155),
                );
                if bg_resp.clicked() {
                    backdrop_clicked_pos = bg_resp.interact_pointer_pos();
                }
            });

        const DLG_W: f32 = 780.0;
        const HDR_H: f32 = 32.0;
        const BODY_H: f32 = 468.0;
        const FTR_H: f32 = 48.0;
        const NAV_W: f32 = 170.0;
        const FORM_OUTER_W: f32 = DLG_W - NAV_W;

        let mut modal_rect = Rect::NOTHING;
        let active_cat = self.category;
        let mut next_cat = self.category;
        let mut do_reset_cat = false;

        egui::Area::new(Id::new("settings_sdlg_modal"))
            .order(Order::Tooltip)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let frame_resp = Frame::NONE
                    .fill(SDLG_BG)
                    .stroke(Stroke::new(1.0_f32, SDLG_BORDER))
                    .show(ui, |ui| {
                        ui.set_width(DLG_W);
                        ui.spacing_mut().item_spacing = Vec2::ZERO;

                        // ── 1. Nagłówek .dh ──
                        let (hdr_rect, _) =
                            ui.allocate_exact_size(Vec2::new(DLG_W, HDR_H), Sense::click());
                        ui.painter().rect_filled(hdr_rect, 0.0_f32, SDLG_HDR_BG);
                        ui.painter().line_segment(
                            [hdr_rect.left_bottom(), hdr_rect.right_bottom()],
                            Stroke::new(1.0_f32, LINE),
                        );
                        ui.painter().text(
                            Pos2::new(hdr_rect.left() + 14.0_f32, hdr_rect.center().y),
                            Align2::LEFT_CENTER,
                            "Ustawienia",
                            FontId::proportional(12.5_f32),
                            TX,
                        );

                        let close_rect = Rect::from_min_size(
                            Pos2::new(hdr_rect.right() - 34.0_f32, hdr_rect.top() + 3.0_f32),
                            Vec2::new(28.0_f32, 26.0_f32),
                        );
                        let close_resp = ui
                            .interact(close_rect, Id::new("settings_close_x"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if close_resp.hovered() {
                            ui.painter()
                                .rect_filled(close_rect, 0.0_f32, SDLG_CLOSE_HOV);
                        }
                        ui.painter().text(
                            close_rect.center(),
                            Align2::CENTER_CENTER,
                            "✕",
                            FontId::proportional(12.5_f32),
                            if close_resp.hovered() {
                                Color32::WHITE
                            } else {
                                TX_2
                            },
                        );
                        if close_resp.clicked() {
                            action = SettingsAction::CancelAndRevert;
                        }

                        // ── 2. Korpus .sbody ──
                        ui.allocate_ui_with_layout(
                            Vec2::new(DLG_W, BODY_H),
                            Layout::left_to_right(Align::TOP),
                            |ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;

                                // Lewa nawigacja <nav>
                                ui.allocate_ui_with_layout(
                                    Vec2::new(NAV_W, BODY_H),
                                    Layout::top_down(Align::LEFT),
                                    |ui| {
                                        let nav_full_rect = Rect::from_min_size(
                                            ui.cursor().min,
                                            Vec2::new(NAV_W, BODY_H),
                                        );
                                        ui.painter()
                                            .rect_filled(nav_full_rect, 0.0_f32, SDLG_NAV_BG);
                                        ui.painter().line_segment(
                                            [
                                                nav_full_rect.right_top(),
                                                nav_full_rect.right_bottom(),
                                            ],
                                            Stroke::new(1.0_f32, LINE),
                                        );

                                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                                        ui.add_space(8.0_f32);

                                        for cat in SettingsCategory::ALL {
                                            let on = active_cat == cat;
                                            let (item_rect, item_resp) = ui.allocate_exact_size(
                                                Vec2::new(NAV_W - 1.0_f32, 32.0_f32),
                                                Sense::click(),
                                            );
                                            let item_resp = item_resp
                                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                                            let bg = if on {
                                                BG_SEL
                                            } else if item_resp.hovered() {
                                                SDLG_NAV_HOV
                                            } else {
                                                SDLG_NAV_BG
                                            };
                                            ui.painter().rect_filled(item_rect, 0.0_f32, bg);
                                            if on {
                                                ui.painter().line_segment(
                                                    [
                                                        Pos2::new(
                                                            item_rect.left() + 1.0_f32,
                                                            item_rect.top(),
                                                        ),
                                                        Pos2::new(
                                                            item_rect.left() + 1.0_f32,
                                                            item_rect.bottom(),
                                                        ),
                                                    ],
                                                    Stroke::new(2.0_f32, ACC_2),
                                                );
                                            }
                                            ui.painter().text(
                                                Pos2::new(
                                                    item_rect.left() + 16.0_f32,
                                                    item_rect.center().y,
                                                ),
                                                Align2::LEFT_CENTER,
                                                cat.label(),
                                                FontId::proportional(12.0_f32),
                                                if on { TX } else { TX_2 },
                                            );
                                            if item_resp.clicked() {
                                                next_cat = cat;
                                            }
                                        }

                                        let used_h = 8.0_f32
                                            + (SettingsCategory::ALL.len() as f32) * 32.0_f32;
                                        let note_h = 68.0_f32;
                                        let spacer = (BODY_H - used_h - note_h).max(8.0_f32);
                                        ui.add_space(spacer);
                                        Frame::NONE
                                            .inner_margin(Margin::symmetric(14, 10))
                                            .show(ui, |ui| {
                                                ui.set_width(NAV_W - 29.0_f32);
                                                ui.add(
                                                    Label::new(
                                                        RichText::new(
                                                            "Zmiany działają na żywo w podglądzie. OK zapisuje, Anuluj cofa.",
                                                        )
                                                        .size(10.5_f32)
                                                        .color(TX_3),
                                                    )
                                                    .wrap(),
                                                );
                                            });
                                    },
                                );

                                // Prawa kolumna .sform
                                ui.allocate_ui_with_layout(
                                    Vec2::new(FORM_OUTER_W, BODY_H),
                                    Layout::top_down(Align::LEFT),
                                    |ui| {
                                        Frame::NONE
                                            .fill(SDLG_BG)
                                            .inner_margin(Margin {
                                                left: 18,
                                                right: 14,
                                                top: 4,
                                                bottom: 10,
                                            })
                                            .show(ui, |ui| {
                                                let inner_w = FORM_OUTER_W - 32.0_f32;
                                                ui.set_width(inner_w);
                                                ui.set_height(BODY_H - 14.0_f32);
                                                ui.spacing_mut().item_spacing = Vec2::ZERO;

                                                ui.add_space(10.0_f32);
                                                ui.label(
                                                    RichText::new(active_cat.heading_upper())
                                                        .size(10.5_f32)
                                                        .strong()
                                                        .color(TX_3),
                                                );
                                                ui.add_space(6.0_f32);
                                                let (h3_line, _) = ui.allocate_exact_size(
                                                    Vec2::new(inner_w, 1.0_f32),
                                                    Sense::hover(),
                                                );
                                                ui.painter().line_segment(
                                                    [h3_line.left_top(), h3_line.right_top()],
                                                    Stroke::new(1.0_f32, LINE),
                                                );

                                                let row_w = inner_w - 10.0_f32;
                                                let bufs = &mut self.edit_buffers;
                                                ScrollArea::vertical()
                                                    .id_salt(("sdlg_scroll", active_cat))
                                                    .auto_shrink([false, false])
                                                    .show(ui, |ui| {
                                                        ui.set_width(row_w);
                                                        ui.spacing_mut().item_spacing = Vec2::ZERO;

                                                        match active_cat {
                                                            SettingsCategory::Paths => {
                                                                path_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Domyślny folder otwierania STEP",
                                                                    "Katalog startowy okna wyboru plików lub folderów .step / .stp.",
                                                                    &mut cfg.default_open_dir,
                                                                    &def.default_open_dir,
                                                                );
                                                                path_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Katalog bazowy zapisu (SCX i DXF)",
                                                                    "Gdy puste — obok plików STEP automatycznie powstaną podfoldery SCX oraz DXF (zawsze 3-literowe nazwy).",
                                                                    &mut cfg.default_save_dir,
                                                                    &def.default_save_dir,
                                                                );
                                                                path_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Domyślny folder CSV obrzeży",
                                                                    "Katalog startowy przy wczytywaniu pliku rozkroju .csv.",
                                                                    &mut cfg.default_csv_dir,
                                                                    &def.default_csv_dir,
                                                                );
                                                            }
                                                            SettingsCategory::DxfBsolid => {
                                                                let mut step_contour = cfg.dxf_contour_mode == DxfContourMode::StepOutline;
                                                                let def_step_contour = def.dxf_contour_mode == DxfContourMode::StepOutline;
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Kontur wyciągnięty ze STEPa na ROZKROJ",
                                                                    "Gdy wyłączone (domyślnie): na ROZKROJ trafia prostokąt pod formatyzację, a wycięcia/ścięte rogi na FREZOWANIE.",
                                                                    &mut step_contour,
                                                                    def_step_contour,
                                                                );
                                                                cfg.dxf_contour_mode = if step_contour {
                                                                    DxfContourMode::StepOutline
                                                                } else {
                                                                    DxfContourMode::RectFormat
                                                                };

                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Ukryj strzałkę usłojenia",
                                                                    "Gdy wyłączone, rysuje strzałkę orientacji wg Grain (L = oś X, W = oś Y).",
                                                                    &mut cfg.dxf_hide_grain_arrow,
                                                                    def.dxf_hide_grain_arrow,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Rozdziel otwory wiercone i frezowane",
                                                                    "Gdy wyłączone, wiercenia i frezowania trafiają razem na warstwę wspólną.",
                                                                    &mut cfg.dxf_split_drill_and_mill,
                                                                    def.dxf_split_drill_and_mill,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Granica średnicy wiercenia",
                                                                    "Do tej średnicy otwory trafiają na WIERCENIE, powyżej na FREZOWANIE.",
                                                                    "mm",
                                                                    &mut cfg.dxf_drill_max_dia_mm,
                                                                    def.dxf_drill_max_dia_mm,
                                                                    5.0..=100.0,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Osobne warstwy z głębokością (GW / GF)",
                                                                    "Tworzy warstwy nieprzelotowe np. Wiercenie GW12, Frezowanie GF6.",
                                                                    &mut cfg.dxf_depth_layers,
                                                                    def.dxf_depth_layers,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Wyciągaj otwory wiercone",
                                                                    "Umieszcza okręgi otworów pionowych w pliku 2D DXF.",
                                                                    &mut cfg.dxf_draw_drills,
                                                                    def.dxf_draw_drills,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Wyciągaj otwory w krawędziach",
                                                                    "Rysuje otwory boczne i gniazda krawędziowe jako prostokąty przy konturze.",
                                                                    &mut cfg.dxf_draw_side_holes,
                                                                    def.dxf_draw_side_holes,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Wyciągaj nuty i wycięcia frezowane",
                                                                    "Umieszcza rowki (Groove), gniazda płaszczyznowe oraz duże otwory na FREZOWANIE.",
                                                                    &mut cfg.dxf_draw_milling,
                                                                    def.dxf_draw_milling,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Uwzględniaj operacje od dołu (Face 6)",
                                                                    "Eksportuje do DXF również otwory/operacje wykonywane na dolnej płaszczyźnie.",
                                                                    &mut cfg.dxf_include_bottom_ops,
                                                                    def.dxf_include_bottom_ops,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Skala strzałki (>= 300×200 mm)",
                                                                    "Proporcja wysokości grotu strzałki do mniejszego boku dużej formatki.",
                                                                    "",
                                                                    &mut cfg.dxf_arrow_scale_large,
                                                                    def.dxf_arrow_scale_large,
                                                                    0.05..=0.95,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Skala strzałki (< 300×200 mm)",
                                                                    "Proporcja wysokości grotu strzałki dla małych i wąskich elementów.",
                                                                    "",
                                                                    &mut cfg.dxf_arrow_scale_small,
                                                                    def.dxf_arrow_scale_small,
                                                                    0.05..=0.95,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa rozkroju",
                                                                    "Nazwa warstwy konturu zewnętrznego (kolor czerwony).",
                                                                    &mut cfg.dxf_layer_rozkroj,
                                                                    &def.dxf_layer_rozkroj,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa wiercenia",
                                                                    "Nazwa warstwy otworów wierconych <= 36 mm (kolor cyjan).",
                                                                    &mut cfg.dxf_layer_wiercenie,
                                                                    &def.dxf_layer_wiercenie,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa frezowania",
                                                                    "Nazwa warstwy frezowań, rowków i dużych otworów (kolor żółty).",
                                                                    &mut cfg.dxf_layer_frezowanie,
                                                                    &def.dxf_layer_frezowanie,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa otworów w krawędziach",
                                                                    "Nazwa warstwy wierceń bocznych Face 1..4 (kolor zielony).",
                                                                    &mut cfg.dxf_layer_wiercenie_bok,
                                                                    &def.dxf_layer_wiercenie_bok,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa Lamello w krawędziach",
                                                                    "Nazwa warstwy łukowatych gniazd Lamello P14 w bokach formatki (kolor pomarańczowy).",
                                                                    &mut cfg.dxf_layer_lamello_bok,
                                                                    &def.dxf_layer_lamello_bok,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa Lamello w powierzchni",
                                                                    "Nazwa warstwy gniazd Lamello P14 w kształcie kości w płaszczyźnie (kolor fioletowy).",
                                                                    &mut cfg.dxf_layer_lamello_pow,
                                                                    &def.dxf_layer_lamello_pow,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa wspólna (gdy bez podziału)",
                                                                    "Używana, gdy wyłączono rozdzielanie wierceń i frezowań.",
                                                                    &mut cfg.dxf_layer_widmo,
                                                                    &def.dxf_layer_widmo,
                                                                );
                                                                path_like_text_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Warstwa strzałki usłojenia",
                                                                    "Nazwa warstwy strzałki orientacji (kolor magenta).",
                                                                    &mut cfg.dxf_layer_strzalka,
                                                                    &def.dxf_layer_strzalka,
                                                                );
                                                            }
                                                            SettingsCategory::RulesDrilling => {
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Kompensacja stożka wiertła 118°",
                                                                    "Dolicza (Ø/2)/tan(59°) dla nieprzelotowych wierteł krętych.",
                                                                    &mut cfg.compensate_drill_cone,
                                                                    def.compensate_drill_cone,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Maks. średnica objęta stożkiem 118°",
                                                                    "Otwory płaskodenne powyżej tej średnicy (Ø15, Ø35) bez naddatku.",
                                                                    "mm",
                                                                    &mut cfg.drill_tip_118_max_dia,
                                                                    def.drill_tip_118_max_dia,
                                                                    0.0..=20.0,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Limit głębokości wiercenia bocznego",
                                                                    "Skraca otwory w krawędziach (Face 1..4) do maks. długości wiertła.",
                                                                    &mut cfg.limit_side_drill,
                                                                    def.limit_side_drill,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Maks. głębokość wiercenia bocznego",
                                                                    "Bezwzględny limit głębokości dla Face 1..4 (domyślnie 50 mm).",
                                                                    "mm",
                                                                    &mut cfg.max_side_drill_depth,
                                                                    def.max_side_drill_depth,
                                                                    10.0..=80.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Naddatek dla otworu poziomego bez dna",
                                                                    "Np. trzpień mimośrodu Ø8 wpadający w pionowy otwór Ø15 (+2 mm).",
                                                                    "mm",
                                                                    &mut cfg.horizontal_no_bottom_allowance,
                                                                    def.horizontal_no_bottom_allowance,
                                                                    0.0..=10.0,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Uwzględniaj częściowe otwory (< 360°)",
                                                                    "Gdy wyłączone, pomija otwarte łuki narożne (np. zaokrąglenia R12).",
                                                                    &mut cfg.include_partial_holes,
                                                                    def.include_partial_holes,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Zamieniaj gzymki Ø11 na Ø12",
                                                                    "Automatyczna zamiana otworów Ø11 na pogłębienie proste Ø12.",
                                                                    &mut cfg.swap_d11_to_d12,
                                                                    def.swap_d11_to_d12,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Automatyczny obrót góra/dół (oś Y)",
                                                                    "Ustawia płaszczyznę z większą liczbą otworów / Lamello na Z+ (Face 5).",
                                                                    &mut cfg.auto_flip_top_bottom,
                                                                    def.auto_flip_top_bottom,
                                                                );
                                                                toggle_row(
                                                                    ui,
                                                                    row_w,
                                                                    "Pomijaj puste formatki przy eksporcie",
                                                                    "Nie tworzy pliku .scx dla gładkich formatek bez żadnych operacji.",
                                                                    &mut cfg.skip_empty_panels,
                                                                    def.skip_empty_panels,
                                                                );
                                                            }
                                                            SettingsCategory::Lamello => {
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Gniazdo w powierzchni (Face 5) — szerokość",
                                                                    "Domyślna szerokość dla frezowania Lamello P14 od góry (frez).",
                                                                    "mm",
                                                                    &mut cfg.lamello_surface_width,
                                                                    def.lamello_surface_width,
                                                                    5.0..=20.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Gniazdo w powierzchni (Face 5) — głębokość",
                                                                    "Domyślna głębokość MaxDepth dla Lamello P14 od góry.",
                                                                    "mm",
                                                                    &mut cfg.lamello_surface_depth,
                                                                    def.lamello_surface_depth,
                                                                    5.0..=25.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Gniazdo w krawędzi (Face 1..4) — szerokość",
                                                                    "Domyślna szerokość dla gniazda bocznego Lamello P14 (tarcza).",
                                                                    "mm",
                                                                    &mut cfg.lamello_edge_width,
                                                                    def.lamello_edge_width,
                                                                    5.0..=20.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Gniazdo w krawędzi (Face 1..4) — głębokość",
                                                                    "Domyślna głębokość MaxDepth dla gniazda bocznego Lamello P14.",
                                                                    "mm",
                                                                    &mut cfg.lamello_edge_depth,
                                                                    def.lamello_edge_depth,
                                                                    5.0..=25.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Długość gniazda Lamello P14",
                                                                    "Parametr Length wpisywany w operacji Type=\"1002\".",
                                                                    "mm",
                                                                    &mut cfg.lamello_slot_length,
                                                                    def.lamello_slot_length,
                                                                    30.0..=120.0,
                                                                );
                                                            }
                                                            SettingsCategory::MachineClamps => {
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Minimalna szerokość formatki (Pr90011)",
                                                                    "Poniżej tej wartości program zgłasza błąd limitu maszyny.",
                                                                    "mm",
                                                                    &mut cfg.min_panel_width,
                                                                    def.min_panel_width,
                                                                    20.0..=150.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Ostrzeżenie przy łapach (Pr90056)",
                                                                    "Alarm przytrzaśnięcia dla wąskich formatek (< 100 mm).",
                                                                    "mm",
                                                                    &mut cfg.anti_pinch_warning_width,
                                                                    def.anti_pinch_warning_width,
                                                                    45.0..=250.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Grubość tarczy piły nutującej",
                                                                    "Fizyczna grubość piły w osi X (domyślnie 4 mm).",
                                                                    "mm",
                                                                    &mut cfg.saw_blade_thickness,
                                                                    def.saw_blade_thickness,
                                                                    2.0..=10.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Domyślna szerokość nuty",
                                                                    "Szerokość rowka wpisywana w Type=\"4\" (domyślnie 6 mm).",
                                                                    "mm",
                                                                    &mut cfg.default_groove_width,
                                                                    def.default_groove_width,
                                                                    3.0..=20.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Dolny frez stały — średnica",
                                                                    "Średnica narzędzia zamontowanego na stałe na dolnym wrzecionie.",
                                                                    "mm",
                                                                    &mut cfg.bottom_spindle_dia,
                                                                    def.bottom_spindle_dia,
                                                                    6.0..=25.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Położenie łap zaciskowych",
                                                                    "Środek łapy od krawędzi bocznej (obie łapy symetrycznie).",
                                                                    "%",
                                                                    &mut cfg.claw_pos_pct,
                                                                    def.claw_pos_pct,
                                                                    5.0..=45.0,
                                                                );
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Bolec pozycjonujący — średnica",
                                                                    "Średnica rysowanego bolca w prawym górnym narożniku.",
                                                                    "mm",
                                                                    &mut cfg.pin_dia_mm,
                                                                    def.pin_dia_mm,
                                                                    6.0..=30.0,
                                                                );
                                                            }
                                                            SettingsCategory::Viewport => {
                                                                num_row(
                                                                    ui,
                                                                    bufs,
                                                                    row_w,
                                                                    "Siatka podglądu — oczko",
                                                                    "Odstęp linii siatki CAD (linie główne co 5 oczek).",
                                                                    "mm",
                                                                    &mut cfg.grid_step_mm,
                                                                    def.grid_step_mm,
                                                                    10.0..=500.0,
                                                                );
                                                            }
                                                        }
                                                    });
                                            });
                                    },
                                );
                            },
                        );

                        // ── 3. Stopka .df ──
                        let (ftr_top_line, _) =
                            ui.allocate_exact_size(Vec2::new(DLG_W, 1.0_f32), Sense::hover());
                        ui.painter().line_segment(
                            [ftr_top_line.left_top(), ftr_top_line.right_top()],
                            Stroke::new(1.0_f32, LINE),
                        );

                        Frame::NONE
                            .fill(BG_2)
                            .inner_margin(Margin::symmetric(14, 9))
                            .show(ui, |ui| {
                                ui.set_width(DLG_W - 28.0_f32);
                                ui.set_height(FTR_H - 19.0_f32);
                                ui.horizontal_centered(|ui| {
                                    if footer_btn(ui, "Przywróć domyślne", 142.0_f32, false)
                                        .clicked()
                                    {
                                        do_reset_cat = true;
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0_f32;
                                        if footer_btn(ui, "OK", 76.0_f32, true).clicked() {
                                            action = SettingsAction::SaveAndClose;
                                        }
                                        if footer_btn(ui, "Anuluj", 82.0_f32, false).clicked() {
                                            action = SettingsAction::CancelAndRevert;
                                        }
                                    });
                                });
                            });
                    });
                modal_rect = frame_resp.response.rect;
            });

        self.category = next_cat;
        if do_reset_cat {
            reset_category(cfg, self.category);
            self.edit_buffers.clear();
        }

        if let Some(click_pos) = backdrop_clicked_pos {
            if !modal_rect.contains(click_pos) {
                action = SettingsAction::CancelAndRevert;
            }
        }

        match action {
            SettingsAction::SaveAndClose => {
                self.edit_buffers.clear();
                self.open = false;
                SettingsAction::SaveAndClose
            }
            SettingsAction::CancelAndRevert => {
                *cfg = self.snapshot.clone();
                self.edit_buffers.clear();
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

fn footer_btn(ui: &mut egui::Ui, text: &str, width: f32, primary: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 28.0_f32), Sense::click());
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
        (SDLG_BTN_HOV, ACC, Color32::WHITE)
    } else {
        (SDLG_BTN_BG, LINE_2, TX)
    };
    ui.painter().rect_filled(rect, 0.0_f32, fill);
    ui.painter()
        .rect_stroke(rect, 0.0_f32, Stroke::new(1.0_f32, border), StrokeKind::Inside);
    let c = if down {
        rect.center() + Vec2::new(0.0, 1.0)
    } else {
        rect.center()
    };
    ui.painter()
        .text(c, Align2::CENTER_CENTER, text, FontId::proportional(12.0_f32), fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn draw_row_divider(ui: &mut egui::Ui, row_w: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(row_w, 1.0_f32), Sense::hover());
    ui.painter()
        .line_segment([r.left_top(), r.right_top()], Stroke::new(1.0_f32, SDLG_ROW_DIV));
}

fn draw_label_and_hint(ui: &mut egui::Ui, max_w: f32, label: &str, hint: &str, changed: bool) {
    ui.set_width(max_w);
    ui.spacing_mut().item_spacing.y = 2.0_f32;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0_f32;
        ui.label(RichText::new(label).size(12.0_f32).color(TX));
        if changed {
            let (dot_r, _) = ui.allocate_exact_size(Vec2::new(5.0_f32, 5.0_f32), Sense::hover());
            ui.painter().rect_filled(dot_r, 0.0_f32, WARN);
        }
    });
    ui.add(Label::new(RichText::new(hint).size(10.5_f32).color(TX_3)).wrap());
}

fn reset_btn_22(ui: &mut egui::Ui, enabled: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(RESET_S, RESET_S), Sense::click());
    if enabled {
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let border = if resp.hovered() { ACC_2 } else { LINE_2 };
        let fg = if resp.hovered() { Color32::WHITE } else { TX_2 };
        if resp.is_pointer_button_down_on() {
            ui.painter()
                .rect_filled(rect, 0.0_f32, Color32::from_rgb(14, 52, 66));
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 0.0_f32, SDLG_BTN_HOV);
        }
        ui.painter()
            .rect_stroke(rect, 0.0_f32, Stroke::new(1.0_f32, border), StrokeKind::Inside);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "↺",
            FontId::proportional(12.0_f32),
            fg,
        );
        resp.on_hover_text(tip).clicked()
    } else {
        ui.painter().rect_stroke(
            rect,
            0.0_f32,
            Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(LINE_2.r(), LINE_2.g(), LINE_2.b(), 65),
            ),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "↺",
            FontId::proportional(12.0_f32),
            Color32::from_rgba_unmultiplied(TX_2.r(), TX_2.g(), TX_2.b(), 65),
        );
        false
    }
}

/// Pole ścieżki folderu — długie pole tekstowe na całą szerokość pod nazwą opcji
fn path_row(
    ui: &mut egui::Ui,
    row_w: f32,
    label: &str,
    hint: &str,
    value: &mut String,
    default_val: &str,
) {
    let changed = value != default_val;
    ui.add_space(9.0_f32);
    draw_label_and_hint(ui, row_w, label, hint, changed);
    ui.add_space(6.0_f32);

    ui.allocate_ui_with_layout(
        Vec2::new(row_w, 26.0_f32),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0_f32;
            let browse_w = 28.0_f32;
            let edit_w = (row_w - browse_w - RESET_S - 12.0_f32).max(180.0_f32);

            let (box_rect, _) =
                ui.allocate_exact_size(Vec2::new(edit_w, 26.0_f32), Sense::hover());
            ui.painter().rect_filled(box_rect, 0.0_f32, BG_0);

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(box_rect.shrink2(Vec2::new(7.0_f32, 3.0_f32)))
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let te_resp = child.add(
                egui::TextEdit::singleline(value)
                    .frame(false)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(edit_w - 14.0_f32),
            );
            ui.painter().rect_stroke(
                box_rect,
                0.0_f32,
                Stroke::new(1.0_f32, if te_resp.has_focus() { ACC_2 } else { LINE_2 }),
                StrokeKind::Inside,
            );

            let (br_rect, br_resp) =
                ui.allocate_exact_size(Vec2::new(browse_w, 26.0_f32), Sense::click());
            let br_resp = br_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(
                br_rect,
                0.0_f32,
                if br_resp.is_pointer_button_down_on() {
                    Color32::from_rgb(14, 52, 66)
                } else if br_resp.hovered() {
                    SDLG_BTN_HOV
                } else {
                    SDLG_BTN_BG
                },
            );
            ui.painter().rect_stroke(
                br_rect,
                0.0_f32,
                Stroke::new(1.0_f32, if br_resp.hovered() { ACC_2 } else { LINE_2 }),
                StrokeKind::Inside,
            );
            ui.painter().text(
                br_rect.center(),
                Align2::CENTER_CENTER,
                "…",
                FontId::monospace(12.0_f32),
                TX,
            );
            if br_resp.on_hover_text("Wybierz folder").clicked() {
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

            let tip = if default_val.is_empty() {
                "Domyślnie: (puste)".to_string()
            } else {
                format!("Domyślnie: {default_val}")
            };
            if reset_btn_22(ui, changed, &tip) {
                *value = default_val.to_string();
            }
        },
    );

    ui.add_space(9.0_f32);
    draw_row_divider(ui, row_w);
}

/// Długie pole tekstowe (np. nazwa podfolderu wyjściowego) pod nazwą opcji
fn path_like_text_row(
    ui: &mut egui::Ui,
    row_w: f32,
    label: &str,
    hint: &str,
    value: &mut String,
    default_val: &str,
) {
    let changed = value != default_val;
    ui.add_space(9.0_f32);
    draw_label_and_hint(ui, row_w, label, hint, changed);
    ui.add_space(6.0_f32);

    ui.allocate_ui_with_layout(
        Vec2::new(row_w, 26.0_f32),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0_f32;
            let edit_w = (row_w - RESET_S - 6.0_f32).max(180.0_f32);

            let (box_rect, _) =
                ui.allocate_exact_size(Vec2::new(edit_w, 26.0_f32), Sense::hover());
            ui.painter().rect_filled(box_rect, 0.0_f32, BG_0);

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(box_rect.shrink2(Vec2::new(7.0_f32, 3.0_f32)))
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let te_resp = child.add(
                egui::TextEdit::singleline(value)
                    .frame(false)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(edit_w - 14.0_f32),
            );
            ui.painter().rect_stroke(
                box_rect,
                0.0_f32,
                Stroke::new(1.0_f32, if te_resp.has_focus() { ACC_2 } else { LINE_2 }),
                StrokeKind::Inside,
            );

            if reset_btn_22(ui, changed, &format!("Domyślnie: {default_val}")) {
                *value = default_val.to_string();
            }
        },
    );

    ui.add_space(9.0_f32);
    draw_row_divider(ui, row_w);
}

/// Zwykłe pole tekstowe dla wartości liczbowych (wpisywanych z klawiatury) z jednostką zapisaną WEWNĄTRZ pola.
/// Ma dokładnie taką samą szerokość (CTRL_W = 88 px) jak przełącznik suwakowy `toggle_row`.
#[allow(clippy::too_many_arguments)]
fn num_row(
    ui: &mut egui::Ui,
    bufs: &mut HashMap<String, String>,
    row_w: f32,
    label: &str,
    hint: &str,
    unit: &str,
    value: &mut f64,
    default_val: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    let changed = (*value - default_val).abs() > 1e-4;
    let desc_w = (row_w - RIGHT_COL_W - COL_GAP).max(180.0_f32);

    ui.add_space(9.0_f32);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = COL_GAP;
        ui.allocate_ui_with_layout(
            Vec2::new(desc_w, 32.0_f32),
            Layout::top_down(Align::LEFT),
            |ui| {
                draw_label_and_hint(ui, desc_w, label, hint, changed);
            },
        );

        ui.allocate_ui_with_layout(
            Vec2::new(RIGHT_COL_W, CTRL_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = CTRL_GAP;

                let (box_rect, _) =
                    ui.allocate_exact_size(Vec2::new(CTRL_W, CTRL_H), Sense::hover());
                ui.painter().rect_filled(box_rect, 0.0_f32, BG_0);

                // Zmierz szerokość jednostki rysowanej WEWNĄTRZ pola po prawej stronie
                let unit_font = FontId::monospace(10.0_f32);
                let unit_galley =
                    ui.painter()
                        .layout_no_wrap(unit.to_string(), unit_font.clone(), TX_3);
                let unit_w = unit_galley.size().x;

                // Jednostka wewnątrz pola
                ui.painter().text(
                    Pos2::new(box_rect.right() - 6.0_f32, box_rect.center().y),
                    Align2::RIGHT_CENTER,
                    unit,
                    unit_font,
                    TX_3,
                );

                // Zwykłe pole tekstowe TextEdit do wpisywania liczby z klawiatury
                let edit_rect = Rect::from_min_max(
                    Pos2::new(box_rect.left() + 6.0_f32, box_rect.top() + 2.0_f32),
                    Pos2::new(
                        (box_rect.right() - unit_w - 9.0_f32).max(box_rect.left() + 28.0_f32),
                        box_rect.bottom() - 2.0_f32,
                    ),
                );
                let buf = bufs
                    .entry(label.to_string())
                    .or_insert_with(|| format_num(*value));

                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(edit_rect)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                let te_resp = child.add(
                    egui::TextEdit::singleline(buf)
                        .id_salt(("sdlg_num", label))
                        .frame(false)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(edit_rect.width()),
                );

                if te_resp.changed() {
                    if let Ok(parsed) = buf.trim().replace(',', ".").parse::<f64>() {
                        if range.contains(&parsed) {
                            *value = parsed;
                        }
                    }
                }
                if te_resp.lost_focus() {
                    if let Ok(parsed) = buf.trim().replace(',', ".").parse::<f64>() {
                        *value = parsed.clamp(*range.start(), *range.end());
                    }
                    *buf = format_num(*value);
                } else if !te_resp.has_focus() {
                    let expected = format_num(*value);
                    if *buf != expected {
                        *buf = expected;
                    }
                }

                ui.painter().rect_stroke(
                    box_rect,
                    0.0_f32,
                    Stroke::new(1.0_f32, if te_resp.has_focus() { ACC_2 } else { LINE_2 }),
                    StrokeKind::Inside,
                );

                if reset_btn_22(
                    ui,
                    changed,
                    &format!("Domyślnie: {} {}", format_num(default_val), unit),
                ) {
                    *value = default_val;
                    bufs.insert(label.to_string(), format_num(default_val));
                }
            },
        );
    });
    ui.add_space(9.0_f32);
    draw_row_divider(ui, row_w);
}

/// Przełącznik suwakowy (true / false) bez cyferek i bez oznaczeń:
/// - true  -> suwak po PRAWEJ stronie, kolor niebieski
/// - false -> suwak po LEWEJ stronie, kolor wyszarzony
/// Szerokość dokładnie CTRL_W = 88 px (taka sama jak pola liczbowego `num_row`).
fn toggle_row(
    ui: &mut egui::Ui,
    row_w: f32,
    label: &str,
    hint: &str,
    value: &mut bool,
    default_val: bool,
) {
    let changed = *value != default_val;
    let desc_w = (row_w - RIGHT_COL_W - COL_GAP).max(180.0_f32);

    ui.add_space(9.0_f32);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = COL_GAP;
        ui.allocate_ui_with_layout(
            Vec2::new(desc_w, 32.0_f32),
            Layout::top_down(Align::LEFT),
            |ui| {
                draw_label_and_hint(ui, desc_w, label, hint, changed);
            },
        );

        ui.allocate_ui_with_layout(
            Vec2::new(RIGHT_COL_W, CTRL_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = CTRL_GAP;

                let (track_rect, sw_resp) =
                    ui.allocate_exact_size(Vec2::new(CTRL_W, CTRL_H), Sense::click());
                let sw_resp = sw_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                if sw_resp.clicked() {
                    *value = !*value;
                }

                let t = ui
                    .ctx()
                    .animate_bool_with_time(Id::new(("sdlg_sw_anim", label)), *value, 0.11);

                let track_off = Color32::from_rgb(25, 27, 29);
                let track_on = Color32::from_rgb(13, 56, 74);
                let border_off = if sw_resp.hovered() {
                    Color32::from_rgb(84, 90, 96)
                } else {
                    Color32::from_rgb(52, 56, 60)
                };
                let border_on = ACC_2;

                let knob_off = if sw_resp.hovered() {
                    Color32::from_rgb(116, 123, 130)
                } else {
                    Color32::from_rgb(94, 100, 106)
                };
                let knob_on = if sw_resp.hovered() {
                    Color32::from_rgb(44, 184, 232)
                } else {
                    Color32::from_rgb(24, 162, 210)
                };

                let track_fill = lerp_color(track_off, track_on, t);
                let track_border = lerp_color(border_off, border_on, t);
                let knob_fill = lerp_color(knob_off, knob_on, t);

                ui.painter().rect_filled(track_rect, 0.0_f32, track_fill);
                ui.painter().rect_stroke(
                    track_rect,
                    0.0_f32,
                    Stroke::new(1.0_f32, track_border),
                    StrokeKind::Inside,
                );

                let pad = 3.0_f32;
                let knob_w = (CTRL_W - pad * 2.0) * 0.46_f32;
                let knob_h = CTRL_H - pad * 2.0;
                let x_left = track_rect.left() + pad;
                let x_right = track_rect.right() - pad - knob_w;
                let knob_x = x_left + (x_right - x_left) * t;
                let knob_rect = Rect::from_min_size(
                    Pos2::new(knob_x, track_rect.top() + pad),
                    Vec2::new(knob_w, knob_h),
                );
                ui.painter().rect_filled(knob_rect, 0.0_f32, knob_fill);

                if reset_btn_22(
                    ui,
                    changed,
                    &format!(
                        "Domyślnie: {}",
                        if default_val { "włączone" } else { "wyłączone" }
                    ),
                ) {
                    *value = default_val;
                }
            },
        );
    });
    ui.add_space(9.0_f32);
    draw_row_divider(ui, row_w);
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0_f32, 1.0_f32);
    let mix = |x: u8, y: u8| -> u8 { ((x as f32) + ((y as f32) - (x as f32)) * t).round() as u8 };
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
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
        SettingsCategory::DxfBsolid => {
            cfg.dxf_contour_mode = d.dxf_contour_mode;
            cfg.dxf_hide_grain_arrow = d.dxf_hide_grain_arrow;
            cfg.dxf_split_drill_and_mill = d.dxf_split_drill_and_mill;
            cfg.dxf_depth_layers = d.dxf_depth_layers;
            cfg.dxf_draw_drills = d.dxf_draw_drills;
            cfg.dxf_draw_side_holes = d.dxf_draw_side_holes;
            cfg.dxf_draw_milling = d.dxf_draw_milling;
            cfg.dxf_include_bottom_ops = d.dxf_include_bottom_ops;
            cfg.dxf_drill_max_dia_mm = d.dxf_drill_max_dia_mm;
            cfg.dxf_arrow_scale_large = d.dxf_arrow_scale_large;
            cfg.dxf_arrow_scale_small = d.dxf_arrow_scale_small;
            cfg.dxf_layer_rozkroj = d.dxf_layer_rozkroj;
            cfg.dxf_layer_wiercenie = d.dxf_layer_wiercenie;
            cfg.dxf_layer_frezowanie = d.dxf_layer_frezowanie;
            cfg.dxf_layer_wiercenie_bok = d.dxf_layer_wiercenie_bok;
            cfg.dxf_layer_lamello_bok = d.dxf_layer_lamello_bok;
            cfg.dxf_layer_lamello_pow = d.dxf_layer_lamello_pow;
            cfg.dxf_layer_widmo = d.dxf_layer_widmo;
            cfg.dxf_layer_strzalka = d.dxf_layer_strzalka;
        }
        SettingsCategory::RulesDrilling => {
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
        SettingsCategory::Lamello => {
            cfg.lamello_surface_width = d.lamello_surface_width;
            cfg.lamello_surface_depth = d.lamello_surface_depth;
            cfg.lamello_edge_width = d.lamello_edge_width;
            cfg.lamello_edge_depth = d.lamello_edge_depth;
            cfg.lamello_slot_length = d.lamello_slot_length;
            cfg.lamello_imbus_diameter = d.lamello_imbus_diameter;
            cfg.lamello_imbus_edge_offset = d.lamello_imbus_edge_offset;
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
