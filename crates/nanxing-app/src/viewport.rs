use crate::theme::*;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use nanxing_core::clamps::{compute_clamps, op_bbox_mm};
use nanxing_core::config::AppConfig;
use nanxing_core::format_num;
use nanxing_core::lamello_p14::{self, Edge, P14, Pt};
use nanxing_core::model::{Operation, PanelProgram};
use nanxing_core::transforms::PanelTransform;
use std::collections::HashSet;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportAction {
    None,
    Transform(PanelTransform),
    ResetTransforms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    XRay,
    TopF5,
    BottomF6,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::XRay => "X-Ray",
            Self::TopF5 => "Góra",
            Self::BottomF6 => "Dół",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Op(usize),
    Edge(u8),
}

pub struct ViewportState {
    pub zoom: f32,
    pub pan: Vec2,
    pub view_mode: ViewMode,
    pub show_grid: bool,
    pub cursor_mm: Option<(f64, f64)>,
    pub hovered_op: Option<usize>,
    pub hovered_edge: Option<u8>,
    pub hidden_layers: HashSet<LayerKind>,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            view_mode: ViewMode::XRay,
            show_grid: true,
            cursor_mm: None,
            hovered_op: None,
            hovered_edge: None,
            hidden_layers: HashSet::new(),
        }
    }
}

