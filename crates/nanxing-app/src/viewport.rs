use crate::theme::*;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use nanxing_core::clamps::{compute_clamps, op_bbox_mm};
use nanxing_core::config::AppConfig;
use nanxing_core::format_num;
use nanxing_core::model::{Operation, PanelProgram};
use std::collections::HashSet;

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

pub fn draw_cad_viewport(
    ui: &mut egui::Ui,
    panel: &PanelProgram,
    cfg: &AppConfig,
    vp: &mut ViewportState,
    selection: &mut Option<Selection>,
    external_hover_edge: Option<u8>,
) {
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
    let pad_r = 68.0_f32;
    let pad_t = 68.0_f32;
    let pad_b = 72.0_f32;
    let ex_t = (cfg.claw_depth_mm as f32).max(20.0) + 4.0;
    let ex_r = (cfg.pin_dx_mm as f32).max(16.0) + 14.0;

    let pl = panel.length.max(1.0) as f32;
    let pw = panel.width.max(1.0) as f32;

    let usable_w = (rect.width() - pad_l - pad_r).max(80.0);
    let usable_h = (rect.height() - pad_t - pad_b).max(80.0);
    let base_scale = (usable_w / (pl + ex_r)).min(usable_h / (pw + ex_t)).max(0.02);
    let scale = base_scale * vp.zoom;

    let cw_px = (pl + ex_r) * scale;
    let ch_px = (pw + ex_t) * scale;
    let ox = rect.left() + pad_l + (usable_w - cw_px) * 0.5 + vp.pan.x;
    let oy_bottom = rect.top() + pad_t + (usable_h - ch_px) * 0.5 + ch_px + vp.pan.y;

    let sx = |x_mm: f64| -> f32 { ox + (x_mm as f32) * scale };
    let sy = |y_mm: f64| -> f32 { oy_bottom - (y_mm as f32) * scale };

    let pointer_pos = response.hover_pos();
    if let Some(pointer) = pointer_pos {
        let mx = ((pointer.x - ox) / scale) as f64;
        let my = ((oy_bottom - pointer.y) / scale) as f64;
        vp.cursor_mm = Some((mx, my));
    } else {
        vp.cursor_mm = None;
    }

    // 1. Siatka (krok z konfiguracji, główne linie co 5 oczek)
    if vp.show_grid {
        draw_metric_grid(&painter, rect, ox, oy_bottom, scale, cfg.grid_step_mm as f32);
    }

    let board_rect = Rect::from_min_max(
        Pos2::new(sx(0.0), sy(panel.width)),
        Pos2::new(sx(panel.length), sy(0.0)),
    );

    // 2. Realny kontur <Outline> formatki (albo prostokąt L×W)
    let board_fill = match vp.view_mode {
        ViewMode::XRay => Color32::from_rgb(22, 25, 28),
        ViewMode::TopF5 => Color32::from_rgb(23, 26, 29),
        ViewMode::BottomF6 => Color32::from_rgb(26, 24, 22),
    };
    let board_stroke = Stroke::new(1.0_f32, Color32::from_rgb(61, 68, 74));

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

    if panel.operations.is_empty() {
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
            Pos2::new(sx(0.0), sy(0.0)),
            Pos2::new(sx(panel.length), sy(0.0)),
            Vec2::new(0.0, 13.0),
        ),
        (
            2u8,
            Pos2::new(sx(0.0), sy(panel.width)),
            Pos2::new(sx(panel.length), sy(panel.width)),
            Vec2::new(0.0, -13.0),
        ),
        (
            3u8,
            Pos2::new(sx(0.0), sy(0.0)),
            Pos2::new(sx(0.0), sy(panel.width)),
            Vec2::new(-16.0, 0.0),
        ),
        (
            4u8,
            Pos2::new(sx(panel.length), sy(0.0)),
            Pos2::new(sx(panel.length), sy(panel.width)),
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
        let alpha_fill = if is_ghost { 30u8 } else { 52u8 };

        let hit_rect = match op {
            Operation::VerticalDrill {
                x, y, diameter, ..
            } => {
                let c = Pos2::new(sx(*x), sy(*y));
                let r = (((*diameter as f32) * 0.5) * scale).max(3.5);
                painter.circle(
                    c,
                    r,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha_fill),
                    Stroke::new(if is_sel { 2.0_f32 } else { 1.1_f32 }, col),
                );
                let arm = r * 1.5;
                painter.line_segment(
                    [c - Vec2::new(arm, 0.0), c + Vec2::new(arm, 0.0)],
                    Stroke::new(1.0_f32, col),
                );
                painter.line_segment(
                    [c - Vec2::new(0.0, arm), c + Vec2::new(0.0, arm)],
                    Stroke::new(1.0_f32, col),
                );
                Rect::from_center_size(c, Vec2::splat((r + 5.0) * 2.0))
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
                let thick_w = ((*diameter as f32) * scale).max(3.0);
                painter.line_segment(
                    [p1, p2],
                    Stroke::new(
                        thick_w,
                        Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 120),
                    ),
                );
                painter.line_segment([p1, p2], Stroke::new(1.1_f32, col));
                painter.circle(
                    p1,
                    3.0_f32,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 80),
                    Stroke::new(1.0_f32, col),
                );
                Rect::from_two_pos(p1, p2).expand(thick_w * 0.5 + 4.0)
            }
            Operation::LamelloP14 {
                face,
                direction,
                x,
                y,
                length,
                width,
                ..
            } => {
                let (cx, cy) = match face {
                    4 => (7.0, *y),
                    3 => (panel.length - 7.0, *y),
                    2 => (*x, 7.0),
                    1 => (*x, panel.width - 7.0),
                    _ => (*x, *y),
                };
                let (w_mm, h_mm) = if *direction == 'Y' {
                    (*width, *length)
                } else {
                    (*length, *width)
                };
                let lrect = Rect::from_center_size(
                    Pos2::new(sx(cx), sy(cy)),
                    Vec2::new(
                        ((w_mm as f32) * scale).max(6.0),
                        ((h_mm as f32) * scale).max(6.0),
                    ),
                );
                painter.rect_filled(
                    lrect,
                    0.0_f32,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha_fill),
                );
                painter.rect_stroke(
                    lrect,
                    0.0_f32,
                    Stroke::new(if is_sel { 2.0_f32 } else { 1.1_f32 }, col),
                    StrokeKind::Inside,
                );
                lrect.expand(4.0)
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
                Rect::from_min_max(Pos2::new(sx(bx0), sy(by1)), Pos2::new(sx(bx1), sy(by0)))
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

    if clicked {
        if let Some(op_idx) = vp.hovered_op {
            *selection = Some(Selection::Op(op_idx));
        } else if let Some(edge_face) = vp.hovered_edge {
            *selection = Some(Selection::Edge(edge_face));
        } else {
            *selection = None;
        }
    }

    // 6. Łapy zaciskowe (wizualizacja orientacji — wiertarka sama przestawia łapy w trakcie programu)
    let clamps = compute_clamps(panel, cfg);
    for (i, c) in clamps.iter().enumerate() {
        let crect = Rect::from_min_max(Pos2::new(sx(c.x0), sy(c.y1)), Pos2::new(sx(c.x1), sy(c.y0)));
        let jaw_rect = Rect::from_min_max(
            Pos2::new(sx(c.x0), sy(panel.width)),
            Pos2::new(sx(c.x1), sy(panel.width - cfg.claw_grip_mm)),
        );
        let fill = Color32::from_rgba_unmultiplied(35, 39, 42, 238);
        let stroke = Color32::from_rgb(90, 99, 106);
        let jaw_col = Color32::from_rgb(67, 75, 82);
        painter.rect_filled(crect, 0.0_f32, fill);
        painter.rect_stroke(crect, 0.0_f32, Stroke::new(1.0_f32, stroke), StrokeKind::Inside);
        painter.rect_filled(jaw_rect, 0.0_f32, jaw_col);
        painter.text(
            Pos2::new(crect.center().x, sy(panel.width + cfg.claw_depth_mm * 0.5)),
            Align2::CENTER_CENTER,
            format!("Łapa {}", i + 1),
            FontId::monospace(10.0_f32),
            TX_2,
        );
    }

    // 7. Bolec pozycjonujący (parametry z AppConfig)
    let pin_center = Pos2::new(
        sx(panel.length + cfg.pin_dx_mm),
        sy((panel.width - cfg.pin_dy_mm).max(8.0)),
    );
    let pin_r = (((cfg.pin_dia_mm * 0.5) as f32) * scale).clamp(5.0, 14.0);
    let pin_stroke = Color32::from_rgb(122, 131, 138);
    painter.circle(
        pin_center,
        pin_r,
        Color32::from_rgb(35, 39, 42),
        Stroke::new(1.0_f32, pin_stroke),
    );
    painter.line_segment(
        [
            pin_center - Vec2::new(pin_r * 1.6, 0.0),
            pin_center + Vec2::new(pin_r * 1.6, 0.0),
        ],
        Stroke::new(1.0_f32, pin_stroke),
    );
    painter.line_segment(
        [
            pin_center - Vec2::new(0.0, pin_r * 1.6),
            pin_center + Vec2::new(0.0, pin_r * 1.6),
        ],
        Stroke::new(1.0_f32, pin_stroke),
    );
    painter.text(
        pin_center + Vec2::new(0.0, pin_r + 12.0),
        Align2::CENTER_TOP,
        format!("Bolec Ø{}", format_num(cfg.pin_dia_mm)),
        FontId::monospace(10.0_f32),
        TX_2,
    );

    // 8. Baza (0,0)
    draw_origin_gizmo(&painter, Pos2::new(sx(0.0), sy(0.0)));

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

    // 10. Nakładka prawa górna (ostrzeżenia limitów szerokości maszyny)
    let mut chip_y = rect.top() + 10.0;
    for w in &panel.warnings {
        draw_warning_chip(
            &painter,
            Pos2::new(rect.right() - 12.0, chip_y),
            &format!("▲ {w}"),
            WARN,
            Color32::from_rgb(28, 24, 16),
            Color32::from_rgb(107, 84, 38),
        );
        chip_y += 24.0;
    }

    // 11. Pływające przyciski Zoom (+ / - / Dopasuj)
    let zoom_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 38.0, rect.bottom() - 90.0),
        Vec2::new(26.0, 78.0),
    );
    let mut zoom_ui = ui.new_child(egui::UiBuilder::new().max_rect(zoom_rect));
    zoom_ui.spacing_mut().item_spacing = Vec2::ZERO;
    egui::Frame::NONE
        .fill(Color32::from_rgb(20, 20, 20))
        .stroke(Stroke::new(1.0_f32, LINE_2))
        .show(&mut zoom_ui, |ui| {
            if ui
                .add_sized([26.0, 26.0], egui::Button::new("+").frame(false))
                .on_hover_text("Przybliż")
                .clicked()
            {
                vp.zoom = (vp.zoom * 1.25).clamp(0.45, 8.0);
            }
            ui.separator();
            if ui
                .add_sized([26.0, 26.0], egui::Button::new("−").frame(false))
                .on_hover_text("Oddal")
                .clicked()
            {
                vp.zoom = (vp.zoom / 1.25).clamp(0.45, 8.0);
            }
            ui.separator();
            if ui
                .add_sized([26.0, 26.0], egui::Button::new("⊡").frame(false))
                .on_hover_text("Dopasuj (F)")
                .clicked()
            {
                vp.reset_camera();
            }
        });

    // 12. Tooltip
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

