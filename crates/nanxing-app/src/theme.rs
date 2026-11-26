use egui::{Color32, CornerRadius, FontId, Margin, Stroke, Visuals};
use nanxing_core::model::Operation;

// Paleta z makiety nanxing-makieta_rev2.html (ostre, płaskie CAD/Windows UI)
pub const BG_0: Color32 = Color32::from_rgb(14, 14, 14); // #0e0e0e
pub const BG_1: Color32 = Color32::from_rgb(18, 18, 18); // #121212
pub const BG_2: Color32 = Color32::from_rgb(22, 22, 22); // #161616
pub const BG_CANVAS: Color32 = Color32::from_rgb(12, 12, 12); // #0c0c0c
pub const BG_STATUS: Color32 = Color32::from_rgb(13, 17, 20); // #0d1114
pub const BG_SEL: Color32 = Color32::from_rgb(17, 33, 41); // #112129
pub const BG_PRI: Color32 = Color32::from_rgb(12, 58, 73); // #0c3a49

pub const LINE: Color32 = Color32::from_rgb(37, 37, 37); // #252525
pub const LINE_2: Color32 = Color32::from_rgb(48, 48, 48); // #303030

pub const TX: Color32 = Color32::from_rgb(205, 209, 212); // #cdd1d4
pub const TX_2: Color32 = Color32::from_rgb(138, 144, 150); // #8a9096
pub const TX_3: Color32 = Color32::from_rgb(91, 97, 103); // #5b6167

pub const ACC: Color32 = Color32::from_rgb(14, 138, 176); // #0e8ab0
pub const ACC_2: Color32 = Color32::from_rgb(20, 168, 212); // #14a8d4
pub const PRI_TX: Color32 = Color32::from_rgb(196, 236, 248); // #c4ecf8
pub const PRI_BORDER: Color32 = Color32::from_rgb(14, 109, 139); // #0e6d8b

pub const WARN: Color32 = Color32::from_rgb(214, 162, 74); // #d6a24a
pub const OK: Color32 = Color32::from_rgb(95, 174, 109); // #5fae6d

// Kolory operacji (KC z makiety + rozróżnienie przelotu)
pub const COL_VDRILL: Color32 = Color32::from_rgb(79, 159, 208); // #4f9fd0 (wiercenie pionowe góra / dół)
pub const COL_VDRILL_THRU: Color32 = Color32::from_rgb(62, 207, 142); // #3ecf8e (wiercenie przelotowe góra+dół)
pub const COL_SDRILL: Color32 = Color32::from_rgb(214, 176, 74); // #d6b04a
pub const COL_LAMELLO: Color32 = Color32::from_rgb(148, 134, 214); // #9486d6 (nuta w krawędzi — delikatnie w granat/indygo)
pub const COL_LAMELLO_FACE: Color32 = Color32::from_rgb(168, 146, 228); // #a892e4 (gniazdo w powierzchni — delikatnie w granat/indygo)
pub const COL_LAMELLO_KEY: Color32 = Color32::from_rgb(236, 210, 247); // #ecd2f7 (otwór pod klucz Clamex)
pub const COL_GROOVE: Color32 = Color32::from_rgb(212, 104, 95); // #d4685f
pub const COL_EDGE: Color32 = Color32::from_rgb(207, 212, 216); // #cfd4d8

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerKind {
    VDrill,
    SDrill,
    Lamello,
    Groove,
    Edge,
}

impl LayerKind {
    pub const ALL: [LayerKind; 5] = [
        LayerKind::VDrill,
        LayerKind::SDrill,
        LayerKind::Lamello,
        LayerKind::Groove,
        LayerKind::Edge,
    ];

    pub fn color(self) -> Color32 {
        match self {
            Self::VDrill => COL_VDRILL,
            Self::SDrill => COL_SDRILL,
            Self::Lamello => COL_LAMELLO,
            Self::Groove => COL_GROOVE,
            Self::Edge => COL_EDGE,
        }
    }

    pub fn full_name(self) -> &'static str {
        match self {
            Self::VDrill => "Wiercenie pionowe",
            Self::SDrill => "Wiercenie boczne",
            Self::Lamello => "Lamello P14",
            Self::Groove => "Nuta piłą w osi X",
            Self::Edge => "Obrzeże CSV",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::VDrill => "WIERC. PION.",
            Self::SDrill => "WIERC. BOK.",
            Self::Lamello => "LAMELLO P14",
            Self::Groove => "NUTA PIŁA X",
            Self::Edge => "OBRZEŻE",
        }
    }

    pub fn of_op(op: &Operation) -> Self {
        match op {
            Operation::VerticalDrill { .. } => Self::VDrill,
            Operation::HorizontalDrill { .. } => Self::SDrill,
            Operation::LamelloP14 { .. } => Self::Lamello,
            Operation::Groove { .. } => Self::Groove,
        }
    }
}

pub fn face_short_label(face: u8) -> &'static str {
    // Zgodne z widokiem maszyny (baza 0,0 w prawym górnym narożniku przy bolcu)
    match face {
        1 => "F1 DÓŁ",
        2 => "F2 GÓRA",
        3 => "F3 LEWO",
        4 => "F4 PRAWO",
        5 => "F5 GÓRA Z+",
        6 => "F6 DÓŁ Z-",
        _ => "F?",
    }
}

pub fn edge_face_label(face: u8) -> &'static str {
    // Zweryfikowane mapowanie <EdgeGroup> w NangXinPCDrillCAM
    match face {
        1 => "F1 Dół",
        2 => "F2 Góra",
        3 => "F3 Lewo",
        4 => "F4 Prawo",
        _ => "F?",
    }
}

pub fn configure_industrial_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_1;
    visuals.window_fill = BG_1;
    visuals.extreme_bg_color = BG_0;
    visuals.faint_bg_color = BG_2;
    visuals.override_text_color = Some(TX);
    visuals.selection.bg_fill = BG_PRI;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACC_2);
    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;
    visuals.window_stroke = Stroke::new(1.0_f32, LINE_2);

    visuals.widgets.noninteractive.bg_fill = BG_1;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TX);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::ZERO;

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(20, 20, 20);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE_2);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TX);
    visuals.widgets.inactive.corner_radius = CornerRadius::ZERO;

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(31, 31, 31);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACC);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TX);
    visuals.widgets.hovered.corner_radius = CornerRadius::ZERO;

    visuals.widgets.active.bg_fill = BG_PRI;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACC_2);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PRI_TX);
    visuals.widgets.active.corner_radius = CornerRadius::ZERO;

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0_f32, 4.0_f32);
    style.spacing.button_padding = egui::vec2(9.0_f32, 4.0_f32);
    style.spacing.window_margin = Margin::same(0);
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(12.0_f32, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(11.5_f32, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(11.0_f32, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(10.0_f32, egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}