impl ViewportState {
    pub fn reset_camera(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    pub fn is_layer_visible(&self, kind: LayerKind) -> bool {
        !self.hidden_layers.contains(&kind)
    }

    pub fn toggle_layer(&mut self, kind: LayerKind) {
        if !self.hidden_layers.remove(&kind) {
            self.hidden_layers.insert(kind);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_cad_viewport(
    ui: &mut egui::Ui,
    panel: &PanelProgram,
    ghost_ops: &[Operation],
    cfg: &AppConfig,
    vp: &mut ViewportState,
    selection: &mut Option<Selection>,
    external_hover_edge: Option<u8>,
    has_user_transforms: bool,
    manual_skip: bool,
    is_upside_down_side: bool,
    is_second_pass: bool,
) -> ViewportAction {
    let available = ui.available_rect_before_wrap();
    let (response, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0_f32, BG_CANVAS);

    if response.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll.abs() > 0.1 {
            let factor = if scroll > 0.0 { 1.12_f32 } else { 1.0 / 1.12_f32 };
            vp.zoom = (vp.zoom * factor).clamp(0.45, 8.0);
        }
    }
    if response.dragged() {
        vp.pan += response.drag_delta();
    }

    let pad_l = 74.0_f32;
    let pad_r = 74.0_f32;
    let pad_t = 68.0_f32;
    let pad_b = 72.0_f32;
    let ex_t = 68.0_f32;
    let ex_r = (cfg.pin_dx_mm as f32 + cfg.pin_dia_mm as f32 * 0.5).max(20.0) + 12.0;

    let pl = panel.length.max(1.0) as f32;
    let pw = panel.width.max(1.0) as f32;

    let usable_w = (rect.width() - pad_l - pad_r).max(80.0);
    let usable_h = (rect.height() - pad_t - pad_b).max(80.0);
    let base_scale = (usable_w / (pl + ex_r)).min(usable_h / (pw + ex_t)).max(0.02);
    let scale = base_scale * vp.zoom;

    let cw_px = (pl + ex_r) * scale;
    let ch_px = (pw + ex_t) * scale;
    // Punkt zerowy (0,0) w prawym górnym narożniku formatki (zgodnie z NangXinPCDrillCAM przy MachiningPoint="1"):
    // X rośnie od prawej (0) w lewo (Length), Y rośnie od góry (0) w dół (Width).
    let ox_left = rect.left() + pad_l + (usable_w - cw_px) * 0.5 + vp.pan.x;
    let ox_right = ox_left + pl * scale;
    let oy_top = rect.top() + pad_t + (usable_h - ch_px) * 0.5 + ex_t * scale + vp.pan.y;
    let oy_bottom = oy_top + pw * scale;

    let sx = |x_mm: f64| -> f32 { ox_right - (x_mm as f32) * scale };
    let sy = |y_mm: f64| -> f32 { oy_top + (y_mm as f32) * scale };

    let pointer_pos = response.hover_pos();
    if let Some(pointer) = pointer_pos {
        let mx = ((ox_right - pointer.x) / scale) as f64;
        let my = ((pointer.y - oy_top) / scale) as f64;
        vp.cursor_mm = Some((mx, my));
    } else {
        vp.cursor_mm = None;
    }

    // 1. Siatka zakotwiczona w punkcie (0,0) w prawym górnym narożniku
    if vp.show_grid {
        draw_metric_grid(&painter, rect, ox_right, oy_top, scale, cfg.grid_step_mm as f32);
    }

    let board_rect = Rect::from_min_max(
        Pos2::new(ox_left, oy_top),
        Pos2::new(ox_right, oy_bottom),
    );

    // 2. Realny kontur <Outline> formatki (albo prostokąt L×W) — subtelne, czytelne rozróżnienie stron bez przysłaniania otworów
    let is_back_view = vp.view_mode == ViewMode::BottomF6;
    let (board_fill, board_stroke) = match (is_upside_down_side, is_back_view) {
        (false, false) => (
            Color32::from_rgb(22, 25, 28),
            Stroke::new(1.1_f32, Color32::from_rgb(64, 72, 80)),
        ),
        (true, false) => (
            // Odwrócona formatka do góry nogami (lub Program 2 _STR2): stonowany ciepły brąz + delikatna bursztynowa ramka
            Color32::from_rgb(30, 24, 19),
            Stroke::new(1.5_f32, Color32::from_rgb(178, 122, 52)),
        ),
        (false, true) => (
            // Widok od tyłu / spodu (klawisz 3) dla strony głównej: stonowany ciemny granat-indygo
            Color32::from_rgb(21, 22, 34),
            Stroke::new(1.5_f32, Color32::from_rgb(114, 108, 196)),
        ),
        (true, true) => (
            // Widok od tyłu na formatce obróconej do góry nogami: stonowany ciemny morski
            Color32::from_rgb(18, 29, 30),
            Stroke::new(1.5_f32, Color32::from_rgb(62, 164, 148)),
        ),
    };

    if panel.outline.len() >= 3 {
        let pts: Vec<Pos2> = panel.outline.iter().map(|p| Pos2::new(sx(p.x), sy(p.y))).collect();
        painter.add(Shape::convex_polygon(pts.clone(), board_fill, Stroke::NONE));
        let mut closed = pts;
        closed.push(closed[0]);
        painter.add(Shape::line(closed, board_stroke));
    } else {
        painter.rect_filled(board_rect, 0.0_f32, board_fill);
        painter.rect_stroke(board_rect, 0.0_f32, board_stroke, StrokeKind::Outside);
    }

    // Subtelny deseń odróżniający stronę odwróconą (+45°) oraz podgląd od tyłu (-45°) — niska intensywność, nie przysłania otworów
    let clipped_board = painter.with_clip_rect(board_rect.intersect(rect));
    if is_upside_down_side {
        let hatch_col = if is_back_view {
            Color32::from_rgba_unmultiplied(68, 192, 172, 10)
        } else {
            Color32::from_rgba_unmultiplied(214, 146, 56, 11)
        };
        draw_diagonal_hatch(&clipped_board, board_rect, 26.0, hatch_col);
    }
    if is_back_view {
        let rev_col = if is_upside_down_side {
            Color32::from_rgba_unmultiplied(214, 152, 62, 10)
        } else {
            Color32::from_rgba_unmultiplied(136, 128, 222, 12)
        };
        draw_reverse_diagonal_hatch(&clipped_board, board_rect, 26.0, rev_col);
    }
    if is_upside_down_side || is_back_view {
        let inner_r = board_rect.shrink(4.0);
        if inner_r.width() > 16.0 && inner_r.height() > 16.0 {
            let bc = board_stroke.color;
            let dash_col = Color32::from_rgba_unmultiplied(bc.r(), bc.g(), bc.b(), 52);
            let corners = [
                inner_r.left_top(),
                inner_r.right_top(),
                inner_r.right_bottom(),
                inner_r.left_bottom(),
            ];
            draw_dashed_closed_poly(&painter, &corners, 7.0, 6.0, Stroke::new(1.0_f32, dash_col));
        }
    }

    if panel.operations.is_empty() && ghost_ops.is_empty() {
        draw_diagonal_hatch(&painter, board_rect, 12.0, Color32::from_rgb(44, 49, 53));
        let txt = if cfg.skip_empty_panels {
            "FORMATKA PUSTA — POMINIĘTA PRZY EKSPORCIE"
        } else {
            "FORMATKA PUSTA"
        };
        painter.text(
            board_rect.center(),
            Align2::CENTER_CENTER,
            txt,
            FontId::monospace(12.5_f32),
            Color32::from_rgb(122, 130, 136),
        );
    }

    // 3. Obrzeża <EdgeGroup> (zweryfikowane mapowanie: 1=Dół, 2=Góra, 3=Lewo, 4=Prawo)
    vp.hovered_edge = None;
    let clicked = response.clicked();
    let edge_defs = [
        (
            1u8,
            Pos2::new(ox_left, oy_bottom),
            Pos2::new(ox_right, oy_bottom),
            Vec2::new(0.0, 13.0),
        ),
        (
            2u8,
            Pos2::new(ox_left, oy_top),
            Pos2::new(ox_right, oy_top),
            Vec2::new(0.0, -13.0),
        ),
        (
            3u8,
            Pos2::new(ox_left, oy_top),
            Pos2::new(ox_left, oy_bottom),
            Vec2::new(-16.0, 0.0),
        ),
        (
            4u8,
            Pos2::new(ox_right, oy_top),
            Pos2::new(ox_right, oy_bottom),
            Vec2::new(16.0, 0.0),
        ),
    ];

    for (face, p1, p2, label_off) in edge_defs {
        let th = panel.edges.get(face);
        let code = panel.edges.code_label(face);
        let is_sel = *selection == Some(Selection::Edge(face));
        let edge_hit = Rect::from_two_pos(p1, p2).expand(7.0);
        if let Some(pos) = pointer_pos {
            if edge_hit.contains(pos) {
                vp.hovered_edge = Some(face);
            }
        }
        let is_hov = vp.hovered_edge == Some(face) || external_hover_edge == Some(face);

        let sw = if th <= 0.01 {
            1.0_f32
        } else if (th - 0.4).abs() < 0.1 {
            2.5_f32
        } else if (th - 0.8).abs() < 0.15 {
            3.5_f32
        } else {
            5.0_f32
        };

        if is_sel || is_hov {
            painter.line_segment(
                [p1, p2],
                Stroke::new(
                    sw + 5.0_f32,
                    Color32::from_rgba_unmultiplied(ACC_2.r(), ACC_2.g(), ACC_2.b(), 140),
                ),
            );
        }
        if th > 0.01 && vp.is_layer_visible(LayerKind::Edge) {
            painter.line_segment([p1, p2], Stroke::new(sw, COL_EDGE));
        }

        let mid = Pos2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5) + label_off;
        let lbl = if th > 0.01 {
            format!("F{face} {code}")
        } else {
            format!("F{face}")
        };
        painter.text(
            mid,
            Align2::CENTER_CENTER,
            lbl,
            FontId::monospace(10.0_f32),
            if th > 0.01 {
                Color32::from_rgb(174, 180, 184)
            } else {
                TX_3
            },
        );
    }

    // 4. Wymiary X / Y
    draw_dimensions(&painter, panel, board_rect);

    // 4b. Półprzezroczyste duchy operacji z drugiej strony / drugiego programu (widoczne w X-Ray oraz w podglądzie Dół F6)
    if vp.view_mode != ViewMode::TopF5 {
        for gop in ghost_ops {
            let gkind = LayerKind::of_op(gop);
            if !vp.is_layer_visible(gkind) {
                continue;
            }
            draw_ghost_operation(&painter, gop, panel, scale, &sx, &sy);
        }
    }

    // 5. Wykryte operacje B-Rep (wiercenia, Lamello P14, nuty)
    vp.hovered_op = None;
    for (idx, op) in panel.operations.iter().enumerate() {
        let kind = LayerKind::of_op(op);
        if !vp.is_layer_visible(kind) {
            continue;
        }
        let op_face = match op {
            Operation::VerticalDrill { face, .. }
            | Operation::HorizontalDrill { face, .. }
            | Operation::LamelloP14 { face, .. }
            | Operation::Groove { face, .. } => *face,
        };
        if vp.view_mode == ViewMode::TopF5 && op_face == 6 {
            continue;
        }
        if vp.view_mode == ViewMode::BottomF6 && op_face == 5 {
            continue;
        }

        let is_sel = *selection == Some(Selection::Op(idx));
        let col = kind.color();
        let is_ghost = vp.view_mode == ViewMode::XRay && op_face == 6;

        let hit_rect = match op {
            Operation::VerticalDrill {
                face,
                x,
                y,
                diameter,
                is_through,
                ..
            } => {
                let c = Pos2::new(sx(*x), sy(*y));
                let r = (((*diameter as f32) * 0.5) * scale).max(1.8_f32);
                let is_lamello_imbus = !*is_through
                    && *face == 5
                    && is_edge_lamello_key_hole(panel, *x, *y, *diameter);

                if is_lamello_imbus {
                    // Otwór pod klucz Clamex z lamello_p14_spec.md (#0a0a0a fill, #ecd2f7 stroke, kropka r=1.4px, bez linii krzyżyka)
                    painter.circle(
                        c,
                        r.max(2.4_f32),
                        Color32::from_rgb(10, 10, 10),
                        Stroke::new(if is_sel { 2.0_f32 } else { 1.1_f32 }, COL_LAMELLO_KEY),
                    );
                    painter.circle_filled(c, 1.4_f32, COL_LAMELLO_KEY);
                    Rect::from_center_size(c, Vec2::splat((r + 5.0) * 2.0))
                } else if *is_through {
                    // Otwór przelotowy (wiertło z góry do połowy + z dołu do połowy):
                    // odmienny kolor + pełne koło średnicy otworu + nieco większy pierścień dookoła
                    let fill_thru = Color32::from_rgba_unmultiplied(
                        COL_VDRILL_THRU.r(),
                        COL_VDRILL_THRU.g(),
                        COL_VDRILL_THRU.b(),
                        230,
                    );
                    painter.circle_filled(c, r, fill_thru);
                    let r_ring = r + (2.8_f32).max(r * 0.36);
                    painter.circle_stroke(
                        c,
                        r_ring,
                        Stroke::new(if is_sel { 2.1_f32 } else { 1.4_f32 }, COL_VDRILL_THRU),
                    );
                    Rect::from_center_size(c, Vec2::splat((r_ring + 4.0) * 2.0))
                } else if *face == 6 {
                    // Otwór nieprzelotowy robiony od dołu formatki (Face 6): sam pierścień w kolorze otworów
                    let ring_w = (1.6_f32).min((r * 0.6).max(1.1_f32));
                    painter.circle_stroke(
                        c,
                        r,
                        Stroke::new(if is_sel { 2.2_f32 } else { ring_w }, COL_VDRILL),
                    );
                    Rect::from_center_size(c, Vec2::splat((r + 5.0) * 2.0))
                } else {
                    // Otwór nieprzelotowy z góry (Face 5): tylko pełne kółko średnicy otworu (bez krzyżyka)
                    let fill_top = Color32::from_rgba_unmultiplied(
                        COL_VDRILL.r(),
                        COL_VDRILL.g(),
                        COL_VDRILL.b(),
                        225,
                    );
                    painter.circle_filled(c, r, fill_top);
                    if is_sel {
                        painter.circle_stroke(c, r, Stroke::new(1.8_f32, Color32::WHITE));
                    }
                    Rect::from_center_size(c, Vec2::splat((r + 5.0) * 2.0))
                }
            }
            Operation::HorizontalDrill {
                face,
                x,
                y,
                diameter,
                depth,
                ..
            } => {
                // Otwór boczny: czysty prostokątny kanał o średnicy otworu — bez kółka na krawędzi i bez linii środkowej
                let p1 = Pos2::new(sx(*x), sy(*y));
                let p2 = match face {
                    4 => Pos2::new(sx(*x + depth), sy(*y)),
                    3 => Pos2::new(sx(*x - depth), sy(*y)),
                    2 => Pos2::new(sx(*x), sy(*y + depth)),
                    1 => Pos2::new(sx(*x), sy(*y - depth)),
                    _ => p1,
                };
                let half_w = ((*diameter as f32) * 0.5 * scale).max(1.4_f32);
                let bar_rect = if (p1.y - p2.y).abs() < 0.5 {
                    Rect::from_min_max(
                        Pos2::new(p1.x.min(p2.x), p1.y - half_w),
                        Pos2::new(p1.x.max(p2.x), p1.y + half_w),
                    )
                } else {
                    Rect::from_min_max(
                        Pos2::new(p1.x - half_w, p1.y.min(p2.y)),
                        Pos2::new(p1.x + half_w, p1.y.max(p2.y)),
                    )
                };
                painter.rect_filled(
                    bar_rect,
                    0.0_f32,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 105),
                );
                painter.rect_stroke(
                    bar_rect,
                    0.0_f32,
                    Stroke::new(if is_sel { 1.8_f32 } else { 1.0_f32 }, col),
                    StrokeKind::Inside,
                );
                bar_rect.expand(4.0)
            }
            Operation::LamelloP14 {
                face,
                direction,
                x,
                y,
                max_depth,
                ..
            } => {
                let mut p14 = P14::default();
                let stroke_w = if is_sel { 2.0_f32 } else { 1.0_f32 };
                if (1..=4).contains(face) {
                    // Nuta w krawędzi — odcinek koła (21 pkt, cięciwa 69,56 mm przy h=14 mm), kolor #a98bd4
                    p14.edge_depth = *max_depth;
                    let (edge, s0) = match face {
                        1 => (Edge::F3, *x), // Tył (Y = W)
                        2 => (Edge::F1, *x), // Przód (Y = 0)
                        3 => (Edge::F2, *y), // Prawo (X = L)
                        _ => (Edge::F4, *y), // Lewo (X = 0)
                    };
                    let raw_pts = lamello_p14::edge_slot(&p14, edge, panel.length, panel.width, s0, 20);
                    let screen_pts: Vec<Pos2> =
                        raw_pts.iter().map(|q| Pos2::new(sx(q[0]), sy(q[1]))).collect();
                    let fill_col = Color32::from_rgba_unmultiplied(
                        COL_LAMELLO.r(),
                        COL_LAMELLO.g(),
                        COL_LAMELLO.b(),
                        51,
                    );
                    painter.add(Shape::convex_polygon(
                        screen_pts.clone(),
                        fill_col,
                        Stroke::NONE,
                    ));
                    painter.add(Shape::closed_line(
                        screen_pts.clone(),
                        Stroke::new(stroke_w, COL_LAMELLO),
                    ));
                    screen_bounds(&screen_pts).expand(4.0)
                } else {
                    // Gniazdo w powierzchni — kształt „kości” osadzony idealnie na środku (x, y)
                    let axis_along_u = *direction == 'X';
                    let fill_col = if is_ghost {
                        Color32::from_rgba_unmultiplied(122, 118, 206, 8)
                    } else {
                        Color32::from_rgba_unmultiplied(
                            COL_LAMELLO_FACE.r(),
                            COL_LAMELLO_FACE.g(),
                            COL_LAMELLO_FACE.b(),
                            52,
                        )
                    };
                    for part in face_socket_convex_parts(&p14, *x, *y, axis_along_u) {
                        let pts: Vec<Pos2> =
                            part.iter().map(|q| Pos2::new(sx(q[0]), sy(q[1]))).collect();
                        painter.add(Shape::convex_polygon(pts, fill_col, Stroke::NONE));
                    }
                    let outline_pts: Vec<Pos2> = lamello_p14::face_socket(&p14, *x, *y, axis_along_u)
                        .iter()
                        .map(|q| Pos2::new(sx(q[0]), sy(q[1])))
                        .collect();
                    if is_ghost {
                        let ghost_stroke = Color32::from_rgba_unmultiplied(126, 122, 210, 48);
                        draw_dashed_closed_poly(
                            &painter,
                            &outline_pts,
                            4.5,
                            4.5,
                            Stroke::new(0.95_f32, ghost_stroke),
                        );
                    } else {
                        painter.add(Shape::closed_line(
                            outline_pts.clone(),
                            Stroke::new(stroke_w, COL_LAMELLO_FACE),
                        ));
                    }
                    screen_bounds(&outline_pts).expand(4.0)
                }
            }
            Operation::Groove {
                x,
                y,
                end_x,
                end_y,
                width,
                ..
            } => {
                let p1 = Pos2::new(sx(*x), sy(*y));
                let p2 = Pos2::new(sx(*end_x), sy(*end_y));
                let gw = ((*width as f32) * scale).max(3.5);
                let aligned_x = (end_y - y).abs() < 0.05;
                let gcol = if aligned_x { col } else { WARN };
                painter.line_segment(
                    [p1, p2],
                    Stroke::new(
                        gw,
                        Color32::from_rgba_unmultiplied(gcol.r(), gcol.g(), gcol.b(), 125),
                    ),
                );
                painter.line_segment([p1, p2], Stroke::new(1.1_f32, gcol));
                Rect::from_two_pos(p1, p2).expand(gw * 0.5 + 4.0)
            }
        };

        if is_sel {
            let (bx0, by0, bx1, by1) = op_bbox_mm(op, panel);
            let sel_rect =
                Rect::from_two_pos(Pos2::new(sx(bx0), sy(by0)), Pos2::new(sx(bx1), sy(by1)))
                    .expand(6.0);
            painter.rect_stroke(
                sel_rect,
                0.0_f32,
                Stroke::new(1.1_f32, Color32::WHITE),
                StrokeKind::Outside,
            );
            painter.text(
                Pos2::new(sel_rect.left(), sel_rect.top() - 4.0),
                Align2::LEFT_BOTTOM,
                format!("N{:03} · {}", (idx + 1) * 10, kind.short_name()),
                FontId::monospace(10.0_f32),
                Color32::WHITE,
            );
        }

        if let Some(pos) = pointer_pos {
            if hit_rect.contains(pos) {
                vp.hovered_op = Some(idx);
            }
        }
    }

    // Obszar pływających przycisków w prawym górnym (obracanie/odbijanie) i prawym dolnym rogu (zoom/siatka)
    const CTRL_MARGIN: f32 = 14.0;
    const CTRL_BTN: f32 = 28.0;
    let orient_bar_rect = Rect::from_min_size(
        Pos2::new(
            rect.right() - CTRL_MARGIN - CTRL_BTN,
            rect.top() + CTRL_MARGIN,
        ),
        Vec2::new(CTRL_BTN, CTRL_BTN * 6.0),
    );
    let ctrl_bar_rect = Rect::from_min_size(
        Pos2::new(
            rect.right() - CTRL_MARGIN - CTRL_BTN,
            rect.bottom() - CTRL_MARGIN - CTRL_BTN * 4.0,
        ),
        Vec2::new(CTRL_BTN, CTRL_BTN * 4.0),
    );
    let pointer_on_ctrl = pointer_pos
        .is_some_and(|p| ctrl_bar_rect.contains(p) || orient_bar_rect.contains(p));

    if clicked && !pointer_on_ctrl {
        if let Some(op_idx) = vp.hovered_op {
            *selection = Some(Selection::Op(op_idx));
        } else if let Some(edge_face) = vp.hovered_edge {
            *selection = Some(Selection::Edge(edge_face));
        } else {
            *selection = None;
        }
    }

    // 6. Łapy zaciskowe — stała proporcja wysokości i szerokości, odsunięte powyżej formatki i napisu F2
    let clamps = compute_clamps(panel, cfg);
    for (i, c) in clamps.iter().enumerate() {
        let raw_rect =
            Rect::from_two_pos(Pos2::new(sx(c.x0), sy(c.y0)), Pos2::new(sx(c.x1), sy(c.y1)));
        // Dopilnuj minimalnego odstępu 22 px ponad górną krawędzią formatki (aby nigdy nie najść na F2 ani otwory)
        let min_clear_bottom = oy_top - 22.0_f32;
        let shift_up = (raw_rect.bottom() - min_clear_bottom).max(0.0_f32);
        let crect = raw_rect.translate(Vec2::new(0.0_f32, -shift_up));

        let lip_h = (crect.height() * 0.16_f32).clamp(2.0_f32, 6.0_f32);
        let lip_rect = Rect::from_min_max(
            Pos2::new(crect.left(), crect.bottom() - lip_h),
            crect.right_bottom(),
        );
        let fill = Color32::from_rgba_unmultiplied(35, 39, 42, 238);
        let stroke = Color32::from_rgb(90, 99, 106);
        let lip_col = Color32::from_rgb(58, 65, 72);
        painter.rect_filled(crect, 0.0_f32, fill);
        painter.rect_filled(lip_rect, 0.0_f32, lip_col);
        painter.rect_stroke(crect, 0.0_f32, Stroke::new(1.0_f32, stroke), StrokeKind::Inside);

        let body_h = (crect.height() - lip_h).max(0.0_f32);
        if crect.width() >= 28.0 && body_h >= 10.0 {
            let fsz = (crect.width() * 0.115).clamp(7.0_f32, 12.5_f32);
            painter.text(
                Pos2::new(crect.center().x, crect.top() + body_h * 0.5),
                Align2::CENTER_CENTER,
                format!("Łapa {}", i + 1),
                FontId::monospace(fsz),
                TX_2,
            );
        }
    }

    // 7. Bolec pozycjonujący — czerwono-różowy okrąg, skalowany proporcjonalnie, bez linii środkowych i bez podpisu
    let pin_center = Pos2::new(
        ox_right + (cfg.pin_dx_mm as f32) * scale,
        oy_top + (cfg.pin_dy_mm.min(panel.width * 0.5) as f32) * scale,
    );
    let pin_r = (((cfg.pin_dia_mm * 0.5) as f32) * scale).max(2.0_f32);
    painter.circle(
        pin_center,
        pin_r,
        Color32::from_rgba_unmultiplied(242, 134, 156, 215),
        Stroke::new(1.3_f32, Color32::from_rgb(224, 66, 96)),
    );

    // 8. Punkt zerowy (0,0) w prawym górnym narożniku formatki
    draw_origin_gizmo(&painter, Pos2::new(ox_right, oy_top));

    // 9. Nakładka lewa górna
    let ovl_pos = rect.left_top() + Vec2::new(12.0, 10.0);
    painter.text(
        ovl_pos,
        Align2::LEFT_TOP,
        format!("{}  [ID: {}]", panel.name, panel.id),
        FontId::monospace(11.5_f32),
        TX,
    );
    let flip_tag = if panel.flipped_top_bottom {
        " (auto-obrót 180° Y)"
    } else {
        ""
    };
    painter.text(
        ovl_pos + Vec2::new(0.0, 16.0),
        Align2::LEFT_TOP,
        format!(
            "{} × {} × {} mm  ·  Widok: {}{}",
            format_num(panel.length),
            format_num(panel.width),
            format_num(panel.thickness),
            vp.view_mode.label(),
            flip_tag
        ),
        FontId::monospace(10.5_f32),
        TX_2,
    );
    let _ = is_second_pass;
    if manual_skip {
        painter.text(
            ovl_pos + Vec2::new(0.0, 32.0),
            Align2::LEFT_TOP,
            "POMIJANA PRZY EKSPORCIE",
            FontId::monospace(10.5_f32),
            WARN,
        );
    }

    // 11a. Pływające przyciski obracania i odbijania w prawym górnym rogu wzdłuż prawej krawędzi
    let action = draw_floating_orient_controls(ui, &painter, orient_bar_rect, has_user_transforms);

    // 11b. Pływające przyciski w prawym dolnym rogu (identyczny odstęp 14 px od prawej krawędzi i od dolnej legendy):
    //      [1] Przybliż (+)   [2] Oddal (−)   [3] Centruj widok (celownik narożny)   [4] Siatka (kratka z 4 linii)
    draw_floating_viewport_controls(ui, &painter, ctrl_bar_rect, vp);

    // 12. Tooltip
    if !pointer_on_ctrl {
        if let Some(pos) = pointer_pos {
            if let Some(op_idx) = vp.hovered_op {
                if let Some(op) = panel.operations.get(op_idx) {
                    let tip = format!("N{:03} · {}", (op_idx + 1) * 10, op);
                    draw_tooltip(&painter, pos + Vec2::new(14.0, 14.0), &tip);
                }
            } else if let Some(edge_face) = vp.hovered_edge {
                let code = panel.edges.code_label(edge_face);
                let tip = format!("{}\n{}", edge_face_label(edge_face), code);
                draw_tooltip(&painter, pos + Vec2::new(14.0, 14.0), &tip);
            }
        }
    }

    action
}

fn draw_floating_orient_controls(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    bar_rect: Rect,
    has_user_transforms: bool,
) -> ViewportAction {
    let mut action = ViewportAction::None;
    let btn_w = bar_rect.width();
    let btn_h = bar_rect.height() / 6.0_f32;

    painter.rect_filled(bar_rect, 0.0_f32, Color32::from_rgb(20, 20, 20));

    let items: [(Option<PanelTransform>, &str); 6] = [
        (
            Some(PanelTransform::Rotate90Plus),
            "Obróć o +90° (zgodnie z ruchem wskazówek zegara)",
        ),
        (
            Some(PanelTransform::Rotate90Minus),
            "Obróć o −90° (przeciwnie do ruchu wskazówek zegara)",
        ),
        (
            Some(PanelTransform::MirrorTopBottom),
            "Lustrzane odbicie Góra / Dół (zamienia krawędź przy łapach z przeciwną bez zmiany strony Z)",
        ),
        (
            Some(PanelTransform::MirrorLeftRight),
            "Lustrzane odbicie Prawo / Lewo (zamienia prawą i lewą krawędź bez zmiany strony Z)",
        ),
        (
            Some(PanelTransform::FlipUpsideDown),
            "Obróć formatkę do góry nogami (przewrót Góra↔Dół + obrót 180° — otwory przy łapach przechodzą na przeciwny bok jako wiercone od dołu)",
        ),
        (
            None,
            "Reset orientacji — przywróć domyślne ułożenie wyznaczone przez silnik",
        ),
    ];

    for (idx, (xf_opt, tip)) in items.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(bar_rect.left(), bar_rect.top() + (idx as f32) * btn_h),
            Vec2::new(btn_w, btn_h),
        );
        let is_reset = xf_opt.is_none();
        let enabled = !is_reset || has_user_transforms;