fn draw_metric_grid(
    painter: &egui::Painter,
    rect: Rect,
    ox: f32,
    oy_bottom: f32,
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

    let x0_mm = (rect.left() - ox) / scale;
    let x1_mm = (rect.right() - ox) / scale;
    let y0_mm = (oy_bottom - rect.bottom()) / scale;
    let y1_mm = (oy_bottom - rect.top()) / scale;

    let mut x = (x0_mm / step_mm).ceil() * step_mm;
    while x <= x1_mm {
        let sx = ox + x * scale;
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

    let mut y = (y0_mm / step_mm).ceil() * step_mm;
    while y <= y1_mm {
        let sy = oy_bottom - y * scale;
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
    let al = 44.0_f32;
    let x_col = Color32::from_rgb(184, 92, 92);
    let y_col = Color32::from_rgb(95, 168, 108);

    painter.line_segment([origin, origin + Vec2::new(al, 0.0)], Stroke::new(1.6_f32, x_col));
    painter.line_segment([origin, origin - Vec2::new(0.0, al)], Stroke::new(1.6_f32, y_col));
    painter.text(
        origin + Vec2::new(al + 6.0, 4.0),
        Align2::LEFT_CENTER,
        "X",
        FontId::monospace(10.0_f32),
        x_col,
    );
    painter.text(
        origin + Vec2::new(-8.0, -al),
        Align2::RIGHT_CENTER,
        "Y",
        FontId::monospace(10.0_f32),
        y_col,
    );

    painter.circle(origin, 6.0_f32, BG_CANVAS, Stroke::new(1.5_f32, ACC_2));
    painter.line_segment(
        [origin - Vec2::new(10.0, 0.0), origin + Vec2::new(10.0, 0.0)],
        Stroke::new(1.0_f32, ACC_2),
    );
    painter.line_segment(
        [origin - Vec2::new(0.0, 10.0), origin + Vec2::new(0.0, 10.0)],
        Stroke::new(1.0_f32, ACC_2),
    );
    painter.text(
        origin + Vec2::new(-10.0, 14.0),
        Align2::RIGHT_CENTER,
        "(0,0)",
        FontId::monospace(10.5_f32),
        ACC_2,
    );
}

fn draw_warning_chip(
    painter: &egui::Painter,
    right_top: Pos2,
    text: &str,
    tx_col: Color32,
    bg_col: Color32,
    border_col: Color32,
) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::monospace(10.5_f32), tx_col);
    let size = galley.size() + Vec2::new(16.0, 6.0);
    let rect = Rect::from_min_size(Pos2::new(right_top.x - size.x, right_top.y), size);
    painter.rect_filled(rect, 0.0_f32, bg_col);
    painter.rect_stroke(rect, 0.0_f32, Stroke::new(1.0_f32, border_col), StrokeKind::Inside);
    painter.galley(rect.left_top() + Vec2::new(8.0, 3.0), galley, tx_col);
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