        let mut resp = ui.interact(r, egui::Id::new(("vp_orient_btn", idx)), Sense::click());
        if enabled {
            resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        resp = resp.on_hover_text(*tip);

        let down = enabled && resp.is_pointer_button_down_on();
        let hov = enabled && resp.hovered();

        let bg = if down {
            Color32::from_rgb(14, 58, 73)
        } else if hov {
            Color32::from_rgb(34, 40, 44)
        } else if is_reset && has_user_transforms {
            BG_SEL
        } else {
            Color32::from_rgb(20, 20, 20)
        };
        painter.rect_filled(r, 0.0_f32, bg);

        if idx > 0 {
            painter.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0_f32, LINE_2));
        }

        let icon_col = if !enabled {
            Color32::from_rgba_unmultiplied(TX_3.r(), TX_3.g(), TX_3.b(), 110)
        } else if down || hov {
            Color32::WHITE
        } else if is_reset && has_user_transforms {
            ACC_2
        } else {
            TX
        };
        let c = if down {
            r.center() + Vec2::new(0.0_f32, 1.0_f32)
        } else {
            r.center()
        };

        draw_orient_icon(painter, c, idx, icon_col);

        if enabled && resp.clicked() {
            action = match xf_opt {
                Some(xf) => ViewportAction::Transform(*xf),
                None => ViewportAction::ResetTransforms,
            };
        }
    }

    painter.rect_stroke(
        bar_rect,
        0.0_f32,
        Stroke::new(1.0_f32, LINE_2),
        StrokeKind::Inside,
    );

    action
}

fn draw_orient_icon(painter: &egui::Painter, c: Pos2, idx: usize, col: Color32) {
    let s = Stroke::new(1.35_f32, col);
    match idx {
        0 => {
            // +90° (łuk zgodnie z ruchem wskazówek zegara + grot + mały plus w środku)
            draw_arc_poly(painter, c, 6.4_f32, -2.35_f32, 1.35_f32, s);
            let tip = c + Vec2::new(6.4_f32 * 1.35_f32.cos(), 6.4_f32 * 1.35_f32.sin());
            painter.line_segment([tip, tip + Vec2::new(-1.2, -3.6)], s);
            painter.line_segment([tip, tip + Vec2::new(3.1, -2.0)], s);
            let ps = Stroke::new(1.15_f32, col);
            painter.line_segment([c - Vec2::new(2.2, 0.0), c + Vec2::new(2.2, 0.0)], ps);
            painter.line_segment([c - Vec2::new(0.0, 2.2), c + Vec2::new(0.0, 2.2)], ps);
        }
        1 => {
            // −90° (łuk przeciwnie do ruchu wskazówek zegara + grot + mały minus w środku)
            draw_arc_poly(painter, c, 6.4_f32, -0.79_f32, -4.49_f32, s);
            let a = -4.49_f32;
            let tip = c + Vec2::new(6.4_f32 * a.cos(), 6.4_f32 * a.sin());
            painter.line_segment([tip, tip + Vec2::new(1.2, -3.6)], s);
            painter.line_segment([tip, tip + Vec2::new(-3.1, -2.0)], s);
            let ms = Stroke::new(1.15_f32, col);
            painter.line_segment([c - Vec2::new(2.2, 0.0), c + Vec2::new(2.2, 0.0)], ms);
        }
        2 => {
            // Lustro Góra / Dół (pozioma oś przerywana + trójkąt górny i dolny)
            let axis_s = Stroke::new(1.1_f32, col);
            for dx in [-6.5_f32, -1.5_f32, 3.5_f32] {
                painter.line_segment(
                    [c + Vec2::new(dx, 0.0), c + Vec2::new(dx + 3.0, 0.0)],
                    axis_s,
                );
            }
            // Górny trójkąt
            painter.add(Shape::closed_line(
                vec![
                    c + Vec2::new(-4.5, -2.3),
                    c + Vec2::new(4.5, -2.3),
                    c + Vec2::new(0.0, -6.8),
                ],
                s,
            ));
            // Dolny trójkąt (wypełniony delikatnie dla czytelności odbicia)
            let bot_tri = vec![
                c + Vec2::new(-4.5, 2.3),
                c + Vec2::new(0.0, 6.8),
                c + Vec2::new(4.5, 2.3),
            ];
            painter.add(Shape::convex_polygon(
                bot_tri.clone(),
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 70),
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(bot_tri, s));
        }
        3 => {
            // Lustro Prawo / Lewo (pionowa oś przerywana + trójkąt lewy i prawy)
            let axis_s = Stroke::new(1.1_f32, col);
            for dy in [-6.5_f32, -1.5_f32, 3.5_f32] {
                painter.line_segment(
                    [c + Vec2::new(0.0, dy), c + Vec2::new(0.0, dy + 3.0)],
                    axis_s,
                );
            }
            // Lewy trójkąt
            painter.add(Shape::closed_line(
                vec![
                    c + Vec2::new(-2.3, -4.5),
                    c + Vec2::new(-2.3, 4.5),
                    c + Vec2::new(-6.8, 0.0),
                ],
                s,
            ));
            // Prawy trójkąt
            let right_tri = vec![
                c + Vec2::new(2.3, -4.5),
                c + Vec2::new(6.8, 0.0),
                c + Vec2::new(2.3, 4.5),
            ];
            painter.add(Shape::convex_polygon(
                right_tri.clone(),
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 70),
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(right_tri, s));
        }
        4 => {
            // Przewrót formatki do góry nogami (przekrój płyty w poziomie + łuk przewrotu góra↔dół)
            let plate = Rect::from_center_size(c, Vec2::new(11.0, 2.6));
            painter.rect_filled(
                plate,
                0.0_f32,
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 90),
            );
            painter.rect_stroke(plate, 0.0_f32, Stroke::new(1.0_f32, col), StrokeKind::Inside);
            // Półkolista strzałka przewrotu z góry na dół wokół osi poziomej
            draw_arc_poly(painter, c, 6.8_f32, -1.95_f32, 1.95_f32, s);
            let bot_tip = c + Vec2::new(6.8_f32 * 1.95_f32.cos(), 6.8_f32 * 1.95_f32.sin());
            painter.line_segment([bot_tip, bot_tip + Vec2::new(3.2, -0.5)], s);
            painter.line_segment([bot_tip, bot_tip + Vec2::new(1.1, -3.1)], s);
            let top_tip = c + Vec2::new(6.8_f32 * (-1.95_f32).cos(), 6.8_f32 * (-1.95_f32).sin());
            painter.line_segment([top_tip, top_tip + Vec2::new(3.2, 0.5)], s);
            painter.line_segment([top_tip, top_tip + Vec2::new(1.1, 3.1)], s);
        }
        _ => {
            // Reset orientacji (okrągła strzałka powrotu z kropką bazową w środku)
            draw_arc_poly(painter, c, 5.8_f32, -2.25_f32, 2.45_f32, s);
            let tip = c + Vec2::new(5.8_f32 * (-2.25_f32).cos(), 5.8_f32 * (-2.25_f32).sin());
            painter.line_segment([tip, tip + Vec2::new(3.3, -0.3)], s);
            painter.line_segment([tip, tip + Vec2::new(0.4, 3.3)], s);
            painter.circle_filled(c, 1.4_f32, col);
        }
    }
}

fn draw_arc_poly(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    a0: f32,
    a1: f32,
    stroke: Stroke,
) {
    let steps = 14usize;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = (i as f32) / (steps as f32);
            let a = a0 + (a1 - a0) * t;
            center + Vec2::new(radius * a.cos(), radius * a.sin())
        })
        .collect();
    painter.add(Shape::line(pts, stroke));
}

fn draw_floating_viewport_controls(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    bar_rect: Rect,
    vp: &mut ViewportState,
) {
    let btn_h = bar_rect.height() * 0.25_f32;
    let btn_w = bar_rect.width();

    painter.rect_filled(bar_rect, 0.0_f32, Color32::from_rgb(20, 20, 20));

    let tips = [
        "Przybliż (+)",
        "Oddal (−)",
        "Wycentruj i dopasuj widok (F)",
        "Pokaż / ukryj siatkę (G)",
    ];

    for (idx, tip) in tips.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(bar_rect.left(), bar_rect.top() + (idx as f32) * btn_h),
            Vec2::new(btn_w, btn_h),
        );
        let resp = ui
            .interact(r, egui::Id::new(("vp_float_btn", idx)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*tip);

        let down = resp.is_pointer_button_down_on();
        let hov = resp.hovered();
        let toggled_on = idx == 3 && vp.show_grid;

        let bg = if down {
            Color32::from_rgb(14, 58, 73)
        } else if hov {
            Color32::from_rgb(34, 40, 44)
        } else if toggled_on {
            BG_SEL
        } else {
            Color32::from_rgb(20, 20, 20)
        };
        painter.rect_filled(r, 0.0_f32, bg);

        if idx > 0 {
            painter.line_segment(
                [r.left_top(), r.right_top()],
                Stroke::new(1.0_f32, LINE_2),
            );
        }

        let icon_col = if down || hov {
            Color32::WHITE
        } else if toggled_on {
            ACC_2
        } else {
            TX
        };
        let c = if down {
            r.center() + Vec2::new(0.0_f32, 1.0_f32)
        } else {
            r.center()
        };
        let stroke = Stroke::new(1.4_f32, icon_col);

        match idx {
            0 => {
                // + (Przybliż)
                painter.line_segment([c - Vec2::new(5.0, 0.0), c + Vec2::new(5.0, 0.0)], stroke);
                painter.line_segment([c - Vec2::new(0.0, 5.0), c + Vec2::new(0.0, 5.0)], stroke);
            }
            1 => {
                // − (Oddal)
                painter.line_segment([c - Vec2::new(5.0, 0.0), c + Vec2::new(5.0, 0.0)], stroke);
            }
            2 => {
                // Ikona centrowania / dopasowania widoku (4 narożniki kadru + punkt centralny)
                let d = 6.0_f32;
                let a = 3.2_f32;
                let s = Stroke::new(1.3_f32, icon_col);
                // Lewy górny
                painter.line_segment(
                    [c + Vec2::new(-d, -d), c + Vec2::new(-d + a, -d)],
                    s,
                );
                painter.line_segment(
                    [c + Vec2::new(-d, -d), c + Vec2::new(-d, -d + a)],
                    s,
                );
                // Prawy górny
                painter.line_segment([c + Vec2::new(d, -d), c + Vec2::new(d - a, -d)], s);
                painter.line_segment([c + Vec2::new(d, -d), c + Vec2::new(d, -d + a)], s);
                // Lewy dolny
                painter.line_segment([c + Vec2::new(-d, d), c + Vec2::new(-d + a, d)], s);
                painter.line_segment([c + Vec2::new(-d, d), c + Vec2::new(-d, d - a)], s);
                // Prawy dolny
                painter.line_segment([c + Vec2::new(d, d), c + Vec2::new(d - a, d)], s);
                painter.line_segment([c + Vec2::new(d, d), c + Vec2::new(d, d - a)], s);
                // Środek kadru
                painter.circle_filled(c, 1.6_f32, icon_col);
            }
            _ => {
                // Kratka z 4 linii (2 pionowe + 2 poziome) do ukrywania / pokazywania siatki
                let ext = 6.5_f32;
                let off = 2.4_f32;
                let gs = Stroke::new(1.2_f32, icon_col);
                painter.line_segment(
                    [c + Vec2::new(-off, -ext), c + Vec2::new(-off, ext)],
                    gs,
                );
                painter.line_segment(
                    [c + Vec2::new(off, -ext), c + Vec2::new(off, ext)],
                    gs,
                );
                painter.line_segment(
                    [c + Vec2::new(-ext, -off), c + Vec2::new(ext, -off)],
                    gs,
                );
                painter.line_segment(
                    [c + Vec2::new(-ext, off), c + Vec2::new(ext, off)],
                    gs,
                );
            }
        }

        if resp.clicked() {
            match idx {
                0 => vp.zoom = (vp.zoom * 1.25).clamp(0.45, 8.0),
                1 => vp.zoom = (vp.zoom / 1.25).clamp(0.45, 8.0),
                2 => vp.reset_camera(),
                _ => vp.show_grid = !vp.show_grid,
            }
        }
    }

    painter.rect_stroke(
        bar_rect,
        0.0_f32,
        Stroke::new(1.0_f32, LINE_2),
        StrokeKind::Inside,
    );
}

fn draw_metric_grid(
    painter: &egui::Painter,
    rect: Rect,
    ox_right: f32,
    oy_top: f32,
    scale: f32,
    cfg_step_mm: f32,
) {
    let base_step = cfg_step_mm.clamp(10.0, 500.0);
    let step_mm = if base_step * scale < 14.0 {
        base_step * 5.0
    } else {
        base_step
    };
    let major_step = step_mm * 5.0;

    // x_mm rośnie w lewo od ox_right: sx = ox_right - x_mm * scale
    let x_min_mm = (ox_right - rect.right()) / scale;
    let x_max_mm = (ox_right - rect.left()) / scale;
    // y_mm rośnie w dół od oy_top: sy = oy_top + y_mm * scale
    let y_min_mm = (rect.top() - oy_top) / scale;
    let y_max_mm = (rect.bottom() - oy_top) / scale;

    let mut x = (x_min_mm / step_mm).ceil() * step_mm;
    while x <= x_max_mm {
        let sx = ox_right - x * scale;
        let is_major = (x / major_step - (x / major_step).round()).abs() < 0.01;
        let col = if is_major {
            Color32::from_rgb(30, 30, 30)
        } else {
            Color32::from_rgb(22, 22, 22)
        };
        painter.line_segment(
            [Pos2::new(sx, rect.top()), Pos2::new(sx, rect.bottom())],
            Stroke::new(1.0_f32, col),
        );
        x += step_mm;
    }

    let mut y = (y_min_mm / step_mm).ceil() * step_mm;
    while y <= y_max_mm {
        let sy = oy_top + y * scale;
        let is_major = (y / major_step - (y / major_step).round()).abs() < 0.01;
        let col = if is_major {
            Color32::from_rgb(30, 30, 30)
        } else {
            Color32::from_rgb(22, 22, 22)
        };
        painter.line_segment(
            [Pos2::new(rect.left(), sy), Pos2::new(rect.right(), sy)],
            Stroke::new(1.0_f32, col),
        );
        y += step_mm;
    }
}

fn draw_diagonal_hatch(painter: &egui::Painter, rect: Rect, step: f32, col: Color32) {
    let stroke = Stroke::new(1.0_f32, col);
    let mut d = -rect.height();
    while d < rect.width() {
        let x1 = (rect.left() + d).max(rect.left());
        let y1 = rect.top() + (x1 - (rect.left() + d));
        let y2 = rect.bottom().min(rect.top() + (rect.right() - (rect.left() + d)));
        let x2 = rect.left() + d + (y2 - rect.top());
        if x1 <= rect.right() && y1 <= rect.bottom() && x2 >= rect.left() && y2 >= rect.top() {
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], stroke);
        }
        d += step;
    }
}

fn draw_reverse_diagonal_hatch(painter: &egui::Painter, rect: Rect, step: f32, col: Color32) {
    let stroke = Stroke::new(1.0_f32, col);
    let mut d = 0.0_f32;
    let max_d = rect.width() + rect.height();
    while d < max_d {
        let x1 = (rect.left() + d).min(rect.right());
        let y1 = rect.top() + (d - (x1 - rect.left()));
        let y2 = rect.bottom().min(rect.top() + d);
        let x2 = rect.left() + (d - (y2 - rect.top()));
        if x2 <= rect.right() && y1 <= rect.bottom() && x1 >= rect.left() && y2 >= rect.top() {
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], stroke);
        }
        d += step;
    }
}

fn draw_dashed_segment(
    painter: &egui::Painter,
    a: Pos2,
    b: Pos2,
    dash_len: f32,
    gap_len: f32,
    stroke: Stroke,
) {
    let d = b - a;
    let total = d.length();
    if total < 0.5 {
        return;
    }
    let dir = d / total;
    let step = (dash_len + gap_len).max(2.0);
    let mut t = 0.0_f32;
    while t < total {
        let t_end = (t + dash_len).min(total);
        painter.line_segment([a + dir * t, a + dir * t_end], stroke);
        t += step;
    }
}

fn draw_dashed_closed_poly(
    painter: &egui::Painter,
    pts: &[Pos2],
    dash_len: f32,
    gap_len: f32,
    stroke: Stroke,
) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    for i in 0..n {
        draw_dashed_segment(painter, pts[i], pts[(i + 1) % n], dash_len, gap_len, stroke);
    }
}

/// Rysuje półprzezroczysty „duch” operacji znajdującej się po drugiej stronie formatki
/// (np. dolne gniazda Lamello w widoku strony głównej albo operacje 1. programu w podglądzie `_STR2`).
fn draw_ghost_operation(
    painter: &egui::Painter,
    op: &Operation,
    panel: &PanelProgram,
    scale: f32,
    sx: &dyn Fn(f64) -> f32,
    sy: &dyn Fn(f64) -> f32,
) {
    match op {
        Operation::LamelloP14 {
            face,
            direction,
            x,
            y,
            max_depth,
            ..
        } => {
            let mut p14 = P14::default();
            if (1..=4).contains(face) {
                p14.edge_depth = *max_depth;
                let (edge, s0) = match face {
                    1 => (Edge::F3, *x),
                    2 => (Edge::F1, *x),
                    3 => (Edge::F2, *y),
                    _ => (Edge::F4, *y),
                };
                let raw_pts = lamello_p14::edge_slot(&p14, edge, panel.length, panel.width, s0, 20);
                let screen_pts: Vec<Pos2> =
                    raw_pts.iter().map(|q| Pos2::new(sx(q[0]), sy(q[1]))).collect();
                let fill_col = Color32::from_rgba_unmultiplied(118, 114, 198, 7);
                let stroke_col = Color32::from_rgba_unmultiplied(124, 120, 206, 44);
                painter.add(Shape::convex_polygon(
                    screen_pts.clone(),
                    fill_col,
                    Stroke::NONE,
                ));
                draw_dashed_closed_poly(
                    painter,
                    &screen_pts,
                    4.5,
                    4.5,
                    Stroke::new(0.95_f32, stroke_col),
                );
            } else {
                let axis_along_u = *direction == 'X';
                let fill_col = Color32::from_rgba_unmultiplied(122, 118, 206, 7);
                let stroke_col = Color32::from_rgba_unmultiplied(126, 122, 210, 46);
                for part in face_socket_convex_parts(&p14, *x, *y, axis_along_u) {
                    let pts: Vec<Pos2> =
                        part.iter().map(|q| Pos2::new(sx(q[0]), sy(q[1]))).collect();
                    painter.add(Shape::convex_polygon(pts, fill_col, Stroke::NONE));
                }
                let outline_pts: Vec<Pos2> = lamello_p14::face_socket(&p14, *x, *y, axis_along_u)
                    .iter()
                    .map(|q| Pos2::new(sx(q[0]), sy(q[1])))
                    .collect();
                draw_dashed_closed_poly(
                    painter,
                    &outline_pts,
                    4.5,
                    4.5,
                    Stroke::new(0.95_f32, stroke_col),
                );
            }
        }
        Operation::VerticalDrill {
            x, y, diameter, ..
        } => {
            let c = Pos2::new(sx(*x), sy(*y));
            let r = (((*diameter as f32) * 0.5) * scale).max(1.8_f32);
            let col = Color32::from_rgba_unmultiplied(
                COL_VDRILL.r(),
                COL_VDRILL.g(),
                COL_VDRILL.b(),
                70,
            );
            painter.circle_stroke(c, r, Stroke::new(1.0_f32, col));
        }
        Operation::HorizontalDrill {
            face,
            x,
            y,
            diameter,
            depth,
            ..
        } => {
            let p1 = Pos2::new(sx(*x), sy(*y));
            let p2 = match face {
                4 => Pos2::new(sx(*x + depth), sy(*y)),
                3 => Pos2::new(sx(*x - depth), sy(*y)),
                2 => Pos2::new(sx(*x), sy(*y + depth)),
                1 => Pos2::new(sx(*x), sy(*y - depth)),
                _ => p1,
            };
            let half_w = ((*diameter as f32) * 0.5 * scale).max(1.4_f32);
            let r = if (p1.y - p2.y).abs() < 0.5 {
                Rect::from_min_max(
                    Pos2::new(p1.x.min(p2.x), p1.y - half_w),
                    Pos2::new(p1.x.max(p2.x), p1.y + half_w),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(p1.x - half_w, p1.y.min(p2.y)),
                    Pos2::new(p1.x + half_w, p1.y.max(p2.y)),
                )
            };
            let stroke_col = Color32::from_rgba_unmultiplied(
                COL_SDRILL.r(),
                COL_SDRILL.g(),
                COL_SDRILL.b(),
                75,
            );
            let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
            draw_dashed_closed_poly(painter, &pts, 4.0, 3.0, Stroke::new(1.0_f32, stroke_col));
        }
        Operation::Groove {
            x,
            y,
            end_x,
            end_y,
            width,
            ..
        } => {
            let p1 = Pos2::new(sx(*x), sy(*y));
            let p2 = Pos2::new(sx(*end_x), sy(*end_y));
            let gw = ((*width as f32) * scale).max(3.0);
            let col = Color32::from_rgba_unmultiplied(
                COL_GROOVE.r(),
                COL_GROOVE.g(),
                COL_GROOVE.b(),
                45,
            );
            painter.line_segment([p1, p2], Stroke::new(gw, col));
        }
    }
}

fn draw_dimensions(painter: &egui::Painter, panel: &PanelProgram, board_rect: Rect) {
    let dim_stroke = Stroke::new(1.0_f32, Color32::from_rgb(58, 64, 70));
    let ox = board_rect.left();
    let oy_top = board_rect.top();
    let oy_bot = board_rect.bottom();
    let bw = board_rect.width();
    let bh = board_rect.height();

    let dy = oy_bot + 34.0;
    painter.line_segment([Pos2::new(ox, dy), Pos2::new(ox + bw, dy)], dim_stroke);
    painter.line_segment([Pos2::new(ox, dy - 4.0), Pos2::new(ox, dy + 4.0)], dim_stroke);
    painter.line_segment(
        [Pos2::new(ox + bw, dy - 4.0), Pos2::new(ox + bw, dy + 4.0)],
        dim_stroke,
    );
    painter.text(
        Pos2::new(ox + bw * 0.5, dy + 12.0),
        Align2::CENTER_CENTER,
        format!("X = {} mm", format_num(panel.length)),
        FontId::monospace(10.5_f32),
        TX_2,
    );

    let dx = ox - 36.0;
    painter.line_segment([Pos2::new(dx, oy_top), Pos2::new(dx, oy_bot)], dim_stroke);
    painter.line_segment([Pos2::new(dx - 4.0, oy_top), Pos2::new(dx + 4.0, oy_top)], dim_stroke);
    painter.line_segment([Pos2::new(dx - 4.0, oy_bot), Pos2::new(dx + 4.0, oy_bot)], dim_stroke);
    painter.text(
        Pos2::new(dx - 6.0, oy_top + bh * 0.5),
        Align2::RIGHT_CENTER,
        format!("Y = {}", format_num(panel.width)),
        FontId::monospace(10.5_f32),
        TX_2,
    );
}

fn draw_origin_gizmo(painter: &egui::Painter, origin: Pos2) {
    // Punkt (0,0) w prawym górnym narożniku: oś +X wskazuje w lewo, oś +Y wskazuje w dół
    let al = 42.0_f32;
    let ah = 5.5_f32;
    let x_col = Color32::from_rgb(184, 92, 92);
    let y_col = Color32::from_rgb(95, 168, 108);

    let x_tip = origin - Vec2::new(al, 0.0);
    let y_tip = origin + Vec2::new(0.0, al);

    painter.line_segment([origin, x_tip], Stroke::new(1.6_f32, x_col));
    painter.add(Shape::convex_polygon(
        vec![
            x_tip,
            x_tip + Vec2::new(ah, -ah * 0.55),
            x_tip + Vec2::new(ah, ah * 0.55),
        ],
        x_col,
        Stroke::NONE,
    ));

    painter.line_segment([origin, y_tip], Stroke::new(1.6_f32, y_col));
    painter.add(Shape::convex_polygon(
        vec![
            y_tip,
            y_tip + Vec2::new(-ah * 0.55, -ah),
            y_tip + Vec2::new(ah * 0.55, -ah),
        ],
        y_col,
        Stroke::NONE,
    ));

    painter.text(
        x_tip + Vec2::new(-5.0, -7.0),
        Align2::RIGHT_CENTER,
        "X",
        FontId::monospace(10.0_f32),
        x_col,
    );
    painter.text(
        y_tip + Vec2::new(8.0, 2.0),
        Align2::LEFT_CENTER,
        "Y",
        FontId::monospace(10.0_f32),
        y_col,
    );

    painter.circle(origin, 5.5_f32, BG_CANVAS, Stroke::new(1.5_f32, ACC_2));
    painter.circle_filled(origin, 1.8_f32, ACC_2);
    painter.text(
        origin + Vec2::new(6.0, -8.0),
        Align2::LEFT_BOTTOM,
        "(0,0)",
        FontId::monospace(10.5_f32),
        ACC_2,
    );
}

fn draw_tooltip(painter: &egui::Painter, pos: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::monospace(10.5_f32), TX);
    let rect = Rect::from_min_size(pos, galley.size() + Vec2::new(14.0, 8.0));
    painter.rect_filled(rect, 0.0_f32, Color32::from_rgb(26, 31, 34));
    painter.rect_stroke(
        rect,
        0.0_f32,
        Stroke::new(1.0_f32, Color32::from_rgb(47, 58, 64)),
        StrokeKind::Outside,
    );
    painter.galley(pos + Vec2::new(7.0, 4.0), galley, TX);
}

fn screen_bounds(pts: &[Pos2]) -> Rect {
    let mut r = Rect::NOTHING;
    for &p in pts {
        r.extend_with(p);
    }
    r
}

fn is_edge_lamello_key_hole(panel: &PanelProgram, x: f64, y: f64, diameter: f64) -> bool {
    panel.operations.iter().any(|op| {
        if let Operation::LamelloP14 {
            face,
            x: lx,
            y: ly,
            ..
        } = *op
        {
            (1..=4).contains(&face)
                && nanxing_core::detector::is_hole_in_edge_lamello_zone(
                    face,
                    lx,
                    ly,
                    x,
                    y,
                    diameter,
                    panel.length,
                    panel.width,
                )
        } else {
            false
        }
    })
}

/// Rozkłada kształt „kości” gniazda powierzchniowego Lamello P14 na 3 wypukłe wielokąty
/// (środkowy wąski pas + lewy i prawy zaokrąglony koniec) dla bezbłędnego wypełnienia w egui.
fn face_socket_convex_parts(p: &P14, u0: f64, v0: f64, axis_along_u: bool) -> [Vec<Pt>; 3] {
    let l = lamello_p14::chord(p.cutter_d / 2.0, p.face_depth);
    let (h, nw, e2) = (l / 2.0, p.sock_w / 2.0, p.end_w / 2.0);
    let el = p.end_len;
    let r = p.end_r.min(el).min(e2).max(0.0);

    let map_pt = |q: Pt| -> Pt {
        if axis_along_u {
            [u0 + q[0], v0 + q[1]]
        } else {
            [u0 + q[1], v0 + q[0]]
        }
    };

    let arc = |dst: &mut Vec<Pt>, cx: f64, cy: f64, a0: f64, a1: f64| {
        if r <= 0.0 {
            return;
        }
        for i in 0..=5 {
            let a = a0 + (a1 - a0) * i as f64 / 5.0;
            dst.push(map_pt([cx + r * a.cos(), cy + r * a.sin()]));
        }
    };

    // 1. Środkowy prostokąt (wąska szyjka 7 mm)
    let center = vec![
        map_pt([-h + el, -nw]),
        map_pt([h - el, -nw]),
        map_pt([h - el, nw]),
        map_pt([-h + el, nw]),
    ];

    // 2. Prawy koniec (10 mm z pełnym półkolem)
    let mut right = Vec::with_capacity(16);
    right.push(map_pt([h - el, -e2]));
    if r > 0.0 {
        right.push(map_pt([h - r, -e2]));
        arc(&mut right, h - r, -e2 + r, -PI / 2.0, 0.0);
        arc(&mut right, h - r, e2 - r, 0.0, PI / 2.0);
        right.push(map_pt([h - el, e2]));
    } else {
        right.push(map_pt([h, -e2]));
        right.push(map_pt([h, e2]));
        right.push(map_pt([h - el, e2]));
    }

    // 3. Lewy koniec (10 mm z pełnym półkolem)
    let mut left = Vec::with_capacity(16);
    left.push(map_pt([-h + el, e2]));
    if r > 0.0 {
        left.push(map_pt([-h + r, e2]));
        arc(&mut left, -h + r, e2 - r, PI / 2.0, PI);
        arc(&mut left, -h + r, -e2 + r, PI, 1.5 * PI);
        left.push(map_pt([-h + el, -e2]));
    } else {
        left.push(map_pt([-h, e2]));
        left.push(map_pt([-h, -e2]));
        left.push(map_pt([-h + el, -e2]));
    }

    [center, right, left]
}
